//! The settings window.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, msg_send, sel};
use objc2_app_kit::{
    NSBackingStoreType, NSBox, NSBoxType, NSButton, NSColor, NSControlSize, NSFont,
    NSFontWeightMedium, NSLayoutAttribute, NSLayoutConstraintOrientation, NSPopUpButton,
    NSStackView, NSSwitch, NSTextField, NSTitlePosition, NSUserInterfaceLayoutOrientation, NSView,
    NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSArray, NSEdgeInsets, NSPoint, NSRect, NSSize, NSString};

use crate::metrics::{BarMetric, INTERVALS_MS};
use crate::update;

const WINDOW_WIDTH: f64 = 420.0;
const MARGIN: f64 = 20.0;
const GROUP_WIDTH: f64 = WINDOW_WIDTH - MARGIN * 2.0;
const ROW_HEIGHT: f64 = 36.0;
const METRIC_ROW_HEIGHT: f64 = 30.0;

pub struct SettingsWindow {
    pub window: Retained<NSWindow>,
    pub metric_switches: Vec<Retained<NSSwitch>>,
    pub interval: Retained<NSPopUpButton>,
    pub login: Retained<NSSwitch>,
    pub login_note: Retained<NSTextField>,
    pub update_button: Retained<NSButton>,
}

fn label(text: &str, mtm: MainThreadMarker) -> Retained<NSTextField> {
    let field = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    field.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);
    field
}

fn switch(title: &str, target: &AnyObject, action: Sel, mtm: MainThreadMarker) -> Retained<NSSwitch> {
    let control = NSSwitch::new(mtm);
    control.setControlSize(NSControlSize::Small);
    unsafe {
        control.setTarget(Some(target));
        control.setAction(Some(action));
        let _: () = msg_send![&*control, setAccessibilityLabel: &*NSString::from_str(title)];
    }
    control
}

fn row(left: &NSView, right: &NSView, height: f64, mtm: MainThreadMarker) -> Retained<NSStackView> {
    let row = NSStackView::stackViewWithViews(&NSArray::from_slice(&[left, right]), mtm);
    row.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
    row.setAlignment(NSLayoutAttribute::CenterY);
    row.setEdgeInsets(NSEdgeInsets { top: 0.0, left: 12.0, bottom: 0.0, right: 12.0 });
    row.setTranslatesAutoresizingMaskIntoConstraints(false);
    row.widthAnchor().constraintEqualToConstant(GROUP_WIDTH).setActive(true);
    row.heightAnchor().constraintGreaterThanOrEqualToConstant(height).setActive(true);
    row
}

/// A titled, rounded group of rows, appended to the window's root stack.
fn add_group(root: &NSStackView, title: &str, rows: &[Retained<NSStackView>], mtm: MainThreadMarker) {
    let heading = label(title, mtm);
    heading.setFont(Some(&NSFont::systemFontOfSize_weight(12.0, unsafe { NSFontWeightMedium })));
    heading.setTextColor(Some(&NSColor::secondaryLabelColor()));
    root.addArrangedSubview(&heading);

    let inner = NSStackView::new(mtm);
    inner.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    inner.setAlignment(NSLayoutAttribute::Leading);
    inner.setSpacing(0.0);
    inner.setTranslatesAutoresizingMaskIntoConstraints(false);
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            let separator = NSBox::new(mtm);
            separator.setBoxType(NSBoxType::Separator);
            separator.setTranslatesAutoresizingMaskIntoConstraints(false);
            separator.widthAnchor().constraintEqualToConstant(GROUP_WIDTH).setActive(true);
            inner.addArrangedSubview(&separator);
        }
        inner.addArrangedSubview(row);
    }

    let group = NSBox::new(mtm);
    group.setBoxType(NSBoxType::Custom);
    group.setTitlePosition(NSTitlePosition::NoTitle);
    group.setBorderColor(&NSColor::separatorColor());
    group.setCornerRadius(10.0);
    group.setFillColor(&NSColor::labelColor().colorWithAlphaComponent(0.04));
    group.setContentViewMargins(NSSize::new(0.0, 0.0));
    group.setTranslatesAutoresizingMaskIntoConstraints(false);
    group.widthAnchor().constraintEqualToConstant(GROUP_WIDTH).setActive(true);
    let content = group.contentView().expect("a box always has a content view");
    content.addSubview(&inner);
    inner.topAnchor().constraintEqualToAnchor(&content.topAnchor()).setActive(true);
    inner.bottomAnchor().constraintEqualToAnchor(&content.bottomAnchor()).setActive(true);
    inner.leadingAnchor().constraintEqualToAnchor(&content.leadingAnchor()).setActive(true);
    inner.trailingAnchor().constraintEqualToAnchor(&content.trailingAnchor()).setActive(true);

    root.addArrangedSubview(&group);
    root.setCustomSpacing_afterView(18.0, &group);
}

pub fn build(target: &AnyObject, mtm: MainThreadMarker) -> SettingsWindow {
    let root = NSStackView::new(mtm);
    root.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    root.setAlignment(NSLayoutAttribute::Leading);
    root.setSpacing(6.0);
    root.setEdgeInsets(NSEdgeInsets { top: MARGIN, left: MARGIN, bottom: MARGIN, right: MARGIN });
    root.setTranslatesAutoresizingMaskIntoConstraints(false);
    root.widthAnchor().constraintEqualToConstant(WINDOW_WIDTH).setActive(true);

    let login = switch("Open at Login", target, sel!(toggleLoginItem:), mtm);
    login.setEnabled(update::bundle_path().is_some());
    let login_title = label("Open at Login", mtm);
    let login_note = label("Waiting for your approval in System Settings", mtm);
    login_note.setFont(Some(&NSFont::systemFontOfSize(11.0)));
    login_note.setTextColor(Some(&NSColor::secondaryLabelColor()));
    login_note.setHidden(true);
    let login_text = NSStackView::stackViewWithViews(
        &NSArray::from_slice(&[login_title.as_ref() as &NSView, login_note.as_ref()]),
        mtm,
    );
    login_text.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    login_text.setAlignment(NSLayoutAttribute::Leading);
    login_text.setSpacing(1.0);
    login_text.setContentHuggingPriority_forOrientation(1.0, NSLayoutConstraintOrientation::Horizontal);

    let interval =
        NSPopUpButton::initWithFrame_pullsDown(NSPopUpButton::alloc(mtm), NSRect::ZERO, false);
    for ms in INTERVALS_MS {
        let seconds = ms / 1000;
        let unit = if seconds == 1 { "second" } else { "seconds" };
        interval.addItemWithTitle(&NSString::from_str(&format!("Every {seconds} {unit}")));
        if let Some(item) = interval.lastItem() {
            item.setTag(ms as isize);
        }
    }
    unsafe {
        interval.setTarget(Some(target));
        interval.setAction(Some(sel!(selectInterval:)));
    }
    let general = [
        row(&login_text, &login, ROW_HEIGHT, mtm),
        row(&label("Refresh Readings", mtm), &interval, ROW_HEIGHT, mtm),
    ];
    add_group(&root, "General", &general, mtm);

    let metric_switches: Vec<_> = BarMetric::ALL
        .iter()
        .map(|metric| switch(metric.title(), target, sel!(toggleMetric:), mtm))
        .collect();
    let metric_rows: Vec<_> = BarMetric::ALL
        .iter()
        .zip(&metric_switches)
        .map(|(metric, control)| row(&label(metric.title(), mtm), control, METRIC_ROW_HEIGHT, mtm))
        .collect();
    add_group(&root, "Show in Menu Bar", &metric_rows, mtm);

    let update_button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str("Check for Updates…"),
            Some(target),
            Some(sel!(updateAction:)),
            mtm,
        )
    };
    let version = label(&format!("Version {}", update::VERSION), mtm);
    add_group(&root, "Updates", &[row(&version, &update_button, ROW_HEIGHT, mtm)], mtm);

    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WINDOW_WIDTH, 400.0));
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            NSWindowStyleMask::Titled | NSWindowStyleMask::Closable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(&NSString::from_str("Silimon Settings"));
    window.setContentView(Some(&root));
    window.setContentSize(root.fittingSize());
    window.center();

    SettingsWindow { window, metric_switches, interval, login, login_note, update_button }
}
