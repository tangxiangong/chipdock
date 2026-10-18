use interoptopus_csharp::RustLibrary;
use interoptopus_csharp::dispatch::Dispatch;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../windows/FlashDesk/Generated")
        });
    std::fs::create_dir_all(&output)?;
    RustLibrary::builder(flashdesk_csharp::inventory())
        .dll_name("flashdesk_csharp")
        .dispatch(Dispatch::single_file("FlashDesk.Core"))
        .build()
        .process()?
        .write_buffers_to(output)?;
    Ok(())
}
