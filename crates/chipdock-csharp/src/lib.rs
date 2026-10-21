//! Windows binding declarations. Interoptopus owns the ABI and wire encoding;
//! hardware, firmware and storage behaviour remains in `chipdock-core`.

mod serial;

use chipdock_core::{
    api::{ChipDockCore, CoreError, ProgressEvent},
    error::{AppError, ErrorResponse},
    models::*,
    services::storage::{self, StoragePaths},
};
use chrono::{DateTime, Utc};
use interoptopus::inventory::RustInventory;
use interoptopus::lang::types::{TypeInfo, WireIO};
use interoptopus::pattern::result::panic_to_result;
use interoptopus::wire::Wire;
use interoptopus::{builtins_wire, ffi, function};
use std::{
    path::PathBuf,
    sync::{Arc, LazyLock},
};
use uuid::Uuid;

/// One process-wide client handle: the WinUI host has a single window, and
/// hardware exclusion is shared by every core handle anyway.
static CORE: LazyLock<Arc<ChipDockCore>> = LazyLock::new(ChipDockCore::new);

#[ffi]
pub enum BridgeError {
    Serialization,
}

#[ffi]
pub struct TextValue {
    pub text: String,
}

#[ffi]
pub struct ChipSearchRequest {
    pub query: String,
    pub limit: u32,
}

#[ffi]
pub struct DirectoryRequest {
    pub directory: String,
}

#[ffi]
pub struct FirmwarePreviewRequest {
    pub firmware: FirmwareInput,
    pub length: u32,
}

/// A progress event. `at_ms` is milliseconds since the Unix epoch.
#[ffi]
pub struct ProgressRecord {
    pub id: String,
    pub kind: JobKind,
    pub stage: JobStage,
    pub progress: Option<f32>,
    pub message: String,
    pub at_ms: i64,
}

/// A saved configuration. `updated_at_ms` is milliseconds since the Unix epoch.
#[ffi]
pub struct ProfileRecord {
    pub id: String,
    pub name: String,
    pub probe: Option<String>,
    pub target: TargetSelection,
    pub flash_options: FlashOptions,
    pub bin_base_address: Option<u64>,
    pub updated_at_ms: i64,
}

/// A recently opened firmware file. `used_at_ms` is milliseconds since the Unix epoch.
#[ffi]
pub struct RecentFirmwareRecord {
    pub path: String,
    pub format: FirmwareFormat,
    pub base_address: Option<u64>,
    pub used_at_ms: i64,
}

#[ffi]
pub struct SaveProfilesRequest {
    pub directory: String,
    pub profiles: Vec<ProfileRecord>,
}

#[ffi]
pub struct SaveRecentFilesRequest {
    pub directory: String,
    pub files: Vec<RecentFirmwareRecord>,
}

#[ffi]
pub struct StatusResponse {
    pub error: Option<ErrorResponse>,
}

#[ffi]
pub struct ProbesResponse {
    pub payload: Vec<ProbeSummary>,
    pub error: Option<ErrorResponse>,
}

#[ffi]
pub struct ConnectionResponse {
    pub payload: Option<ConnectionInfo>,
    pub error: Option<ErrorResponse>,
}

#[ffi]
pub struct ChipsResponse {
    pub payload: Vec<String>,
    pub error: Option<ErrorResponse>,
}

#[ffi]
pub struct MemoryMapResponse {
    pub payload: Vec<MemoryRegionLayout>,
    pub error: Option<ErrorResponse>,
}

#[ffi]
pub struct FirmwareUsageResponse {
    pub payload: Option<FirmwareUsage>,
    pub error: Option<ErrorResponse>,
}

#[ffi]
pub struct FirmwarePreviewResponse {
    pub payload: Option<FirmwarePreview>,
    pub error: Option<ErrorResponse>,
}

/// The identifier of a completed flash or erase job.
#[ffi]
pub struct JobResponse {
    pub payload: Option<String>,
    pub error: Option<ErrorResponse>,
}

#[ffi]
pub struct MemoryReadResponse {
    pub payload: Option<MemoryReadResult>,
    pub error: Option<ErrorResponse>,
}

#[ffi]
pub struct EventsResponse {
    pub payload: Vec<ProgressRecord>,
}

#[ffi]
pub struct ProfilesResponse {
    pub payload: Vec<ProfileRecord>,
    pub error: Option<ErrorResponse>,
}

#[ffi]
pub struct RecentFilesResponse {
    pub payload: Vec<RecentFirmwareRecord>,
    pub error: Option<ErrorResponse>,
}

fn invoke<T: TypeInfo + WireIO>(
    work: impl FnOnce() -> Result<T, BridgeError>,
) -> ffi::Result<Wire<T>, BridgeError> {
    // Use the library's panic boundary; no Rust unwind reaches the CLR.
    panic_to_result(|| {
        work()
            .and_then(|value| Wire::try_from(value).map_err(|_| BridgeError::Serialization))
            .into()
    })
}

fn request<T: TypeInfo + WireIO>(mut wire: Wire<T>) -> Result<T, BridgeError> {
    wire.try_unwire().map_err(|_| BridgeError::Serialization)
}

fn response(error: CoreError) -> ErrorResponse {
    match error {
        CoreError::Operation { response } => *response,
    }
}

/// Splits a core result into the `payload` / `error` pair of a wire response.
fn split<T, U: Default>(
    result: Result<T, CoreError>,
    wrap: impl FnOnce(T) -> U,
) -> (U, Option<ErrorResponse>) {
    match result {
        Ok(value) => (wrap(value), None),
        Err(error) => (U::default(), Some(response(error))),
    }
}

fn split_app<T, U: Default>(
    result: chipdock_core::error::Result<T>,
    wrap: impl FnOnce(T) -> U,
) -> (U, Option<ErrorResponse>) {
    split(result.map_err(CoreError::from), wrap)
}

fn unix_ms(time: DateTime<Utc>) -> i64 {
    time.timestamp_millis()
}

fn from_unix_ms(milliseconds: i64) -> chipdock_core::error::Result<DateTime<Utc>> {
    DateTime::from_timestamp_millis(milliseconds).ok_or_else(|| AppError::InvalidUserInput {
        detail: "时间戳超出支持范围".into(),
    })
}

/// Seconds between the Unix epoch and Foundation's 2001 reference date, which
/// `AppTimestamp` uses for the Swift host.
const REFERENCE_EPOCH_SECONDS: f64 = 978_307_200.0;

impl From<ProgressEvent> for ProgressRecord {
    fn from(event: ProgressEvent) -> Self {
        Self {
            id: event.id,
            kind: event.kind,
            stage: event.stage,
            progress: event.progress,
            message: event.message,
            at_ms: ((f64::from(event.at) + REFERENCE_EPOCH_SECONDS) * 1000.0).round() as i64,
        }
    }
}

impl From<Profile> for ProfileRecord {
    fn from(profile: Profile) -> Self {
        Self {
            id: profile.id.to_string(),
            name: profile.name,
            probe: profile.probe,
            target: profile.target,
            flash_options: profile.flash_options,
            bin_base_address: profile.bin_base_address,
            updated_at_ms: unix_ms(profile.updated_at),
        }
    }
}

impl TryFrom<ProfileRecord> for Profile {
    type Error = AppError;

    fn try_from(profile: ProfileRecord) -> chipdock_core::error::Result<Self> {
        Ok(Self {
            id: Uuid::parse_str(&profile.id).map_err(|error| AppError::InvalidUserInput {
                detail: format!("无效的配置 ID：{error}"),
            })?,
            name: profile.name,
            probe: profile.probe,
            target: profile.target,
            flash_options: profile.flash_options,
            bin_base_address: profile.bin_base_address,
            updated_at: from_unix_ms(profile.updated_at_ms)?,
        })
    }
}

impl From<RecentFile> for RecentFirmwareRecord {
    fn from(file: RecentFile) -> Self {
        Self {
            path: file.path,
            format: file.format,
            base_address: file.base_address,
            used_at_ms: unix_ms(file.used_at),
        }
    }
}

impl TryFrom<RecentFirmwareRecord> for RecentFile {
    type Error = AppError;

    fn try_from(file: RecentFirmwareRecord) -> chipdock_core::error::Result<Self> {
        Ok(Self {
            path: file.path,
            format: file.format,
            base_address: file.base_address,
            used_at: from_unix_ms(file.used_at_ms)?,
        })
    }
}

#[ffi]
pub fn version() -> ffi::Result<Wire<TextValue>, BridgeError> {
    invoke(|| {
        Ok(TextValue {
            text: CORE.version(),
        })
    })
}

#[ffi]
pub fn list_probes() -> ffi::Result<Wire<ProbesResponse>, BridgeError> {
    invoke(|| {
        let (payload, error) = split(CORE.list_probes(), |value| value);
        Ok(ProbesResponse { payload, error })
    })
}

#[ffi]
pub fn connect_target(
    input: Wire<ConnectRequest>,
) -> ffi::Result<Wire<ConnectionResponse>, BridgeError> {
    invoke(|| {
        let (payload, error) = split(CORE.connect_target(request(input)?), Some);
        Ok(ConnectionResponse { payload, error })
    })
}

#[ffi]
pub fn search_chips(
    input: Wire<ChipSearchRequest>,
) -> ffi::Result<Wire<ChipsResponse>, BridgeError> {
    invoke(|| {
        let input = request(input)?;
        let (payload, error) = split(CORE.search_chips(input.query, input.limit), |value| value);
        Ok(ChipsResponse { payload, error })
    })
}

#[ffi]
pub fn target_memory_map(
    input: Wire<TextValue>,
) -> ffi::Result<Wire<MemoryMapResponse>, BridgeError> {
    invoke(|| {
        let (payload, error) = split(CORE.target_memory_map(request(input)?.text), |value| value);
        Ok(MemoryMapResponse { payload, error })
    })
}

#[ffi]
pub fn firmware_usage(
    input: Wire<FirmwareInput>,
) -> ffi::Result<Wire<FirmwareUsageResponse>, BridgeError> {
    invoke(|| {
        let (payload, error) = split(CORE.firmware_usage(request(input)?), Some);
        Ok(FirmwareUsageResponse { payload, error })
    })
}

#[ffi]
pub fn firmware_preview(
    input: Wire<FirmwarePreviewRequest>,
) -> ffi::Result<Wire<FirmwarePreviewResponse>, BridgeError> {
    invoke(|| {
        let input = request(input)?;
        let (payload, error) = split(CORE.firmware_preview(input.firmware, input.length), Some);
        Ok(FirmwarePreviewResponse { payload, error })
    })
}

#[ffi]
pub fn flash_firmware(input: Wire<FlashRequest>) -> ffi::Result<Wire<JobResponse>, BridgeError> {
    invoke(|| {
        let (payload, error) = split(CORE.flash_firmware(request(input)?), Some);
        Ok(JobResponse { payload, error })
    })
}

#[ffi]
pub fn erase_target(input: Wire<EraseRequest>) -> ffi::Result<Wire<JobResponse>, BridgeError> {
    invoke(|| {
        let (payload, error) = split(CORE.erase_target(request(input)?), Some);
        Ok(JobResponse { payload, error })
    })
}

#[ffi]
pub fn read_memory(
    input: Wire<MemoryRequest>,
) -> ffi::Result<Wire<MemoryReadResponse>, BridgeError> {
    invoke(|| {
        let (payload, error) = split(CORE.read_memory(request(input)?), Some);
        Ok(MemoryReadResponse { payload, error })
    })
}

/// Drains progress independently of the blocking hardware call that produces it.
#[ffi]
pub fn take_events() -> ffi::Result<Wire<EventsResponse>, BridgeError> {
    invoke(|| {
        Ok(EventsResponse {
            payload: CORE.take_events().into_iter().map(Into::into).collect(),
        })
    })
}

#[ffi]
pub fn load_profiles(
    input: Wire<DirectoryRequest>,
) -> ffi::Result<Wire<ProfilesResponse>, BridgeError> {
    invoke(|| {
        let paths = StoragePaths::new(PathBuf::from(request(input)?.directory));
        let (payload, error) = split_app(storage::load_profiles(&paths), |profiles| {
            profiles.into_iter().map(Into::into).collect()
        });
        Ok(ProfilesResponse { payload, error })
    })
}

#[ffi]
pub fn save_profiles(
    input: Wire<SaveProfilesRequest>,
) -> ffi::Result<Wire<StatusResponse>, BridgeError> {
    invoke(|| {
        let input = request(input)?;
        let paths = StoragePaths::new(PathBuf::from(input.directory));
        let result = input
            .profiles
            .into_iter()
            .map(TryInto::try_into)
            .collect::<chipdock_core::error::Result<Vec<Profile>>>()
            .and_then(|profiles| storage::save_profiles(&paths, &profiles));
        let ((), error) = split_app(result, |value| value);
        Ok(StatusResponse { error })
    })
}

#[ffi]
pub fn load_recent_files(
    input: Wire<DirectoryRequest>,
) -> ffi::Result<Wire<RecentFilesResponse>, BridgeError> {
    invoke(|| {
        let paths = StoragePaths::new(PathBuf::from(request(input)?.directory));
        let (payload, error) = split_app(storage::load_recent_files(&paths), |files| {
            files.into_iter().map(Into::into).collect()
        });
        Ok(RecentFilesResponse { payload, error })
    })
}

#[ffi]
pub fn save_recent_files(
    input: Wire<SaveRecentFilesRequest>,
) -> ffi::Result<Wire<StatusResponse>, BridgeError> {
    invoke(|| {
        let input = request(input)?;
        let paths = StoragePaths::new(PathBuf::from(input.directory));
        let result = input
            .files
            .into_iter()
            .map(TryInto::try_into)
            .collect::<chipdock_core::error::Result<Vec<RecentFile>>>()
            .and_then(|files| storage::save_recent_files(&paths, &files));
        let ((), error) = split_app(result, |value| value);
        Ok(StatusResponse { error })
    })
}

pub fn inventory() -> RustInventory {
    RustInventory::new()
        .register(builtins_wire!())
        .register(function!(serial::chipdock_serial_ports))
        .register(function!(serial::chipdock_serial_poll))
        .register(function!(serial::chipdock_serial_connect))
        .register(function!(serial::chipdock_serial_disconnect))
        .register(function!(serial::chipdock_serial_send))
        .register(function!(version))
        .register(function!(list_probes))
        .register(function!(connect_target))
        .register(function!(search_chips))
        .register(function!(target_memory_map))
        .register(function!(firmware_usage))
        .register(function!(firmware_preview))
        .register(function!(flash_firmware))
        .register(function!(erase_target))
        .register(function!(read_memory))
        .register(function!(take_events))
        .register(function!(load_profiles))
        .register(function!(save_profiles))
        .register(function!(load_recent_files))
        .register(function!(save_recent_files))
        .validate()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_firmware_preview_should_preserve_addresses_and_holes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("firmware.hex");
        std::fs::write(
            &path,
            ":020000040800F2\n:0200000041427B\n:0100040043B8\n:00000001FF\n",
        )
        .unwrap();
        let input = Wire::from(FirmwarePreviewRequest {
            firmware: FirmwareInput {
                path: path.to_string_lossy().into_owned(),
                format: None,
                base_address: None,
            },
            length: 4096,
        });
        let response = firmware_preview(input).unwrap().unwire();
        assert!(response.error.is_none());
        let preview = response.payload.unwrap();
        assert_eq!(
            (
                preview.start_address,
                preview.bytes,
                preview.usage.used_bytes
            ),
            (
                0x0800_0000,
                vec![Some(0x41), Some(0x42), None, None, Some(0x43)],
                3
            )
        );
    }

    #[test]
    fn wire_chip_search_should_return_builtin_targets() {
        let input = Wire::from(ChipSearchRequest {
            query: "STM32F103VE".into(),
            limit: 10,
        });
        let response = search_chips(input).unwrap().unwire();
        assert!(response.error.is_none());
        assert!(
            response
                .payload
                .iter()
                .any(|chip| chip.starts_with("STM32F103VE"))
        );
    }

    #[test]
    fn wire_storage_should_round_trip_profiles_and_recent_files() {
        let directory = tempfile::tempdir().unwrap();
        let directory_name = directory.path().join("数据").to_string_lossy().into_owned();
        let profile = ProfileRecord {
            id: Uuid::new_v4().to_string(),
            name: "中文配置".into(),
            probe: Some("0483:3748".into()),
            target: TargetSelection {
                chip: Some("STM32F103C8".into()),
                protocol: WireProtocol::Jtag,
                speed_khz: Some(1000),
                connect_under_reset: true,
            },
            flash_options: FlashOptions {
                verify: true,
                dry_run: false,
                skip_erase: false,
                allow_erase_all: true,
                reset_after: false,
            },
            bin_base_address: Some(0x0800_0000),
            updated_at_ms: 1_760_000_000_123,
        };
        let saved = save_profiles(Wire::from(SaveProfilesRequest {
            directory: directory_name.clone(),
            profiles: vec![profile],
        }))
        .unwrap()
        .unwire();
        assert!(saved.error.is_none());
        let loaded = load_profiles(Wire::from(DirectoryRequest {
            directory: directory_name.clone(),
        }))
        .unwrap()
        .unwire();
        assert!(loaded.error.is_none());
        assert_eq!(loaded.payload.len(), 1);
        assert_eq!(loaded.payload[0].name, "中文配置");
        assert_eq!(loaded.payload[0].updated_at_ms, 1_760_000_000_123);
        assert!(matches!(
            loaded.payload[0].target.protocol,
            WireProtocol::Jtag
        ));

        let saved = save_recent_files(Wire::from(SaveRecentFilesRequest {
            directory: directory_name.clone(),
            files: vec![RecentFirmwareRecord {
                path: "C:\\固件\\app.bin".into(),
                format: FirmwareFormat::Bin,
                base_address: Some(0x0800_0000),
                used_at_ms: -500,
            }],
        }))
        .unwrap()
        .unwire();
        assert!(saved.error.is_none());
        let loaded = load_recent_files(Wire::from(DirectoryRequest {
            directory: directory_name,
        }))
        .unwrap()
        .unwire();
        assert_eq!(loaded.payload[0].used_at_ms, -500);
        assert_eq!(loaded.payload[0].format, FirmwareFormat::Bin);
    }

    #[test]
    fn invalid_profile_id_should_return_a_structured_error() {
        let directory = tempfile::tempdir().unwrap();
        let response = save_profiles(Wire::from(SaveProfilesRequest {
            directory: directory.path().to_string_lossy().into_owned(),
            profiles: vec![ProfileRecord {
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
                updated_at_ms: 0,
            }],
        }))
        .unwrap()
        .unwire();
        let error = response.error.expect("invalid UUID must be rejected");
        assert!(matches!(
            error.code,
            chipdock_core::error::ErrorCode::InvalidUserInput
        ));
    }

    #[test]
    fn progress_timestamps_should_use_the_unix_epoch() {
        let event = ProgressEvent {
            id: "job".into(),
            kind: JobKind::Flash,
            stage: JobStage::Completed,
            progress: Some(1.0),
            message: "完成".into(),
            at: chipdock_core::timestamp::AppTimestamp::from(0.25),
        };
        assert_eq!(ProgressRecord::from(event).at_ms, 978_307_200_250);
    }

    #[test]
    fn library_panic_boundary_returns_a_result() {
        let result = invoke::<StatusResponse>(|| panic!("boundary regression"));
        assert!(matches!(result, ffi::Result::Panic));
    }
}
