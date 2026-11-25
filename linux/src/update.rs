//! Transport for the standard NSH updater, serialized by the workbench update lane.
use anyhow::{Context, Result, ensure};
use nsh_updater::{Config, Download, install::Installation};
use reqwest::blocking::Client;
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tempfile::NamedTempFile;

#[cfg(debug_assertions)]
#[derive(Clone, serde::Deserialize)]
pub struct Feed {
    pub version: String,
    pub notes: String,
}

#[derive(Clone)]
pub struct Available {
    pub version: String,
    pub notes: String,
    pub can_install: bool,
    pub(crate) asset: nsh_updater::Available,
    pub(crate) github: bool,
}

struct Package {
    config: Config,
    bytes: Vec<u8>,
    directory: PathBuf,
}

impl Package {
    fn current() -> Result<Option<Self>> {
        let exe = std::env::current_exe()?;
        let directory = exe.parent().context("找不到程序安装目录")?;
        let resources = if cfg!(target_os = "macos") {
            directory.join("../Resources/nsh")
        } else {
            directory.join("nsh")
        };
        let bytes = match fs::read(resources.join("assets").join(nsh_updater::CONFIG_ASSET)) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let shipped = Config::parse(&bytes)?;
        let mut shell: serde_json::Value =
            serde_json::from_str(include_str!("../shell-updater.json"))?;
        shell["id"] = shipped.id.into();
        shell["version"] = shipped.version.into();
        let config = Config::parse(&serde_json::to_vec(&shell)?)?;
        ensure!(config.public_key.is_some(), "更新清单缺少签名公钥");
        if Installation::current(&config.id, &bytes).is_none() {
            return Ok(None);
        }
        Ok(Some(Self {
            config,
            bytes,
            directory: directory.to_owned(),
        }))
    }

    fn writable(&self) -> Option<Installation> {
        if self
            .directory
            .join("package-managed")
            .try_exists()
            .unwrap_or(true)
            || (cfg!(target_os = "linux") && self.directory.starts_with("/usr"))
        {
            return None;
        }
        Installation::current(&self.config.id, &self.bytes).filter(Installation::can_install)
    }
}

fn active(cancel: &AtomicBool) -> Result<()> {
    ensure!(!cancel.load(Ordering::Relaxed), "更新已取消");
    Ok(())
}

fn client() -> Result<Client> {
    Ok(Client::builder()
        .https_only(true)
        .user_agent("ChipDock")
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .build()?)
}

fn metadata(client: &Client, url: &str, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    client
        .get(url)
        .send()?
        .error_for_status()?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "更新清单过大");
    Ok(bytes)
}

fn check(
    client: &Client,
    config: &Config,
    cancel: &AtomicBool,
) -> Result<Option<nsh_updater::Available>> {
    active(cancel)?;
    let bytes = metadata(client, &config.manifest_url(), 1024 * 1024)?;
    active(cancel)?;
    let signature = metadata(
        client,
        &config.signature_url().context("缺少更新签名")?,
        1024,
    )?;
    active(cancel)?;
    Ok(config.check(&bytes, Some(&signature), &nsh_updater::platform())?)
}

fn file_name(asset: &nsh_updater::Available) -> Result<String> {
    let url = url::Url::parse(&asset.url)?;
    let name = url
        .path_segments()
        .and_then(|mut parts| parts.next_back())
        .filter(|s| !s.is_empty())
        .context("更新归档缺少文件名")?;
    Ok(name.to_owned())
}

fn copy_verified(
    asset: &nsh_updater::Available,
    mut input: impl Read,
    mut out: impl Write,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<()> {
    let size = asset.size.context("更新清单缺少归档长度")?;
    let mut verify = Download::new(
        asset.sha256.as_deref().context("更新清单缺少归档摘要")?,
        Some(size),
    );
    let mut buf = [0u8; 65536];
    loop {
        active(cancel)?;
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let done = verify.update(&buf[..n])?;
        out.write_all(&buf[..n])?;
        progress(done, size);
    }
    verify.finish()?;
    active(cancel)
}

#[derive(Default)]
pub(crate) struct UpdateLane {
    archive: Option<(NamedTempFile, nsh_updater::Available)>,
}

impl UpdateLane {
    pub fn check(&mut self, cancel: &AtomicBool) -> Result<Option<Available>> {
        let Some(package) = Package::current()? else {
            return Ok(None);
        };
        if let Some(installation) = package.writable() {
            installation.remove_leftovers();
        }
        let client = client()?;
        let (selected, github) = match check(&client, &package.config, cancel) {
            Ok(result) => (result, false),
            Err(_) => (
                check(&client, &package.config.channel("github")?, cancel)?,
                true,
            ),
        };
        active(cancel)?;
        Ok(selected.map(|asset| Available {
            version: asset.version.clone(),
            notes: asset.notes.clone().unwrap_or_default(),
            can_install: package.writable().is_some(),
            github,
            asset,
        }))
    }

    pub fn download(
        &mut self,
        update: &Available,
        cancel: &AtomicBool,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<()> {
        let package = Package::current()?.context("当前不是可更新的应用安装包")?;
        package.writable().context("请使用包管理器更新此安装")?;
        let asset = &update.asset;
        self.archive = None;
        let client = client()?;
        let mut fetch = |candidate: &nsh_updater::Available| -> Result<NamedTempFile> {
            active(cancel)?;
            let response = client.get(&candidate.url).send()?.error_for_status()?;
            let mut tmp = NamedTempFile::new()?;
            copy_verified(asset, response, &mut tmp, cancel, &mut progress)?;
            Ok(tmp)
        };
        let tmp = match fetch(asset) {
            Ok(tmp) => tmp,
            Err(_) if !update.github => {
                let alternate = check(&client, &package.config.channel("github")?, cancel)?
                    .context("备用源没有所选更新")?;
                ensure!(
                    asset.version == alternate.version
                        && asset.size == alternate.size
                        && asset.sha256 == alternate.sha256
                        && file_name(asset)? == file_name(&alternate)?,
                    "备用源归档与已选更新不一致"
                );
                fetch(&alternate)?
            }
            Err(err) => return Err(err),
        };
        active(cancel)?;
        self.archive = Some((tmp, asset.clone()));
        Ok(())
    }

    pub fn install(&mut self, cancel: &AtomicBool) -> Result<()> {
        let package = Package::current()?.context("找不到 NSH 安装包")?;
        let installation = package.writable().context("请使用包管理器更新此安装")?;
        let (archive, selected) = self.archive.as_ref().context("尚未下载更新")?;
        copy_verified(
            selected,
            archive.reopen()?,
            std::io::sink(),
            cancel,
            |_, _| {},
        )?;
        installation.install(archive.path())?;
        self.archive = None;
        Ok(())
    }
}
