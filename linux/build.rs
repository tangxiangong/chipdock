fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let source = match std::fs::read_to_string(manifest.join("nsh-source-dir")) {
        Ok(path) => std::path::PathBuf::from(path.trim()),
        Err(_) => manifest,
    };
    let target_os = std::env::var("CARGO_CFG_TARGET_OS")?;
    if target_os == "macos" {
        println!(
            "cargo:rerun-if-changed={}",
            source.join("build-macos-assets.sh").display()
        );
        println!(
            "cargo:rerun-if-changed={}",
            source.join("../assets/app-icons/ChipDock.icon").display()
        );
        println!("cargo:rerun-if-env-changed=DEVELOPER_DIR");
        let output = std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("macos-assets");
        let status = std::process::Command::new("bash")
            .arg(source.join("build-macos-assets.sh"))
            .arg(&output)
            .status()?;
        if !status.success() {
            return Err(
                "Canonical macOS icon compilation failed; full Xcode 26+ is required".into(),
            );
        }
        println!(
            "cargo:rustc-env=CHIPDOCK_MACOS_ICON_PNG={}",
            output.join("ChipDock.png").display()
        );
    }
    Ok(())
}
