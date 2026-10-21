use interoptopus_csharp::RustLibrary;
use interoptopus_csharp::dispatch::Dispatch;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../windows/ChipDock/Generated")
        });
    std::fs::create_dir_all(&output)?;
    RustLibrary::builder(chipdock_csharp::inventory())
        .dll_name("chipdock_csharp")
        .dispatch(Dispatch::single_file("ChipDock.Core"))
        .build()
        .process()?
        .write_buffers_to(&output)?;
    // Keep generated source stable and clean across native builds.
    let generated = output.join("Interop.cs");
    let source = std::fs::read_to_string(&generated)?;
    let clean = source
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(generated, format!("{}\n", clean.trim_end()))?;
    Ok(())
}
