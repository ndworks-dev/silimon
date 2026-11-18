//! Item da barra de menus, menu suspenso e amostragem em segundo plano.

use std::cell::{Cell, OnceCell, RefCell};
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use dispatch2::DispatchQueue;
use macmon::{Metrics, Sampler};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSCellImagePosition,
    NSColor, NSControlStateValueOff, NSControlStateValueOn, NSFont, NSFontAttributeName,
    NSFontWeightLight, NSFontWeightRegular, NSForegroundColorAttributeName, NSImage,
    NSImageSymbolConfiguration, NSMenu, NSMenuDelegate, NSMenuItem, NSStatusBar, NSStatusItem,
    NSVariableStatusItemLength,
};
use objc2_foundation::{
    NSAttributedString, NSAttributedStringKey, NSDictionary, NSMutableAttributedString,
    NSNotification, NSString, ns_string,
};

use crate::dashboard::{DashboardView, STATE};
use crate::metrics::{self, BarMetric, INTERVALS_MS};
use crate::prefs;

const ITEM_PADDING: f64 = 8.0;

static INTERVAL_MS: AtomicU32 = AtomicU32::new(metrics::DEFAULT_INTERVAL_MS);

thread_local! {
    static DELEGATE: OnceCell<Retained<AppDelegate>> = const { OnceCell::new() };
}

#[derive(Default)]
pub struct Ivars {
    status_item: OnceCell<Retained<NSStatusItem>>,
    dashboard: OnceCell<Retained<DashboardView>>,
    metric_items: RefCell<Vec<Retained<NSMenuItem>>>,
    interval_items: RefCell<Vec<Retained<NSMenuItem>>>,
    selection: RefCell<Vec<BarMetric>>,
    last_title: RefCell<String>,
    reserved_width: Cell<Option<f64>>,
    menu_open: Cell<bool>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SilimonAppDelegate"]
    #[ivars = Ivars]
    pub struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            self.build_interface();
            spawn_sampler();
        }
    }

    unsafe impl NSMenuDelegate for AppDelegate {
        #[unsafe(method(menuWillOpen:))]
        fn menu_will_open(&self, _menu: &NSMenu) {
            self.ivars().menu_open.set(true);
            if let Some(view) = self.ivars().dashboard.get() {
                view.refresh();
            }
        }

        #[unsafe(method(menuDidClose:))]
        fn menu_did_close(&self, _menu: &NSMenu) {
            self.ivars().menu_open.set(false);
        }
    }

    impl AppDelegate {
        #[unsafe(method(toggleMetric:))]
        fn toggle_metric(&self, sender: &NSMenuItem) {
            let metric = BarMetric::ALL[sender.tag() as usize];
            {
                let mut selection = self.ivars().selection.borrow_mut();
                let enabled = !selection.contains(&metric);
                selection.retain(|m| *m != metric);
                if enabled {
                    selection.push(metric);
                }
                selection.sort_by_key(|m| BarMetric::ALL.iter().position(|x| x == m));
                prefs::save_selection(&selection);
            }
            self.ivars().reserved_width.set(None);
            self.ivars().last_title.borrow_mut().clear();
            self.sync_checkmarks();
            self.update_title();
        }

        #[unsafe(method(selectInterval:))]
        fn select_interval(&self, sender: &NSMenuItem) {
            let interval = sender.tag() as u32;
            INTERVAL_MS.store(interval, Ordering::Relaxed);
            STATE.with_borrow_mut(|state| state.interval_ms = interval);
            prefs::save_interval(interval);
            self.sync_checkmarks();
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars::default());
        unsafe { msg_send![super(this), init] }
    }

    fn menu_item(&self, title: &str, action: objc2::runtime::Sel, tag: isize) -> Retained<NSMenuItem> {
        let mtm = self.mtm();
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(title),
                Some(action),
                ns_string!(""),
            )
        };
        let target: &AnyObject = self.as_ref();
        unsafe { item.setTarget(Some(target)) };
        item.setTag(tag);
        item
    }

    fn submenu(&self, title: &str, items: &[Retained<NSMenuItem>]) -> Retained<NSMenuItem> {
        let mtm = self.mtm();
        let menu = NSMenu::new(mtm);
        for item in items {
            menu.addItem(item);
        }
        let parent = NSMenuItem::new(mtm);
        parent.setTitle(&NSString::from_str(title));
        parent.setSubmenu(Some(&menu));
        parent
    }

    fn build_interface(&self) {
        let mtm = self.mtm();
        let ivars = self.ivars();
        ivars.selection.replace(prefs::load_selection());
        let interval = prefs::load_interval();
        INTERVAL_MS.store(interval, Ordering::Relaxed);
        STATE.with_borrow_mut(|state| state.interval_ms = interval);

        let menu = NSMenu::new(mtm);
        menu.setDelegate(Some(ProtocolObject::from_ref(self)));

        let dashboard = DashboardView::new(mtm);
        dashboard.refresh();
        let dashboard_item = NSMenuItem::new(mtm);
        dashboard_item.setView(Some(&dashboard));
        menu.addItem(&dashboard_item);
        menu.addItem(&NSMenuItem::separatorItem(mtm));

        let metric_items: Vec<_> = BarMetric::ALL
            .iter()
            .enumerate()
            .map(|(tag, m)| self.menu_item(m.title(), sel!(toggleMetric:), tag as isize))
            .collect();
        menu.addItem(&self.submenu("Mostrar na barra", &metric_items));
        ivars.metric_items.replace(metric_items);

        let interval_items: Vec<_> = INTERVALS_MS
            .iter()
            .map(|ms| {
                let seconds = ms / 1000;
                let unit = if seconds == 1 { "segundo" } else { "segundos" };
                self.menu_item(&format!("A cada {seconds} {unit}"), sel!(selectInterval:), *ms as isize)
            })
            .collect();
        menu.addItem(&self.submenu("Atualização", &interval_items));
        ivars.interval_items.replace(interval_items);
        menu.addItem(&NSMenuItem::separatorItem(mtm));

        let quit = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                ns_string!("Sair do Silimon"),
                Some(sel!(terminate:)),
                ns_string!("q"),
            )
        };
        menu.addItem(&quit);

        let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        if let Some(button) = item.button(mtm) {
            let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
                ns_string!("flame.fill"),
                Some(ns_string!("Silimon")),
            );
            let weight = unsafe { NSFontWeightLight };
            let config = NSImageSymbolConfiguration::configurationWithPointSize_weight(12.0, weight);
            let image = image.and_then(|image| image.imageWithSymbolConfiguration(&config));
            if let Some(image) = image {
                image.setTemplate(true);
                button.setImage(Some(&image));
            }
            button.setImagePosition(NSCellImagePosition::ImageLeft);
        }
        item.setMenu(Some(&menu));

        let _ = ivars.status_item.set(item);
        let _ = ivars.dashboard.set(dashboard);
        self.sync_checkmarks();
        self.update_title();
    }

    fn sync_checkmarks(&self) {
        let state = |on: bool| if on { NSControlStateValueOn } else { NSControlStateValueOff };
        let selection = self.ivars().selection.borrow();
        for (item, metric) in self.ivars().metric_items.borrow().iter().zip(BarMetric::ALL) {
            item.setState(state(selection.contains(&metric)));
        }
        let interval = INTERVAL_MS.load(Ordering::Relaxed);
        for (item, ms) in self.ivars().interval_items.borrow().iter().zip(INTERVALS_MS) {
            item.setState(state(ms == interval));
        }
    }

    fn update_title(&self) {
        let ivars = self.ivars();
        let (title, segments) = STATE.with_borrow(|state| {
            let selection = ivars.selection.borrow();
            (
                metrics::bar_title(&selection, state.metrics.as_ref()),
                metrics::bar_segments(&selection, state.metrics.as_ref()),
            )
        });
        if *ivars.last_title.borrow() == title {
            return;
        }
        if let Some(item) = ivars.status_item.get()
            && let Some(button) = item.button(self.mtm())
        {
            let reserved = ivars.reserved_width.get().unwrap_or_else(|| {
                let template = metrics::bar_template(&ivars.selection.borrow());
                button.setAttributedTitle(&styled_title(&template));
                let width = button.intrinsicContentSize().width;
                ivars.reserved_width.set(Some(width));
                width
            });
            button.setAttributedTitle(&styled_title(&segments));
            // Só cresce se um valor realmente exceder o espaço reservado.
            let length = (reserved.max(button.intrinsicContentSize().width) + ITEM_PADDING).ceil();
            if (item.length() - length).abs() > 0.5 {
                item.setLength(length);
            }
        }
        ivars.last_title.replace(title);
    }

    fn on_sample(&self, sample: Metrics) {
        STATE.with_borrow_mut(|state| state.record(sample));
        self.update_title();
        if self.ivars().menu_open.get() {
            if let Some(view) = self.ivars().dashboard.get() {
                view.refresh();
            }
        }
    }
}

fn styled_run(text: &str, size: f64, alpha: f64) -> Retained<NSAttributedString> {
    let font = NSFont::monospacedDigitSystemFontOfSize_weight(size, unsafe { NSFontWeightRegular });
    let color = NSColor::labelColor().colorWithAlphaComponent(alpha);
    let keys: [&NSAttributedStringKey; 2] =
        unsafe { [NSFontAttributeName, NSForegroundColorAttributeName] };
    let objects: [&AnyObject; 2] = [font.as_ref(), color.as_ref()];
    let attributes = NSDictionary::from_slices(&keys, &objects);
    unsafe {
        NSAttributedString::new_with_attributes(&NSString::from_str(text), &attributes)
    }
}

/// Rótulos pequenos e discretos, valores um pouco mais presentes.
fn styled_title(segments: &[(&str, String)]) -> Retained<NSAttributedString> {
    let title = NSMutableAttributedString::new();
    for (index, (label, value)) in segments.iter().enumerate() {
        if index > 0 {
            title.appendAttributedString(&styled_run("   ", 12.0, 1.0));
        }
        if !label.is_empty() {
            title.appendAttributedString(&styled_run(&format!("{label} "), 9.5, 0.6));
        }
        title.appendAttributedString(&styled_run(value, 12.0, 0.88));
    }
    title.into_super()
}

fn spawn_sampler() {
    thread::Builder::new()
        .name("silimon-sampler".into())
        .spawn(|| {
            loop {
                let mut sampler = match Sampler::new() {
                    Ok(sampler) => sampler,
                    Err(error) => {
                        eprintln!("silimon: falha ao iniciar os sensores: {error}");
                        thread::sleep(Duration::from_secs(5));
                        continue;
                    }
                };
                loop {
                    match sampler.get_metrics(INTERVAL_MS.load(Ordering::Relaxed)) {
                        Ok(sample) => DispatchQueue::main().exec_async(move || {
                            DELEGATE.with(|cell| {
                                if let Some(delegate) = cell.get() {
                                    delegate.on_sample(sample);
                                }
                            });
                        }),
                        Err(error) => {
                            eprintln!("silimon: falha na leitura: {error}");
                            thread::sleep(Duration::from_secs(5));
                            break;
                        }
                    }
                }
            }
        })
        .expect("não foi possível iniciar a thread de amostragem");
}

pub fn run(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let delegate = AppDelegate::new(mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    DELEGATE.with(|cell| {
        let _ = cell.set(delegate);
    });
    app.run();
}
