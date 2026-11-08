use super::*;

pub(crate) fn verify_asset(bytes: &[u8], asset: &Asset) -> Result<()> {
    ensure!(bytes.len() as u64 == asset.size, "更新文件长度不匹配");
    ensure!(
        digest(bytes) == asset.sha256.to_lowercase(),
        "更新文件摘要不匹配"
    );
    Ok(())
}

pub fn install(path: &Path, update: &Available) -> Result<()> {
    verify_asset(&std::fs::read(path)?, &update.asset)?;
    let current = std::env::current_exe()?;
    #[cfg(not(windows))]
    {
        replace_portable(&current, path, |current| {
            std::process::Command::new(current)
                .arg("--after-update")
                .spawn()
                .map(|_| ())
                .map_err(Into::into)
        })?;
    }
    #[cfg(windows)]
    {
        // Run a copy of the verified new executable after the old process releases its image.
        let helper = current.with_extension("updater.exe");
        std::fs::copy(path, &helper)?;
        std::process::Command::new(helper)
            .arg("--finish-update")
            .arg(&current)
            .arg(path)
            .spawn()?;
    }
    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn replace_portable(
    current: &Path,
    pending: &Path,
    launch: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    let parent = current.parent().context("无效的安装路径")?;
    let backup = tempfile::NamedTempFile::new_in(parent)?;
    std::fs::copy(current, backup.path())?;
    std::fs::rename(pending, current)?;
    if let Err(error) = launch(current) {
        backup.persist(current)?;
        return Err(error);
    }
    Ok(())
}

pub(crate) fn valid_windows_target(target: &Path, helper: &Path) -> bool {
    target.parent() == helper.parent()
        && helper == target.with_extension("updater.exe")
        && target
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("chipdock-gpui") && n.ends_with(".exe"))
}

pub fn finish_update(arguments: &[String]) -> Result<bool> {
    if arguments.first().map(String::as_str) != Some("--finish-update") {
        return Ok(false);
    }
    ensure!(cfg!(windows) && arguments.len() == 3, "无效的更新辅助命令");
    let target = Path::new(&arguments[1]);
    let pending = Path::new(&arguments[2]);
    let helper = std::env::current_exe()?;
    ensure!(
        target.parent() == helper.parent() && pending.parent() == helper.parent(),
        "更新目标目录不匹配"
    );
    ensure!(valid_windows_target(target, &helper), "更新目标文件无效");
    ensure!(
        Sha256::digest(std::fs::read(&helper)?) == Sha256::digest(std::fs::read(pending)?),
        "更新文件已改变"
    );
    for _ in 0..120 {
        let backup = target.with_extension(format!("previous-{}", std::process::id()));
        if std::fs::rename(target, &backup).is_ok() {
            if let Err(e) = std::fs::rename(pending, target) {
                std::fs::rename(&backup, target)?;
                return Err(e.into());
            }
            if let Err(e) = std::process::Command::new(target)
                .arg("--after-update")
                .spawn()
            {
                std::fs::remove_file(target)?;
                std::fs::rename(&backup, target)?;
                return Err(e.into());
            }
            let _ = std::fs::remove_file(backup);
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    bail!("应用仍在运行，无法替换更新")
}
