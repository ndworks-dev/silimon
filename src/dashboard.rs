//! The panel drawn inside the dropdown menu.

use std::cell::RefCell;

use macmon::Metrics;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAppearanceCustomization, NSBezierPath, NSColor, NSFont, NSFontAttributeName,
    NSFontWeightMedium, NSFontWeightRegular, NSForegroundColorAttributeName, NSView,
};
use objc2_foundation::{
    NSAttributedStringKey, NSDictionary, NSPoint, NSProcessInfo, NSProcessInfoThermalState, NSRect,
    NSSize, NSString,
};

use crate::metrics::{self, HISTORY_LEN, History};

pub const WIDTH: f64 = 300.0;
const MARGIN: f64 = 14.0;
const ROW: f64 = 19.0;
const BODY: f64 = 12.0;

#[derive(Default)]
pub struct DashboardState {
    pub metrics: Option<Metrics>,
    pub cpu_temp: History,
    pub gpu_temp: History,
    pub pressure: History,
    pub interval_ms: u32,
    /// Fills the background when exporting a PNG, since the system draws it in the menu.
    pub opaque: bool,
}

impl DashboardState {
    pub fn record(&mut self, metrics: Metrics) {
        self.cpu_temp.push(metrics.temp.cpu_temp_avg);
        self.gpu_temp.push(metrics.temp.gpu_temp_avg);
        self.pressure.push(metrics.memory.pressure_percent.map(f32::from));
        self.metrics = Some(metrics);
    }
}

thread_local! {
    pub static STATE: RefCell<DashboardState> = RefCell::default();
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "SilimonDashboardView"]
    pub struct DashboardView;

    impl DashboardView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let dark = self.effectiveAppearance().name().to_string().contains("Dark");
            STATE.with_borrow(|state| {
                render(state, &mut Canvas { y: 0.0, draw: true, dark });
            });
        }
    }
);

impl DashboardView {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, 120.0));
        let this = Self::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    pub fn refresh(&self) {
        let height = STATE.with_borrow(|state| {
            let mut canvas = Canvas { y: 0.0, draw: false, dark: false };
            render(state, &mut canvas);
            canvas.y
        });
        if (self.frame().size.height - height).abs() > 0.5 {
            self.setFrameSize(NSSize::new(WIDTH, height));
        }
        self.setNeedsDisplay(true);
    }
}

#[derive(Clone, Copy)]
enum Status {
    Good,
    Warning,
    Serious,
    Critical,
}

impl Status {
    fn color(self) -> Retained<NSColor> {
        hex(match self {
            Status::Good => 0x0ca30c,
            Status::Warning => 0xfab219,
            Status::Serious => 0xec835a,
            Status::Critical => 0xd03b3b,
        })
    }
}

fn plot_background() -> Retained<NSColor> {
    NSColor::labelColor().colorWithAlphaComponent(0.045)
}

fn value_color() -> Retained<NSColor> {
    NSColor::labelColor().colorWithAlphaComponent(0.78)
}

fn label_color() -> Retained<NSColor> {
    NSColor::labelColor().colorWithAlphaComponent(0.52)
}

fn hex(rgb: u32) -> Retained<NSColor> {
    let channel = |shift: u32| f64::from((rgb >> shift) & 0xff) / 255.0;
    NSColor::colorWithSRGBRed_green_blue_alpha(channel(16), channel(8), channel(0), 1.0)
}

fn text_attributes(
    size: f64,
    weight: f64,
    color: &NSColor,
) -> Retained<NSDictionary<NSAttributedStringKey, AnyObject>> {
    let font = NSFont::monospacedDigitSystemFontOfSize_weight(size, weight);
    let keys: [&NSAttributedStringKey; 2] =
        unsafe { [NSFontAttributeName, NSForegroundColorAttributeName] };
    let objects: [&AnyObject; 2] = [font.as_ref(), color.as_ref()];
    NSDictionary::from_slices(&keys, &objects)
}

struct Canvas {
    y: f64,
    draw: bool,
    dark: bool,
}

impl Canvas {
    fn series(&self, slot: usize) -> Retained<NSColor> {
        hex(match (slot, self.dark) {
            (0, false) => 0x2a78d6,
            (0, true) => 0x3987e5,
            (_, false) => 0xeb6834,
            (_, true) => 0xd95926,
        })
    }

    fn text(&self, value: &str, x: f64, y: f64, size: f64, weight: f64, color: &NSColor, right: bool) {
        if !self.draw {
            return;
        }
        let attributes = text_attributes(size, weight, color);
        let string = NSString::from_str(value);
        let width: NSSize = unsafe { msg_send![&*string, sizeWithAttributes: &*attributes] };
        let origin = NSPoint::new(if right { x - width.width } else { x }, y);
        let _: () = unsafe { msg_send![&*string, drawAtPoint: origin, withAttributes: &*attributes] };
    }

    fn rounded(&self, x: f64, y: f64, w: f64, h: f64, radius: f64, color: &NSColor) {
        if !self.draw {
            return;
        }
        let rect = NSRect::new(NSPoint::new(x, y), NSSize::new(w, h));
        color.setFill();
        NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect, radius, radius).fill();
    }

    fn section(&mut self, title: &str) {
        self.y += 7.0;
        if self.draw {
            let path = NSBezierPath::bezierPath();
            path.moveToPoint(NSPoint::new(MARGIN, self.y + 0.5));
            path.lineToPoint(NSPoint::new(WIDTH - MARGIN, self.y + 0.5));
            path.setLineWidth(1.0);
            NSColor::separatorColor().setStroke();
            path.stroke();
        }
        self.y += 7.0;
        self.text(title, MARGIN, self.y, 11.0, unsafe { NSFontWeightMedium }, &label_color(), false);
        self.y += 18.0;
    }

    fn row(&mut self, label: &str, value: &str) {
        let weight = unsafe { NSFontWeightRegular };
        self.text(label, MARGIN, self.y, BODY, weight, &label_color(), false);
        self.text(value, WIDTH - MARGIN, self.y, BODY, weight, &value_color(), true);
        self.y += ROW;
    }

    fn pair(&mut self, left: (&str, String), right: (&str, String)) {
        let middle = WIDTH / 2.0;
        let (label, value) = (label_color(), value_color());
        let regular = unsafe { NSFontWeightRegular };
        self.text(left.0, MARGIN, self.y, BODY, regular, &label, false);
        self.text(&left.1, middle - 8.0, self.y, BODY, regular, &value, true);
        self.text(right.0, middle + 8.0, self.y, BODY, regular, &label, false);
        self.text(&right.1, WIDTH - MARGIN, self.y, BODY, regular, &value, true);
        self.y += ROW;
    }

    fn status_row(&mut self, label: &str, value: &str, status: Status) {
        if self.draw {
            let attributes = text_attributes(BODY, unsafe { NSFontWeightRegular }, &value_color());
            let string = NSString::from_str(value);
            let size: NSSize = unsafe { msg_send![&*string, sizeWithAttributes: &*attributes] };
            let dot_x = WIDTH - MARGIN - size.width - 12.0;
            self.rounded(dot_x, self.y + 5.5, 6.0, 6.0, 3.0, &status.color());
        }
        self.row(label, value);
    }

    fn meter(&mut self, ratio: f32, color: &NSColor) {
        let width = WIDTH - MARGIN * 2.0;
        self.rounded(MARGIN, self.y + 1.0, width, 4.0, 2.0, &NSColor::quaternaryLabelColor());
        let filled = (width * f64::from(ratio.clamp(0.0, 1.0))).max(4.0);
        self.rounded(MARGIN, self.y + 1.0, filled, 4.0, 2.0, color);
        self.y += 12.0;
    }

    fn line(&self, values: &History, range: (f32, f32), frame: NSRect, color: &NSColor) {
        let count = values.values().len();
        if count == 0 {
            return;
        }
        let step = frame.size.width / (HISTORY_LEN - 1) as f64;
        let right = frame.origin.x + frame.size.width;
        let span = f64::from(range.1 - range.0);
        let path = NSBezierPath::bezierPath();
        let mut pen_down = false;
        for (index, value) in values.values().enumerate() {
            if !value.is_finite() {
                pen_down = false;
                continue;
            }
            let level = (f64::from(value - range.0) / span).clamp(0.0, 1.0);
            let point = NSPoint::new(
                right - (count - 1 - index) as f64 * step,
                frame.origin.y + frame.size.height * (1.0 - level),
            );
            if pen_down { path.lineToPoint(point) } else { path.moveToPoint(point) }
            pen_down = true;
        }
        path.setLineWidth(1.5);
        color.setStroke();
        path.stroke();
    }

    fn chart_frame(&self, height: f64) -> NSRect {
        NSRect::new(
            NSPoint::new(MARGIN + 8.0, self.y + 8.0),
            NSSize::new(WIDTH - MARGIN * 2.0 - 40.0, height - 16.0),
        )
    }

    fn temperature_chart(&mut self, state: &DashboardState) {
        const HEIGHT: f64 = 92.0;
        if self.draw {
            let range = metrics::temperature_range([&state.cpu_temp, &state.gpu_temp]);
            let frame = self.chart_frame(HEIGHT);
            self.rounded(MARGIN, self.y, WIDTH - MARGIN * 2.0, HEIGHT, 8.0, &plot_background());
            let muted = label_color();
            let regular = unsafe { NSFontWeightRegular };
            let right = WIDTH - MARGIN - 8.0;
            self.text(&format!("{:.0}°", range.1), right, frame.origin.y - 6.0, 10.0, regular, &muted, true);
            self.text(&format!("{:.0}°", range.0), right, frame.origin.y + frame.size.height - 7.0, 10.0, regular, &muted, true);
            self.line(&state.gpu_temp, range, frame, &self.series(1));
            self.line(&state.cpu_temp, range, frame, &self.series(0));
        }
        self.y += HEIGHT + 6.0;
        let muted = label_color();
        let regular = unsafe { NSFontWeightRegular };
        self.rounded(MARGIN, self.y + 4.0, 6.0, 6.0, 3.0, &self.series(0));
        self.text("CPU", MARGIN + 11.0, self.y, 11.0, regular, &muted, false);
        if state.gpu_temp.values().any(f32::is_finite) {
            self.rounded(MARGIN + 50.0, self.y + 4.0, 6.0, 6.0, 3.0, &self.series(1));
            self.text("GPU", MARGIN + 61.0, self.y, 11.0, regular, &muted, false);
        }
        let minutes = f64::from(state.interval_ms) * HISTORY_LEN as f64 / 60_000.0;
        self.text(&format!("last {minutes:.0} min"), WIDTH - MARGIN, self.y, 11.0, regular, &muted, true);
        self.y += 18.0;
    }

    fn pressure_chart(&mut self, state: &DashboardState) {
        const HEIGHT: f64 = 52.0;
        if self.draw {
            self.rounded(MARGIN, self.y, WIDTH - MARGIN * 2.0, HEIGHT, 6.0, &plot_background());
            let frame = self.chart_frame(HEIGHT);
            let muted = label_color();
            let regular = unsafe { NSFontWeightRegular };
            let right = WIDTH - MARGIN - 8.0;
            self.text("100%", right, frame.origin.y - 6.0, 10.0, regular, &muted, true);
            self.text("0%", right, frame.origin.y + frame.size.height - 7.0, 10.0, regular, &muted, true);
            self.line(&state.pressure, (0.0, 100.0), frame, &self.series(0));
        }
        self.y += HEIGHT + 6.0;
    }
}

fn thermal_status() -> (&'static str, Status) {
    let state = NSProcessInfo::processInfo().thermalState();
    if state == NSProcessInfoThermalState::Critical {
        ("Critical", Status::Critical)
    } else if state == NSProcessInfoThermalState::Serious {
        ("Serious", Status::Serious)
    } else if state == NSProcessInfoThermalState::Fair {
        ("Fair", Status::Warning)
    } else {
        ("Nominal", Status::Good)
    }
}

fn pressure_status(level: Option<u8>) -> (&'static str, Status) {
    match level {
        Some(2) => ("Critical", Status::Critical),
        Some(1) => ("Warning", Status::Warning),
        _ => ("Normal", Status::Good),
    }
}

fn temperature(value: Option<f32>) -> String {
    value.map_or("—".into(), |t| format!("{t:.0}°C"))
}

fn tier_name(label: &str) -> String {
    match label {
        "E" => "E-cores".into(),
        "P" => "P-cores".into(),
        "S" => "S-cores".into(),
        _ => "Cores".into(),
    }
}

fn render(state: &DashboardState, c: &mut Canvas) {
    if c.draw && state.opaque {
        NSColor::windowBackgroundColor().setFill();
        NSBezierPath::fillRect(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, 4000.0)));
    }
    c.y = 8.0;
    let (thermal, status) = thermal_status();
    c.status_row("Thermal Pressure", thermal, status);
    let Some(m) = &state.metrics else {
        c.row("Sensors", "Reading…");
        c.y += 6.0;
        return;
    };
    c.row("CPU Temperature", &temperature(m.temp.cpu_temp_avg));
    if m.temp.gpu_temp_avg.is_some() {
        c.row("GPU Temperature", &temperature(m.temp.gpu_temp_avg));
    }
    c.y += 4.0;
    c.temperature_chart(state);

    c.section("Memory");
    let (label, status) = pressure_status(m.memory.pressure_level);
    let percent = m.memory.pressure_percent;
    let value = percent.map_or(label.to_string(), |p| format!("{label} · {p}%"));
    c.status_row("Pressure", &value, status);
    c.meter(f32::from(percent.unwrap_or(0)) / 100.0, &status.color());
    c.pressure_chart(state);
    let used = metrics::gigabytes(m.memory.ram_usage);
    c.row("In Use", &format!("{used} of {}", metrics::gigabytes(m.memory.ram_total)));
    c.row("Swap", &metrics::gigabytes(m.memory.swap_usage));

    c.section("Processors");
    c.row("CPU", &metrics::percent(m.cpu_active_ratio));
    for tier in &m.cpu_tiers {
        let value = format!("{} · {} MHz", metrics::percent(tier.active_ratio), tier.freq_mhz);
        c.row(&tier_name(&tier.label), &value);
    }
    let gpu = format!("{} · {} MHz", metrics::percent(m.gpu_active_ratio), m.gpu_freq_mhz);
    c.row("GPU", &gpu);

    c.section("Power");
    c.pair(("CPU", metrics::watts(m.cpu_power)), ("GPU", metrics::watts(m.gpu_power)));
    c.pair(("RAM", metrics::watts(m.ram_power)), ("ANE", metrics::watts(m.ane_power)));
    c.pair(("Chip", metrics::watts(m.chip_power)), ("System", metrics::watts(m.sys_power)));

    c.section("Activity");
    if let (Some(down), Some(up)) = (m.net_in, m.net_out) {
        c.row("Network", &format!("↓ {}  ↑ {}", metrics::rate(down), metrics::rate(up)));
    }
    if let (Some(read), Some(write)) = (m.disk_read, m.disk_write) {
        c.row("Disk", &format!("R {}  W {}", metrics::rate(read), metrics::rate(write)));
    }
    for (index, fan) in m.fans.iter().enumerate() {
        c.row(&format!("Fan {}", index + 1), &format!("{} RPM", fan.rpm));
    }
    if let Some(battery) = m.battery_percent {
        let suffix = match (m.battery_charging, m.battery_plugged) {
            (Some(true), _) => " · charging",
            (_, Some(true)) => " · plugged in",
            _ => "",
        };
        c.row("Battery", &format!("{battery}%{suffix}"));
    }
    c.y += 4.0;
}
