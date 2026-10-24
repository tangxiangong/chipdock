//! Records shared by the native application services.

use crate::timestamp::AppTimestamp;
use chipdock_core::{
    error::{AppError, ErrorResponse},
    models::*,
};
use uuid::Uuid;

/// A structured, recoverable error shared with Swift.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// Retains target candidates and recovery advice.
    #[error("{response:?}")]
    Operation { response: Box<ErrorResponse> },
}

impl From<AppError> for CoreError {
    fn from(error: AppError) -> Self {
        Self::Operation {
            response: Box::new(error.to_response()),
        }
    }
}

/// A progress event with native Swift `Date` and a stable string job identifier.
#[derive(Debug, Clone)]
pub struct ProgressEvent {
    pub id: String,
    pub kind: JobKind,
    pub stage: JobStage,
    pub progress: Option<f32>,
    pub message: String,
    pub at: AppTimestamp,
}

impl From<JobEvent> for ProgressEvent {
    fn from(event: JobEvent) -> Self {
        Self {
            id: event.id.0.to_string(),
            kind: event.kind,
            stage: event.stage,
            progress: event.progress,
            message: event.message,
            at: event.at.into(),
        }
    }
}

/// A saved configuration. The UUID/RFC3339 disk schema stays compatible.
#[derive(Debug, Clone)]
pub struct SavedProfile {
    pub id: String,
    pub name: String,
    pub probe: Option<String>,
    pub target: TargetSelection,
    pub flash_options: FlashOptions,
    pub bin_base_address: Option<u64>,
    pub updated_at: AppTimestamp,
}

impl From<Profile> for SavedProfile {
    fn from(profile: Profile) -> Self {
        Self {
            id: profile.id.to_string(),
            name: profile.name,
            probe: profile.probe,
            target: profile.target,
            flash_options: profile.flash_options,
            bin_base_address: profile.bin_base_address,
            updated_at: profile.updated_at.into(),
        }
    }
}

impl TryFrom<SavedProfile> for Profile {
    type Error = AppError;

    fn try_from(profile: SavedProfile) -> chipdock_core::error::Result<Self> {
        Ok(Self {
            id: Uuid::parse_str(&profile.id).map_err(|error| AppError::InvalidUserInput {
                detail: format!("无效的配置 ID：{error}"),
            })?,
            name: profile.name,
            probe: profile.probe,
            target: profile.target,
            flash_options: profile.flash_options,
            bin_base_address: profile.bin_base_address,
            updated_at: profile.updated_at.try_into()?,
        })
    }
}

/// A recently opened firmware file.
#[derive(Debug, Clone)]
pub struct RecentFirmware {
    pub path: String,
    pub format: FirmwareFormat,
    pub base_address: Option<u64>,
    pub used_at: AppTimestamp,
}

impl From<RecentFile> for RecentFirmware {
    fn from(file: RecentFile) -> Self {
        Self {
            path: file.path,
            format: file.format,
            base_address: file.base_address,
            used_at: file.used_at.into(),
        }
    }
}

impl TryFrom<RecentFirmware> for RecentFile {
    type Error = AppError;

    fn try_from(file: RecentFirmware) -> chipdock_core::error::Result<Self> {
        Ok(Self {
            path: file.path,
            format: file.format,
            base_address: file.base_address,
            used_at: file.used_at.try_into()?,
        })
    }
}
