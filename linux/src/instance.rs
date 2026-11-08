//! One desktop owns the process-wide services and user-facing local IPC endpoint.
use anyhow::{Context, Result};
use std::{
    fs::{File, OpenOptions, TryLockError},
    path::{Path, PathBuf},
    time::Duration,
};

pub struct Instance {
    _lock: File,
}
impl Instance {
    pub fn acquire(wait_for_update: bool, firmware: Option<&str>) -> Result<Option<Self>> {
        let directory = crate::model::preferences_path()?
            .parent()
            .context("无效设置目录")?
            .to_owned();
        Self::acquire_at(&directory, wait_for_update, firmware)
    }
    fn acquire_at(
        directory: &Path,
        wait_for_update: bool,
        firmware: Option<&str>,
    ) -> Result<Option<Self>> {
        std::fs::create_dir_all(directory)?;
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(directory.join("instance.lock"))?;
        for attempt in 0..=120 {
            match lock.try_lock() {
                Ok(()) => return Ok(Some(Self { _lock: lock })),
                Err(TryLockError::WouldBlock) if wait_for_update && attempt < 120 => {
                    std::thread::sleep(Duration::from_millis(250))
                }
                Err(TryLockError::WouldBlock) => {
                    if let Some(firmware) = firmware {
                        let path = std::fs::canonicalize(firmware)?;
                        let mut file = tempfile::NamedTempFile::new_in(directory)?;
                        serde_json::to_writer(&mut file, &path)?;
                        file.persist(directory.join("open-firmware.json"))?;
                    }
                    return Ok(None);
                }
                Err(TryLockError::Error(error)) => return Err(error.into()),
            }
        }
        anyhow::bail!("等待旧进程退出超时")
    }
}
pub fn firmware_argument() -> Option<String> {
    std::env::args().skip(1).find(|s| {
        Path::new(s).extension().is_some_and(|e| {
            ["elf", "axf", "hex", "bin"].contains(&e.to_string_lossy().to_lowercase().as_str())
        })
    })
}
pub fn take_firmware() -> Result<Option<PathBuf>> {
    let path = crate::model::preferences_path()?.with_file_name("open-firmware.json");
    match std::fs::read(&path) {
        Ok(bytes) => {
            std::fs::remove_file(path)?;
            Ok(Some(serde_json::from_slice(&bytes)?))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_one_desktop_owns_the_lock_and_second_launch_hands_off_firmware() {
        let directory = tempfile::tempdir().unwrap();
        let firmware = directory.path().join("firmware.bin");
        std::fs::write(&firmware, b"firmware").unwrap();
        let first = Instance::acquire_at(directory.path(), false, None)
            .unwrap()
            .unwrap();
        assert!(
            Instance::acquire_at(directory.path(), false, firmware.to_str())
                .unwrap()
                .is_none()
        );
        let handoff: PathBuf = serde_json::from_slice(
            &std::fs::read(directory.path().join("open-firmware.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(handoff, firmware.canonicalize().unwrap());
        drop(first);
        assert!(
            Instance::acquire_at(directory.path(), false, None)
                .unwrap()
                .is_some()
        );
    }
}
