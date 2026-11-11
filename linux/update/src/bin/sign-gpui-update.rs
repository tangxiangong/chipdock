use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use chipdock_gpui_update::{Asset, Feed, LINUX_FEED, PUBLIC_KEY, SignedFeed, digest, verify};
use ed25519_dalek::{Signer, SigningKey};
use std::{collections::BTreeMap, path::Path};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(
        args.len() == 3,
        "Usage: sign-gpui-update <version> <asset-directory> <notes-file>"
    );
    let version = semver::Version::parse(args[0].trim_start_matches('v'))?;
    ensure!(
        args[0] == format!("v{version}")
            && version.pre.is_empty()
            && version.build.is_empty()
            && version.to_string() == env!("CARGO_PKG_VERSION"),
        "Release tag must match the canonical stable package version"
    );
    let directory = Path::new(&args[1]);
    let secret = STANDARD.decode(
        std::env::var("SPARKLE_PRIVATE_ED_KEY")
            .context("Missing release signing key")?
            .trim(),
    )?;
    // Sparkle exports either a 32-byte seed or a seed followed by the public key.
    ensure!(
        matches!(secret.len(), 32 | 64),
        "Invalid release signing key length"
    );
    let seed: [u8; 32] = secret[..32].try_into()?;
    let key = SigningKey::from_bytes(&seed);
    ensure!(
        STANDARD.encode(key.verifying_key().as_bytes()) == PUBLIC_KEY,
        "Release signing key does not match the client public key"
    );
    let mut assets = BTreeMap::new();
    for platform in [
        "linux-x86_64",
        "linux-aarch64",
        "macos-x86_64",
        "macos-aarch64",
        "windows-x86_64",
        "windows-aarch64",
    ] {
        let name = if platform.starts_with("linux-") {
            format!("ChipDock-{version}-{platform}")
        } else {
            let extension = if platform.starts_with("windows") {
                ".exe"
            } else {
                ""
            };
            format!("chipdock-gpui-{platform}{extension}")
        };
        let path = directory.join(&name);
        if path.is_file() {
            let bytes = std::fs::read(path)?;
            assets.insert(
                platform.into(),
                Asset {
                    name,
                    size: bytes.len() as u64,
                    sha256: digest(&bytes),
                },
            );
        }
    }
    ensure!(
        assets.contains_key("linux-x86_64") && assets.contains_key("linux-aarch64"),
        "Both Linux architectures must be present before signing"
    );
    let mut packages = Vec::new();
    for arch in ["x86_64", "aarch64"] {
        for extension in ["tar.gz", "deb", "rpm"] {
            let name = format!("ChipDock-{version}-linux-{arch}.{extension}");
            let bytes = std::fs::read(directory.join(&name))
                .with_context(|| format!("Missing Linux package: {name}"))?;
            packages.push(Asset {
                name,
                size: bytes.len() as u64,
                sha256: digest(&bytes),
            });
        }
    }
    let feed = Feed {
        identity: "com.xiaoyu.chipdock.gpui".into(),
        version: version.to_string(),
        notes: std::fs::read_to_string(&args[2])?,
        assets,
        packages,
    };
    let payload = serde_json::to_vec(&feed)?;
    let signed = SignedFeed {
        payload: STANDARD.encode(&payload),
        signature: STANDARD.encode(key.sign(&payload).to_bytes()),
    };
    let bytes = serde_json::to_vec_pretty(&signed)?;
    verify(&bytes, PUBLIC_KEY)?;
    std::fs::write(directory.join(LINUX_FEED), bytes)?;
    Ok(())
}
