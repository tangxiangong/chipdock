use super::install::verify_asset;
use super::*;

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .user_agent("ChipDock-GPUI")
        .timeout(Duration::from_secs(60))
        .build()?)
}

fn fetch(client: &reqwest::blocking::Client, url: &str, limit: u64) -> Result<Vec<u8>> {
    let mut response = client.get(url).send()?.error_for_status()?.take(limit + 1);
    let mut bytes = Vec::new();
    response.read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "更新响应超过大小限制");
    Ok(bytes)
}

pub fn check() -> Result<Option<Available>> {
    let client = client()?;
    resolve_feed(
        |fallback| {
            fetch(
                &client,
                &if fallback {
                    format!(
                        "{REPOSITORY}/releases/download/chipdock-updates/gpui-stable.signed.json"
                    )
                } else {
                    format!("{MIRROR}/updates/gpui-stable.signed.json")
                },
                1024 * 1024,
            )
        },
        PUBLIC_KEY,
        env!("CARGO_PKG_VERSION"),
        &target(),
    )
}

pub fn download(
    update: &Available,
    cancelled: &AtomicBool,
    progress: impl Fn(u64, u64),
) -> Result<PathBuf> {
    ensure!(
        super::can_self_update(),
        "请通过包管理器或完整安装包更新此安装"
    );
    let client = client()?;
    let mirror = format!(
        "{MIRROR}/releases/v{}/{}",
        update.version, update.asset.name
    );
    let github = format!(
        "{REPOSITORY}/releases/download/v{}/{}",
        update.version, update.asset.name
    );
    let load = |url: &str| -> Result<Vec<u8>> {
        ensure!(!cancelled.load(Ordering::Relaxed), "下载已取消");
        let mut response = client.get(url).send()?.error_for_status()?;
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 65536];
        loop {
            ensure!(!cancelled.load(Ordering::Relaxed), "下载已取消");
            let count = response.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            ensure!(
                bytes.len() as u64 + count as u64 <= update.asset.size,
                "下载超过签名清单中的大小"
            );
            bytes.extend_from_slice(&buffer[..count]);
            progress(bytes.len() as u64, update.asset.size);
        }
        Ok(bytes)
    };
    let bytes = download_verified(update, cancelled, |fallback| {
        if fallback {
            progress(0, update.asset.size);
        }
        load(if fallback { &github } else { &mirror })
    })?;
    let executable = std::env::current_exe()?;
    // Keep staging on the same volume so promotion is atomic on Unix.
    let parent = executable.parent().context("无法定位安装目录")?;
    let path = parent.join(format!(".chipdock-gpui-{}.pending", update.version));
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    std::io::Write::write_all(&mut file, &bytes)?;
    file.as_file().sync_all()?;
    file.persist(&path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(path)
}

pub(crate) fn resolve_feed(
    mut read: impl FnMut(bool) -> Result<Vec<u8>>,
    key: &str,
    current: &str,
    target: &str,
) -> Result<Option<Available>> {
    let mut open = |fallback| -> Result<Option<Available>> {
        select(verify(&read(fallback)?, key)?, current, target)
    };
    open(false).or_else(|_| open(true))
}
pub(crate) fn download_verified(
    update: &Available,
    cancelled: &AtomicBool,
    mut load: impl FnMut(bool) -> Result<Vec<u8>>,
) -> Result<Vec<u8>> {
    let mut verified = |fallback| -> Result<Vec<u8>> {
        ensure!(!cancelled.load(Ordering::Relaxed), "下载已取消");
        let bytes = load(fallback)?;
        ensure!(!cancelled.load(Ordering::Relaxed), "下载已取消");
        verify_asset(&bytes, &update.asset)?;
        Ok(bytes)
    };
    verified(false).or_else(|error| {
        if cancelled.load(Ordering::Relaxed) {
            Err(error)
        } else {
            verified(true)
        }
    })
}
