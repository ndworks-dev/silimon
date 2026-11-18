mod app;
mod dashboard;
mod login_item;
mod metrics;
mod prefs;
mod settings;
mod snapshot;
mod update;

use objc2::MainThreadMarker;

fn main() {
    let mtm = MainThreadMarker::new().expect("the app must start on the main thread");
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [flag, archive, sha256, app] = args.as_slice() {
        if flag == "--apply-update" {
            if let Err(error) = update::install_archive(archive.as_ref(), sha256, app.as_ref()) {
                eprintln!("silimon: {error}");
                std::process::exit(1);
            }
            return;
        }
    }
    if let [flag, path, rest @ ..] = args.as_slice() {
        if flag == "--snapshot" {
            let dark = rest.iter().any(|arg| arg == "--dark");
            if let Err(error) = snapshot::write(mtm, path, dark) {
                eprintln!("silimon: {error}");
                std::process::exit(1);
            }
            return;
        }
    }
    app::run(mtm);
}
