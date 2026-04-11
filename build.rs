fn main() {
    // Allow CI to inject the release version via environment variable.
    // Local builds fall back to the version declared in Cargo.toml.
    let version = std::env::var("THURKUBE_RELEASE_VERSION")
        .map(|v| v.strip_prefix('v').unwrap_or(&v).to_owned())
        .unwrap_or_else(|_| {
            println!("cargo:rustc-cfg=dev_build");
            std::env::var("CARGO_PKG_VERSION").unwrap()
        });

    println!("cargo:rustc-env=THURKUBE_VERSION={version}");
    println!("cargo:rustc-check-cfg=cfg(dev_build)");
}
