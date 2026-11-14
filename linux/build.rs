fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=ChipDock.rc");
    println!("cargo:rerun-if-changed=../windows/ChipDock/Assets/ChipDock.ico");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS")?;
    if target_os == "windows" {
        embed_resource::compile("ChipDock.rc", embed_resource::NONE).manifest_optional()?;
    }
    if target_os == "macos" {
        println!("cargo:rerun-if-changed=build-macos-assets.sh");
        println!("cargo:rerun-if-changed=../assets/app-icons/ChipDock.icon");
        println!("cargo:rerun-if-env-changed=DEVELOPER_DIR");
        let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
        let output = std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("macos-assets");
        let status = std::process::Command::new("bash")
            .arg(manifest.join("build-macos-assets.sh"))
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
