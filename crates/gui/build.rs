fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../cat-icon.ico");
        res.set("ProductName", "Cat");
        res.set("FileDescription", "Cat — portable GitHub repository vault");
        res.set("LegalCopyright", "ThePsychof");
        if let Err(e) = res.compile() {
            eprintln!("warning: failed to embed exe icon/metadata: {e}");
        }
    }
}