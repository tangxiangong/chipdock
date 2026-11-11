mod download;
mod install;
#[cfg(test)]
mod tests;
pub use download::{check, download};
pub use install::{can_self_update, finish_update, install};
// Signed portable-client updates never select SwiftUI/WinUI packages.
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

pub const PUBLIC_KEY: &str = "NgorrvaZPObmbOUEmKbbCvPbPqcD+kcw7E2NRTfxlKI=";
pub const LINUX_FEED: &str = "linux-stable.signed.json";
pub const LEGACY_FEED: &str = "gpui-stable.signed.json";
const MIRROR: &str = "https://app-release-1302963684.cos.ap-guangzhou.myqcloud.com/chipdock";
const REPOSITORY: &str = "https://github.com/tangxiangong/chipdock";
#[derive(Clone, Serialize, Deserialize)]
pub struct SignedFeed {
    pub payload: String,
    pub signature: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Feed {
    pub identity: String,
    pub version: String,
    pub notes: String,
    pub assets: BTreeMap<String, Asset>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<Asset>,
}
impl Feed {
    pub fn release_assets(&self) -> impl Iterator<Item = &Asset> {
        self.assets.values().chain(self.packages.iter())
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub sha256: String,
}
#[derive(Clone)]
pub struct Available {
    pub version: String,
    pub notes: String,
    pub asset: Asset,
}

pub fn target() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}
pub fn verify(bytes: &[u8], key: &str) -> Result<Feed> {
    let signed: SignedFeed = serde_json::from_slice(bytes)?;
    let payload = STANDARD.decode(&signed.payload)?;
    let public: [u8; 32] = STANDARD
        .decode(key)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("更新公钥长度无效"))?;
    VerifyingKey::from_bytes(&public)?.verify_strict(
        &payload,
        &Signature::from_slice(&STANDARD.decode(signed.signature)?)?,
    )?;
    let feed: Feed = serde_json::from_slice(&payload)?;
    ensure!(
        feed.identity == "com.xiaoyu.chipdock.gpui",
        "更新包不属于此 ChipDock 客户端"
    );
    let version = semver::Version::parse(&feed.version)?;
    ensure!(
        version.pre.is_empty() && version.build.is_empty() && version.to_string() == feed.version,
        "稳定频道需要规范的正式版本号"
    );
    for (target, asset) in &feed.assets {
        ensure!(
            [
                "linux-x86_64",
                "linux-aarch64",
                "macos-x86_64",
                "macos-aarch64",
                "windows-x86_64",
                "windows-aarch64"
            ]
            .contains(&target.as_str()),
            "未知更新平台"
        );
        let extension = if target.starts_with("windows") {
            ".exe"
        } else {
            ""
        };
        let legacy = asset.name == format!("chipdock-gpui-{target}{extension}");
        ensure!(
            legacy
                || (target.starts_with("linux-")
                    && asset.name == format!("ChipDock-{}-{target}", feed.version)),
            "更新资产与平台不匹配"
        );
    }
    let mut names = std::collections::BTreeSet::new();
    for package in &feed.packages {
        ensure!(
            ["x86_64", "aarch64"].iter().any(|arch| {
                feed.assets.contains_key(&format!("linux-{arch}"))
                    && ["tar.gz", "deb", "rpm"].iter().any(|extension| {
                        ["ChipDock", "chipdock-gpui"].iter().any(|product| {
                            package.name
                                == format!("{product}-{}-linux-{arch}.{extension}", feed.version)
                        })
                    })
            }),
            "安装包与版本或平台不匹配"
        );
    }
    for asset in feed.release_assets() {
        ensure!(names.insert(&asset.name), "发布文件名重复");
        ensure!(
            asset.size > 0 && asset.size <= 1024 * 1024 * 1024,
            "更新文件大小无效"
        );
        ensure!(
            asset.sha256.len() == 64 && asset.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "更新摘要无效"
        );
    }
    Ok(feed)
}

pub fn select(feed: Feed, current: &str, target: &str) -> Result<Option<Available>> {
    if semver::Version::parse(&feed.version)? <= semver::Version::parse(current)? {
        return Ok(None);
    }
    let asset = feed
        .assets
        .get(target)
        .context("此版本尚未发布当前平台的更新包")?
        .clone();
    Ok(Some(Available {
        version: feed.version,
        notes: feed.notes,
        asset,
    }))
}

pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
