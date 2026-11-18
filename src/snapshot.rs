//! Gera um PNG do painel com dados reais, para conferir o layout sem abrir o menu.

use macmon::Sampler;
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSApplication, NSBitmapImageFileType,
};
use objc2_foundation::{NSDictionary, NSString};

use crate::dashboard::{DashboardView, STATE};

pub fn write(mtm: MainThreadMarker, path: &str, dark: bool) -> Result<(), String> {
    let _app = NSApplication::sharedApplication(mtm);
    let mut sampler = Sampler::new().map_err(|e| e.to_string())?;
    STATE.with_borrow_mut(|state| {
        state.interval_ms = 2000;
        state.opaque = true;
    });
    for _ in 0..12 {
        let sample = sampler.get_metrics(250).map_err(|e| e.to_string())?;
        STATE.with_borrow_mut(|state| state.record(sample));
    }

    let view = DashboardView::new(mtm);
    let name = unsafe { if dark { NSAppearanceNameDarkAqua } else { NSAppearanceNameAqua } };
    view.setAppearance(NSAppearance::appearanceNamed(name).as_deref());
    view.refresh();

    let bounds = view.bounds();
    let bitmap = view.bitmapImageRepForCachingDisplayInRect(bounds).ok_or("bitmap indisponível")?;
    view.cacheDisplayInRect_toBitmapImageRep(bounds, &bitmap);
    let png = unsafe {
        bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .ok_or("falha ao gerar o PNG")?;
    if png.writeToFile_atomically(&NSString::from_str(path), true) {
        Ok(())
    } else {
        Err(format!("não foi possível gravar {path}"))
    }
}
