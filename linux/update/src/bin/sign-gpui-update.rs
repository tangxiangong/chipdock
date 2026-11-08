use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use chipdock_gpui_update::{Asset, Feed, PUBLIC_KEY, SignedFeed, digest, verify};
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
        version.pre.is_empty(),
        "Only stable releases can advance the GPUI feed"
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
        let name = format!(
            "chipdock-gpui-{platform}{}",
            if platform.starts_with("windows") {
                ".exe"
            } else {
                ""
            }
        );
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
    let feed = Feed {
        identity: "com.xiaoyu.chipdock.gpui".into(),
        version: version.to_string(),
        notes: std::fs::read_to_string(&args[2])?,
        assets,
    };
    let payload = serde_json::to_vec(&feed)?;
    let signed = SignedFeed {
        payload: STANDARD.encode(&payload),
        signature: STANDARD.encode(key.sign(&payload).to_bytes()),
    };
    let bytes = serde_json::to_vec_pretty(&signed)?;
    verify(&bytes, PUBLIC_KEY)?;
    std::fs::write(directory.join("gpui-stable.signed.json"), bytes)?;
    Ok(())
}
