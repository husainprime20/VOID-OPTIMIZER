// VOID — Windows resources.
//
// Stamps the executable with the VOID mark, the product name and the version
// that Explorer shows in the file properties. Compilation is best-effort: if the
// resource compiler is missing (a bare Rust toolchain without the Windows SDK)
// the build still succeeds, the exe just keeps the default icon.

fn main() {
    println!("cargo:rerun-if-changed=assets/void.ico");
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(windows)]
    {
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/void.ico");
        res.set("ProductName", "VOID");
        res.set("FileDescription", "VOID — Competitive Optimizer");
        res.set("CompanyName", "VOID");
        res.set("LegalCopyright", "MIT");
        res.set("ProductVersion", &version);
        res.set("FileVersion", &version);
        res.set("OriginalFilename", "VOID.exe");
        if res.compile().is_err() {
            println!("cargo:warning=VOID: resource compiler unavailable, exe icon skipped");
        }
    }
}
