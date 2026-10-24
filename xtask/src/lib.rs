//! ChipDock release verification and COS mirroring, following BibCiTeX's Rust xtask.
pub mod cos;
mod xml;

use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, VerifyingKey};
use p256::{ecdsa::signature::Verifier, pkcs8::DecodePublicKey};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};
use xml::parse_xml;

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
pub const REPOSITORY: &str = "tangxiangong/chipdock";
const SPARKLE: &str = "http://www.andymatuschak.org/xml-namespaces/sparkle";

fn ensure(value: bool, message: &str) -> Result<()> {
    if value { Ok(()) } else { Err(message.into()) }
}

/// The update verification keys already embedded in the native clients.
pub struct UpdateKeys {
    pub sparkle: String,
    pub windows: String,
}

impl UpdateKeys {
    pub fn from_project() -> Result<Self> {
        let sparkle = include_str!("../../macos/ChipDock.xcodeproj/project.pbxproj")
            .lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix("SPARKLE_PUBLIC_ED_KEY = ")
                    .map(|key| key.trim_end_matches(';').trim_matches('"').to_owned())
            })
            .ok_or("Missing Sparkle public key")?;
        let windows = parse_xml(include_str!("../../windows/ChipDock/ChipDock.csproj"))?
            .children
            .iter()
            .flat_map(|group| &group.children)
            .find(|node| node.name == "UpdatePublicKey")
            .ok_or("Missing Windows public key")?
            .text
            .trim()
            .to_owned();
        Ok(Self { sparkle, windows })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    All,
    Macos,
    Windows,
}

impl std::str::FromStr for Platform {
    type Err = Box<dyn Error + Send + Sync>;
    fn from_str(value: &str) -> Result<Self> {
        match value {
            "all" => Ok(Self::All),
            "macos" => Ok(Self::Macos),
            "windows" => Ok(Self::Windows),
            _ => Err("Expected all, macos or windows".into()),
        }
    }
}

pub fn version(tag: &str) -> Result<Version> {
    let value = tag
        .strip_prefix('v')
        .ok_or("Release tag must start with v")?;
    let parsed = Version::parse(value)?;
    ensure(
        parsed.pre.is_empty() && parsed.build.is_empty() && parsed.to_string() == value,
        "Only canonical stable release tags are supported",
    )?;
    Ok(parsed)
}

fn hash(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn safe_file(directory: &Path, name: &str) -> Result<PathBuf> {
    ensure(
        !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
            && !matches!(name, "." | ".."),
        "Unsafe release filename",
    )?;
    let path = directory.join(name);
    ensure(
        path.is_file() && !path.is_symlink(),
        "Missing or unsafe release file",
    )?;
    Ok(path)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    build: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
}

impl Identity {
    fn order(&self) -> Result<(Version, u64)> {
        Ok((
            version(&format!("v{}", self.version))?,
            self.build.unwrap_or(0),
        ))
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Channels {
    schema: u32,
    feeds: BTreeMap<String, Identity>,
}
impl Default for Channels {
    fn default() -> Self {
        Self {
            schema: 1,
            feeds: BTreeMap::new(),
        }
    }
}

fn windows_identity(data: &[u8], arch: &str, key: &str) -> Result<Identity> {
    let envelope: Value = serde_json::from_slice(data)?;
    let feed = STANDARD.decode(envelope["feed"].as_str().ok_or("Missing signed feed")?)?;
    let signature = p256::ecdsa::Signature::from_slice(
        &STANDARD.decode(
            envelope["signature"]
                .as_str()
                .ok_or("Missing Windows signature")?,
        )?,
    )?;
    let key = p256::ecdsa::VerifyingKey::from_public_key_der(&STANDARD.decode(key.trim())?)?;
    key.verify(&feed, &signature)
        .map_err(|_| "Invalid Windows update signature")?;
    let feed: Value = serde_json::from_slice(&feed)?;
    let assets = feed["Assets"].as_array().ok_or("Invalid Velopack feed")?;
    ensure(
        assets.len() == 1,
        "Expected exactly one full Windows package",
    )?;
    let asset = &assets[0];
    let value = asset["Version"].as_str().ok_or("Missing Windows version")?;
    version(&format!("v{value}"))?;
    let filename = format!("ChipDock-{value}-win-{arch}-full.nupkg");
    ensure(
        asset["PackageId"] == "ChipDock"
            && (asset["Type"] == "Full" || asset["Type"] == 1)
            && asset["FileName"] == filename,
        "Unexpected Windows app, architecture or package",
    )?;
    let size = asset["Size"]
        .as_u64()
        .filter(|size| *size > 0)
        .ok_or("Invalid Windows package size")?;
    let checksum = asset["SHA256"].as_str().ok_or("Missing Windows SHA-256")?;
    ensure(
        checksum.len() == 64 && checksum.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Invalid Windows SHA-256",
    )?;
    Ok(Identity {
        version: value.into(),
        build: None,
        signature: None,
        file: Some(filename),
        size,
        sha256: Some(checksum.to_ascii_lowercase()),
    })
}

fn appcast_identity(data: &[u8]) -> Result<(Identity, String)> {
    let root = parse_xml(std::str::from_utf8(data)?)?;
    ensure(
        root.namespace.is_empty() && root.name == "rss",
        "Expected Sparkle RSS",
    )?;
    let item = root.one("", "channel")?.one("", "item")?;
    let value = item
        .one(SPARKLE, "shortVersionString")?
        .text
        .trim()
        .to_owned();
    version(&format!("v{value}"))?;
    let build = item.one(SPARKLE, "version")?.text.parse()?;
    let enclosure = item.one("", "enclosure")?;
    let signature = enclosure.attr(SPARKLE, "edSignature").to_owned();
    Signature::from_slice(&STANDARD.decode(&signature)?)?;
    let size = enclosure.attr("", "length").parse()?;
    ensure(size > 0, "Invalid Sparkle archive size")?;
    Ok((
        Identity {
            version: value,
            build: Some(build),
            signature: Some(signature),
            file: None,
            size,
            sha256: None,
        },
        enclosure.attr("", "url").into(),
    ))
}

fn identity(name: &str, data: &[u8], keys: &UpdateKeys) -> Result<Identity> {
    match name {
        "appcast.xml" => Ok(appcast_identity(data)?.0),
        "releases.win-x64.signed.json" => windows_identity(data, "x64", &keys.windows),
        "releases.win-arm64.signed.json" => windows_identity(data, "arm64", &keys.windows),
        _ => Err("Unknown update feed".into()),
    }
}

/// Fully verified immutable payloads and the corresponding client-facing feeds.
#[derive(Default)]
pub struct Plan {
    pub assets: BTreeMap<String, PathBuf>,
    pub feeds: BTreeMap<String, PathBuf>,
    pub(crate) identities: BTreeMap<String, Identity>,
}

/// Validate all signed payloads before any COS network mutation.
pub fn prepare(directory: &Path, tag: &str, platform: Platform, keys: &UpdateKeys) -> Result<Plan> {
    let value = version(tag)?.to_string();
    let mut plan = Plan::default();
    if matches!(platform, Platform::Macos | Platform::All) {
        let name = format!("ChipDock-{value}-macOS-universal.zip");
        let archive = safe_file(directory, &name)?;
        let appcast = safe_file(directory, "appcast.xml")?;
        let original = fs::read(&appcast)?;
        let (identity, url) = appcast_identity(&original)?;
        ensure(identity.version == value, "Sparkle version mismatch")?;
        let expected = format!("https://github.com/{REPOSITORY}/releases/download/{tag}/{name}");
        ensure(url == expected, "Unexpected Sparkle archive URL")?;
        ensure(
            identity.size == fs::metadata(&archive)?.len(),
            "Sparkle archive size mismatch",
        )?;
        let key: [u8; 32] = STANDARD
            .decode(keys.sparkle.trim())?
            .try_into()
            .map_err(|_| "Invalid Sparkle public key")?;
        let signature = Signature::from_slice(
            &STANDARD.decode(
                identity
                    .signature
                    .as_ref()
                    .ok_or("Missing Sparkle signature")?,
            )?,
        )?;
        VerifyingKey::from_bytes(&key)?.verify_strict(&fs::read(&archive)?, &signature)?;
        let feed = directory.join("cos-appcast.xml");
        // Match BibCiTeX: retain exact XML/notes and rewrite only the verified archive URL.
        fs::write(
            &feed,
            std::str::from_utf8(&original)?.replace(
                &expected,
                &format!("{}/releases/{tag}/{name}", cos::PUBLIC_ROOT),
            ),
        )?;
        plan.assets
            .insert(format!("releases/{tag}/{name}"), archive);
        plan.assets
            .insert(format!("releases/{tag}/appcast.xml"), appcast);
        plan.feeds.insert("appcast.xml".into(), feed);
        plan.identities.insert("appcast.xml".into(), identity);
    }
    if matches!(platform, Platform::Windows | Platform::All) {
        for arch in ["x64", "arm64"] {
            let name = format!("releases.win-{arch}.signed.json");
            let signed = safe_file(directory, &name)?;
            let identity = windows_identity(&fs::read(&signed)?, arch, &keys.windows)?;
            ensure(identity.version == value, "Windows version mismatch")?;
            let filename = identity.file.as_deref().ok_or("Missing Windows filename")?;
            let package = safe_file(directory, filename)?;
            ensure(
                identity.size == fs::metadata(&package)?.len()
                    && identity.sha256.as_deref() == Some(&hash(&package)?),
                "Windows package hash or size mismatch",
            )?;
            plan.assets
                .insert(format!("releases/{tag}/{filename}"), package);
            for extension in ["exe", "msi"] {
                let filename = format!("ChipDock-{value}-windows-{arch}.{extension}");
                plan.assets.insert(
                    format!("releases/{tag}/{filename}"),
                    safe_file(directory, &filename)?,
                );
            }
            plan.assets
                .insert(format!("releases/{tag}/{name}"), signed.clone());
            plan.feeds.insert(name.clone(), signed);
            plan.identities.insert(name, identity);
        }
    }
    Ok(plan)
}

fn github(arguments: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("gh").args(arguments).output()?;
    ensure(
        output.status.success(),
        &format!(
            "GitHub command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    Ok(output.stdout)
}

/// Download immutable GitHub assets and verify the server's digest before parsing them.
pub fn download_release(tag: &str, platform: Platform, directory: &Path) -> Result<()> {
    let value = version(tag)?.to_string();
    fs::create_dir_all(directory)?;
    let release: Value = serde_json::from_slice(&github(&[
        "api",
        &format!("repos/{REPOSITORY}/releases/tags/{tag}"),
    ])?)?;
    ensure(
        release["draft"] == false && release["prerelease"] == false,
        "Expected a published stable GitHub release",
    )?;
    let assets = release["assets"]
        .as_array()
        .ok_or("Missing GitHub asset list")?;
    let mut names = Vec::new();
    if matches!(platform, Platform::Macos | Platform::All) {
        names.extend([
            format!("ChipDock-{value}-macOS-universal.zip"),
            "appcast.xml".into(),
        ]);
    }
    if matches!(platform, Platform::Windows | Platform::All) {
        for arch in ["x64", "arm64"] {
            names.extend([
                format!("releases.win-{arch}.signed.json"),
                format!("ChipDock-{value}-win-{arch}-full.nupkg"),
                format!("ChipDock-{value}-windows-{arch}.exe"),
                format!("ChipDock-{value}-windows-{arch}.msi"),
            ]);
        }
    }
    for name in names {
        let info = assets
            .iter()
            .find(|asset| asset["name"] == name)
            .ok_or("Missing versioned GitHub payload")?;
        let target = directory.join(&name);
        if !target.try_exists()? {
            github(&[
                "release",
                "download",
                tag,
                "--repo",
                REPOSITORY,
                "--pattern",
                &name,
                "--dir",
                directory.to_str().ok_or("Invalid output directory")?,
            ])?;
        }
        let path = safe_file(directory, &name)?;
        ensure(
            info["size"].as_u64() == Some(fs::metadata(&path)?.len())
                && info["digest"] == format!("sha256:{}", hash(&path)?),
            "GitHub size or SHA-256 mismatch",
        )?;
        println!("Verified GitHub asset: {name}");
    }
    Ok(())
}

#[cfg(test)]
mod tests;
