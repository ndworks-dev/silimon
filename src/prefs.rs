//! Preferences persisted in NSUserDefaults.

use objc2_foundation::{NSString, NSUserDefaults};

use crate::metrics::{self, BarMetric, DEFAULT_INTERVAL_MS, INTERVALS_MS};

const BAR_KEY: &str = "barMetrics";
const INTERVAL_KEY: &str = "sampleIntervalMs";

pub fn load_selection() -> Vec<BarMetric> {
    let defaults = NSUserDefaults::standardUserDefaults();
    let saved = defaults.stringForKey(&NSString::from_str(BAR_KEY)).map(|s| s.to_string());
    metrics::parse_selection(saved.as_deref())
}

pub fn save_selection(selection: &[BarMetric]) {
    let value = NSString::from_str(&metrics::serialize_selection(selection));
    let defaults = NSUserDefaults::standardUserDefaults();
    unsafe { defaults.setObject_forKey(Some(&value), &NSString::from_str(BAR_KEY)) };
}

pub fn load_interval() -> u32 {
    let defaults = NSUserDefaults::standardUserDefaults();
    let saved = defaults.integerForKey(&NSString::from_str(INTERVAL_KEY)) as u32;
    if INTERVALS_MS.contains(&saved) { saved } else { DEFAULT_INTERVAL_MS }
}

pub fn save_interval(interval_ms: u32) {
    let defaults = NSUserDefaults::standardUserDefaults();
    defaults.setInteger_forKey(interval_ms as isize, &NSString::from_str(INTERVAL_KEY));
}
