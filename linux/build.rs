fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=ChipDock.rc");
    println!("cargo:rerun-if-changed=../windows/ChipDock/Assets/ChipDock.ico");
    if std::env::var("CARGO_CFG_TARGET_OS")? == "windows" {
        embed_resource::compile("ChipDock.rc", embed_resource::NONE).manifest_optional()?;
    }
    Ok(())
}
