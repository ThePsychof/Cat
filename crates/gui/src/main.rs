// Console temporarily left enabled (no windows_subsystem override) so any
// crash or panic during GUI bring-up is actually visible. Re-add
// `#![cfg_attr(windows, windows_subsystem = "windows")]` once the app is
// confirmed stable.

fn main() {
    if let Err(error) = cat_gui::run() {
        eprintln!("Cat failed to start: {error}");
        std::process::exit(1);
    }
}
