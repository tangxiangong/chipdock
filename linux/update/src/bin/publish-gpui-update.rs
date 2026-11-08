use anyhow::{Context, Result, ensure};
use chipdock_gpui_update::{PUBLIC_KEY, digest, verify};
use serde_json::Value;
use std::{path::Path, process::Command};

fn gh(args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("gh").args(args).output()?;
    ensure!(
        output.status.success(),
        "GitHub request failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(
        args.len() == 2,
        "Usage: publish-gpui-update <tag> <directory>"
    );
    let tag = &args[0];
    let directory = Path::new(&args[1]);
    let repo = std::env::var("GITHUB_REPOSITORY").context("Missing GITHUB_REPOSITORY")?;
    ensure!(
        repo == "tangxiangong/chipdock",
        "Unexpected release repository"
    );
    let feed_path = directory.join("gpui-stable.signed.json");
    let bytes = std::fs::read(&feed_path)?;
    let feed = verify(&bytes, PUBLIC_KEY)?;
    ensure!(
        tag == &format!("v{}", feed.version) && feed.version == env!("CARGO_PKG_VERSION"),
        "Release tag does not match package version"
    );
    let published: Value =
        serde_json::from_slice(&gh(&["api", &format!("repos/{repo}/releases/tags/{tag}")])?)?;
    ensure!(
        published["draft"] == false && published["prerelease"] == false,
        "Expected a published stable release"
    );
    let existing = published["assets"].as_array().context("Missing assets")?;
    let mut files: Vec<_> = feed.assets.values().map(|a| a.name.as_str()).collect();
    files.push("gpui-stable.signed.json");
    for name in files {
        let path = directory.join(name);
        let data = std::fs::read(&path)?;
        if let Some(asset) = feed.assets.values().find(|a| a.name == name) {
            ensure!(
                asset.size == data.len() as u64 && asset.sha256 == digest(&data),
                "Local asset differs from signed feed"
            );
        }
        if let Some(asset) = existing.iter().find(|a| a["name"] == name) {
            ensure!(
                asset["size"].as_u64() == Some(data.len() as u64)
                    && asset["digest"] == format!("sha256:{}", digest(&data)),
                "Immutable release asset differs: {name}"
            );
        } else {
            gh(&[
                "release",
                "upload",
                tag,
                path.to_str().context("Invalid asset path")?,
                "--repo",
                &repo,
            ])?;
        }
    }
    // The shared chipdock-updates release already belongs to the native clients.
    // List first so network/authentication failures never look like a missing feed.
    let releases: Value = serde_json::from_slice(&gh(&[
        "api",
        &format!("repos/{repo}/releases?per_page=100"),
    ])?)?;
    if let Some(current) = releases
        .as_array()
        .context("Invalid release list")?
        .iter()
        .find(|r| r["tag_name"] == "chipdock-updates")
    {
        if current["assets"]
            .as_array()
            .is_some_and(|a| a.iter().any(|a| a["name"] == "gpui-stable.signed.json"))
        {
            let temp = tempfile::tempdir()?;
            gh(&[
                "release",
                "download",
                "chipdock-updates",
                "--repo",
                &repo,
                "--pattern",
                "gpui-stable.signed.json",
                "--dir",
                temp.path().to_str().context("Invalid temp directory")?,
            ])?;
            let old_bytes = std::fs::read(temp.path().join("gpui-stable.signed.json"))?;
            let old = verify(&old_bytes, PUBLIC_KEY)?;
            let order =
                semver::Version::parse(&feed.version)?.cmp(&semver::Version::parse(&old.version)?);
            if order.is_lt() {
                println!("Retained newer GPUI feed");
                return Ok(());
            }
            ensure!(
                !order.is_eq() || old_bytes == bytes,
                "Conflicting feed at the same version"
            );
        }
    } else {
        gh(&[
            "release",
            "create",
            "chipdock-updates",
            "--repo",
            &repo,
            "--target",
            &std::env::var("GITHUB_SHA")?,
            "--title",
            "ChipDock Updates",
            "--notes",
            "Signed update feeds",
            "--prerelease",
        ])?;
    }
    gh(&[
        "release",
        "upload",
        "chipdock-updates",
        feed_path.to_str().context("Invalid feed path")?,
        "--repo",
        &repo,
        "--clobber",
    ])?;
    Ok(())
}
