//! Shared native application operations. Hardware calls are mutually exclusive.

use crate::records::*;
use chipdock_core::{
    error::AppError,
    models::*,
    services::{self, jobs::JobEvents, storage::StoragePaths},
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard, TryLockError},
};

type Result<T> = std::result::Result<T, CoreError>;

// Native callers may construct more than one core. All handles in this process
// must observe the same USB-operation exclusion, while retaining their own events.
static HARDWARE_ACCESS: Mutex<()> = Mutex::new(());

fn try_hardware_guard(lock: &Mutex<()>) -> Result<MutexGuard<'_, ()>> {
    match lock.try_lock() {
        Ok(guard) => Ok(guard),
        Err(TryLockError::WouldBlock) => Err(AppError::JobAlreadyRunning.into()),
        // This mutex only guards exclusivity, not mutable data. A panic unwinds
        // and drops the session; poisoning must not permanently disable the app.
        Err(TryLockError::Poisoned(error)) => Ok(error.into_inner()),
    }
}

/// One client handle. Progress drains independently of blocking hardware work.
#[derive(Default)]
pub struct ChipDockCore {
    events: JobEvents,
}

impl ChipDockCore {
    fn hardware_guard(&self) -> Result<MutexGuard<'static, ()>> {
        try_hardware_guard(&HARDWARE_ACCESS)
    }
}

impl ChipDockCore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    pub fn list_probes(&self) -> Result<Vec<ProbeSummary>> {
        let _guard = self.hardware_guard()?;
        Ok(services::probe::list_probes()?)
    }

    pub fn connect_target(&self, request: ConnectRequest) -> Result<ConnectionInfo> {
        let _guard = self.hardware_guard()?;
        Ok(services::probe::connect_target(request)?)
    }

    pub fn search_chips(&self, query: String, limit: u32) -> Result<Vec<String>> {
        Ok(services::target::search_chips(&query, limit as usize)?)
    }

    pub fn target_memory_map(&self, chip: String) -> Result<Vec<MemoryRegionLayout>> {
        Ok(services::target::target_memory_map(&chip)?)
    }

    pub fn firmware_usage(&self, input: FirmwareInput) -> Result<FirmwareUsage> {
        Ok(services::firmware::analyze_firmware_usage(&input)?)
    }

    pub fn firmware_preview(&self, input: FirmwareInput, length: u32) -> Result<FirmwarePreview> {
        Ok(services::firmware::read_firmware_preview(&input, length)?)
    }

    pub fn flash_firmware(&self, request: FlashRequest) -> Result<String> {
        let _guard = self.hardware_guard()?;
        Ok(services::flash::flash_firmware(&self.events, request)?
            .0
            .to_string())
    }

    pub fn erase_target(&self, request: EraseRequest) -> Result<String> {
        let _guard = self.hardware_guard()?;
        Ok(services::memory::erase_target(&self.events, request)?
            .0
            .to_string())
    }

    pub fn read_memory(&self, request: MemoryRequest) -> Result<MemoryReadResult> {
        let _guard = self.hardware_guard()?;
        Ok(services::memory::read_memory(request)?)
    }

    pub fn take_events(&self) -> Vec<ProgressEvent> {
        self.events.drain().into_iter().map(Into::into).collect()
    }

    pub fn load_profiles(&self, directory: String) -> Result<Vec<SavedProfile>> {
        Ok(
            services::storage::load_profiles(&StoragePaths::new(PathBuf::from(directory)))?
                .into_iter()
                .map(Into::into)
                .collect(),
        )
    }

    pub fn save_profiles(&self, directory: String, profiles: Vec<SavedProfile>) -> Result<()> {
        let profiles = profiles
            .into_iter()
            .map(TryInto::try_into)
            .collect::<chipdock_core::error::Result<Vec<_>>>()?;
        Ok(services::storage::save_profiles(
            &StoragePaths::new(PathBuf::from(directory)),
            &profiles,
        )?)
    }

    pub fn load_recent_files(&self, directory: String) -> Result<Vec<RecentFirmware>> {
        Ok(
            services::storage::load_recent_files(&StoragePaths::new(PathBuf::from(directory)))?
                .into_iter()
                .map(Into::into)
                .collect(),
        )
    }

    pub fn save_recent_files(&self, directory: String, files: Vec<RecentFirmware>) -> Result<()> {
        let files = files
            .into_iter()
            .map(TryInto::try_into)
            .collect::<chipdock_core::error::Result<Vec<_>>>()?;
        Ok(services::storage::save_recent_files(
            &StoragePaths::new(PathBuf::from(directory)),
            &files,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timestamp::AppTimestamp;
    use chrono::Utc;

    #[test]
    fn separate_core_handles_should_reject_concurrent_hardware_access() {
        let _guard = HARDWARE_ACCESS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let first = ChipDockCore::new();
        let second = ChipDockCore::new();
        assert!(first.hardware_guard().is_err());
        assert!(second.hardware_guard().is_err());
        assert!(first.take_events().is_empty());
    }

    #[test]
    fn hardware_exclusion_should_recover_after_unwinding() {
        let lock = Arc::new(Mutex::new(()));
        let worker_lock = lock.clone();
        let result = std::thread::spawn(move || {
            let _guard = worker_lock.lock().unwrap();
            panic!("simulated hardware-worker unwind");
        })
        .join();
        assert!(result.is_err());
        assert!(try_hardware_guard(&lock).is_ok());
    }

    #[test]
    fn typed_api_should_search_builtin_chips_without_hardware() {
        let core = ChipDockCore::new();
        let chips = core.search_chips("STM32F103VE".into(), 10).unwrap();
        assert!(chips.iter().any(|chip| chip.starts_with("STM32F103VE")));
    }

    #[test]
    fn profile_conversion_should_reject_invalid_uuid() {
        let profile = SavedProfile {
            id: "invalid".into(),
            name: "test".into(),
            probe: None,
            target: TargetSelection {
                chip: None,
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
            bin_base_address: None,
            updated_at: Utc::now().into(),
        };
        assert!(Profile::try_from(profile).is_err());
    }

    #[test]
    fn foreign_dates_outside_chrono_range_should_return_an_error_instead_of_panicking() {
        let future = AppTimestamp::from(100_000_000_000_000.0);
        let recent = RecentFirmware {
            path: "test.bin".into(),
            format: FirmwareFormat::Bin,
            base_address: Some(0),
            used_at: future,
        };
        assert!(matches!(
            RecentFile::try_from(recent),
            Err(AppError::InvalidUserInput { .. })
        ));
    }
}
