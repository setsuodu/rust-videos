fn main() {
    let ver = std::env::var("APP_VERSION")
        .or_else(|_| std::env::var("CARGO_PKG_VERSION"))
        .unwrap_or_else(|_| "0.0.0-dev".into());
    println!("cargo:rustc-env=APP_VERSION={ver}");
    println!("cargo:rerun-if-env-changed=APP_VERSION");
    println!("cargo:rerun-if-changed=icon.ico");
    println!("cargo:rerun-if-changed=src/index.html");

    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icon.ico");
        res.set("ProductName", "暴风影音");
        res.set("FileDescription", "暴风影音");
        res.set("FileVersion", &ver);
        res.set("ProductVersion", &ver);
        if let Err(e) = res.compile() {
            println!("cargo:warning=embed icon failed: {e}");
        }
    }
}
