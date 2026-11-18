mod app;
mod dashboard;
mod metrics;
mod prefs;
mod snapshot;

use objc2::MainThreadMarker;

fn main() {
    let mtm = MainThreadMarker::new().expect("o app precisa iniciar na thread principal");
    let args: Vec<String> = std::env::args().skip(1).collect();
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
