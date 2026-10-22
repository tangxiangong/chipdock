use std::path::PathBuf;
use xtask::{Platform, Result, UpdateKeys, download_release, prepare};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args
        .next()
        .ok_or("Expected publish-cos or verify-release")?;
    let mut tag = None;
    let mut platform = Platform::All;
    let mut directory = None;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--tag" => tag = Some(args.next().ok_or("Missing --tag value")?),
            "--platform" => platform = args.next().ok_or("Missing --platform value")?.parse()?,
            "--directory" => {
                directory = Some(PathBuf::from(args.next().ok_or("Missing directory")?))
            }
            _ => return Err(format!("Unknown argument: {argument}").into()),
        }
    }
    let tag = tag.ok_or("--tag is required")?;
    xtask::version(&tag)?;
    if !matches!(
        command.as_str(),
        "publish-cos" | "verify-release" | "replace-cos-release"
    ) {
        return Err("Expected publish-cos, verify-release or replace-cos-release".into());
    }
    let work = tempfile::tempdir()?;
    let path = directory.as_deref().unwrap_or(work.path());
    let keys = UpdateKeys::from_project()?;
    download_release(&tag, platform, path)?;
    let plan = prepare(path, &tag, platform, &keys)?;
    println!(
        "Verified {} assets and {} signed feeds for {tag}",
        plan.assets.len(),
        plan.feeds.len()
    );
    match command.as_str() {
        "publish-cos" => xtask::cos::publish(&plan, path, &keys)?,
        "replace-cos-release" => xtask::cos::replace_release(&plan, path, &keys)?,
        _ => {}
    }
    Ok(())
}
