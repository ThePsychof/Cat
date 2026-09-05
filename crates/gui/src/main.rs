#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    if let Err(error) = cat_gui::run() {
        eprintln!("Cat failed to start: {error}");
        std::process::exit(1);
    }
}
