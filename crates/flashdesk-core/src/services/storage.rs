use crate::{
    error::{AppError, Result},
    models::{Profile, RecentFile},
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// 应用本地存储文件路径集合。
#[derive(Debug, Clone)]
pub struct StoragePaths {
    root: PathBuf,
}

impl StoragePaths {
    /// 使用指定根目录创建存储路径集合。
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// 用户烧录配置 JSON 文件路径。
    pub fn profiles(&self) -> PathBuf {
        self.root.join("profiles.json")
    }

    /// 最近文件 JSON 文件路径。
    pub fn recent_files(&self) -> PathBuf {
        self.root.join("recent-files.json")
    }
}

/// 确保存储根目录存在。
pub fn ensure_storage(paths: &StoragePaths) -> Result<()> {
    fs::create_dir_all(&paths.root).map_err(|err| storage_io_error("create storage root", err))?;
    Ok(())
}

/// 读取用户保存的烧录配置列表。
pub fn load_profiles(paths: &StoragePaths) -> Result<Vec<Profile>> {
    read_json_array(&paths.profiles())
}

/// 覆盖写入用户保存的烧录配置列表。
pub fn save_profiles(paths: &StoragePaths, profiles: &[Profile]) -> Result<()> {
    write_json_pretty(&paths.profiles(), profiles)
}

/// 读取最近使用的固件文件列表。
pub fn load_recent_files(paths: &StoragePaths) -> Result<Vec<RecentFile>> {
    read_json_array(&paths.recent_files())
}

/// 覆盖写入最近使用的固件文件列表。
pub fn save_recent_files(paths: &StoragePaths, recent_files: &[RecentFile]) -> Result<()> {
    write_json_pretty(&paths.recent_files(), recent_files)
}

fn read_json_array<T>(path: &Path) -> Result<Vec<T>>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(storage_io_error("read json array", error)),
    };
    serde_json::from_str(&content).map_err(|err| AppError::StorageFailure {
        detail: err.to_string(),
    })
}

fn write_json_pretty<T>(path: &Path, value: &T) -> Result<()>
where
    T: serde::Serialize + ?Sized,
{
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|err| storage_io_error("create json parent dir", err))?;
    let content = serde_json::to_vec_pretty(value).map_err(|err| AppError::StorageFailure {
        detail: err.to_string(),
    })?;
    // A temporary file in the same directory makes the rename atomic. Existing
    // readers retain a complete old snapshot, and interrupted writes cannot
    // truncate the user's profiles or recent-file history.
    let mut temporary = tempfile::Builder::new()
        .prefix(".flashdesk-")
        .tempfile_in(parent)
        .map_err(|err| storage_io_error("create temporary json file", err))?;
    temporary
        .write_all(&content)
        .map_err(|err| storage_io_error("write temporary json file", err))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|err| storage_io_error("sync json file", err))?;
    // Clear the Windows temporary attribute, then restore automatic cleanup
    // before renaming. Unlike tempfile::persist, std::fs::rename supports
    // replacing a Windows destination while readers retain its old snapshot.
    let temporary = temporary
        .into_temp_path()
        .keep()
        .map_err(|err| storage_io_error("replace json file", err.error))?;
    let mut temporary = tempfile::TempPath::try_from_path(temporary)
        .map_err(|err| storage_io_error("track temporary json file", err))?;
    fs::rename(&temporary, path).map_err(|err| storage_io_error("replace json file", err))?;
    temporary.disable_cleanup(true);
    Ok(())
}

fn storage_io_error(operation: &str, err: std::io::Error) -> AppError {
    AppError::StorageFailure {
        detail: format!("{operation}: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{FirmwareFormat, FlashOptions, TargetSelection, WireProtocol};
    use chrono::Utc;
    use std::fs;
    use uuid::Uuid;

    fn paths() -> StoragePaths {
        StoragePaths::new(
            std::env::temp_dir().join(format!("flashdesk-storage-test-{}", Uuid::new_v4())),
        )
    }

    fn profile() -> Profile {
        Profile {
            id: Uuid::new_v4(),
            name: "STM32 default".to_string(),
            probe: None,
            target: TargetSelection {
                chip: Some("STM32F103C8".to_string()),
                protocol: WireProtocol::Swd,
                speed_khz: Some(4000),
                connect_under_reset: false,
            },
            flash_options: FlashOptions {
                verify: true,
                dry_run: false,
                skip_erase: false,
                allow_erase_all: false,
                reset_after: true,
            },
            bin_base_address: Some(0x0800_0000),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn save_profiles_should_round_trip_profiles_json() {
        let paths = paths();
        let expected = vec![profile()];
        save_profiles(&paths, &expected).unwrap();
        let actual = load_profiles(&paths).unwrap();
        assert_eq!(actual[0].name, expected[0].name);
    }

    #[test]
    fn load_recent_files_should_return_empty_when_file_missing() {
        let paths = paths();
        let actual = load_recent_files(&paths).unwrap();
        assert!(actual.is_empty());
    }

    #[test]
    fn save_recent_files_should_round_trip_recent_files_json() {
        let paths = paths();
        let expected = vec![RecentFile {
            path: "/tmp/app.bin".to_string(),
            format: FirmwareFormat::Bin,
            base_address: Some(0),
            used_at: Utc::now(),
        }];
        save_recent_files(&paths, &expected).unwrap();
        let actual = load_recent_files(&paths).unwrap();
        assert_eq!(actual[0].path, expected[0].path);
    }

    #[test]
    fn load_profiles_should_return_storage_failure_for_invalid_json() {
        let paths = paths();
        fs::create_dir_all(paths.profiles().parent().unwrap()).unwrap();
        fs::write(paths.profiles(), "{invalid json").unwrap();
        let err = load_profiles(&paths).unwrap_err();
        assert!(matches!(err, AppError::StorageFailure { .. }));
    }

    #[test]
    fn ensure_storage_should_return_storage_failure_when_root_is_file() {
        let paths = StoragePaths::new(
            std::env::temp_dir().join(format!("flashdesk-storage-file-root-{}", Uuid::new_v4())),
        );
        fs::write(&paths.root, "not a directory").unwrap();
        let err = ensure_storage(&paths).unwrap_err();
        assert!(matches!(err, AppError::StorageFailure { .. }));
    }

    #[test]
    fn saving_profiles_should_preserve_an_existing_readers_complete_snapshot() {
        use std::io::Read;
        let directory = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(directory.path().to_path_buf());
        save_profiles(&paths, &[]).unwrap();
        let mut reader = fs::File::open(paths.profiles()).unwrap();
        save_profiles(&paths, &[profile()]).unwrap();
        let mut old_snapshot = String::new();
        reader.read_to_string(&mut old_snapshot).unwrap();
        assert_eq!(old_snapshot, "[]");
        assert_eq!(load_profiles(&paths).unwrap().len(), 1);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn saved_profiles_should_not_have_the_windows_temporary_attribute() {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_TEMPORARY: u32 = 0x100;
        let directory = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(directory.path().to_path_buf());
        save_profiles(&paths, &[profile()]).unwrap();
        let attributes = fs::metadata(paths.profiles()).unwrap().file_attributes();
        assert_eq!(attributes & FILE_ATTRIBUTE_TEMPORARY, 0);
    }

    #[test]
    fn failed_atomic_replacement_should_clean_up_temporary_files() {
        let directory = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(directory.path().to_path_buf());
        fs::create_dir(paths.profiles()).unwrap();
        let error = save_profiles(&paths, &[profile()]).unwrap_err();
        assert!(matches!(error, AppError::StorageFailure { .. }));
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn storage_errors_should_retain_the_cause_at_the_uniffi_boundary() {
        let error = AppError::StorageFailure {
            detail: "replace json file: permission denied".into(),
        };
        assert_eq!(
            error.to_response().detail.as_deref(),
            Some("replace json file: permission denied")
        );
    }
}
