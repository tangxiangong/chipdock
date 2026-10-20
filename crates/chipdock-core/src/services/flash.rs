use crate::services::jobs::JobEvents;
use crate::{
    error::{AppError, Result},
    models::{FirmwareFormat, FlashOptions, FlashRequest, JobId, JobKind, JobStage, WireProtocol},
    services::firmware::{analyze_firmware_usage, validate_firmware},
    services::jobs::{emit_job_event, new_job_id, probe_progress},
    services::probe::require_probe,
};
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
struct FlashExecutionPolicy {
    allow_destructive_unlock: bool,
    reset_after: bool,
}

impl FlashExecutionPolicy {
    fn from_options(options: &FlashOptions) -> Self {
        Self {
            // Unlocking a protected chip can erase it during attach, before the
            // downloader's dry-run check. Prechecks must not grant that permission.
            allow_destructive_unlock: options.allow_erase_all && !options.dry_run,
            reset_after: options.reset_after && !options.dry_run,
        }
    }

    fn permissions(&self) -> probe_rs::Permissions {
        if self.allow_destructive_unlock {
            probe_rs::Permissions::default().allow_erase_all()
        } else {
            probe_rs::Permissions::default()
        }
    }
}

/// 校验烧录请求并返回已确认的固件格式。
pub fn validate_flash_request(request: &FlashRequest) -> Result<FirmwareFormat> {
    if request.options.skip_erase && request.options.allow_erase_all {
        return Err(AppError::InvalidUserInput {
            detail: "skip_erase 和 allow_erase_all 不能同时启用".to_string(),
        });
    }

    if request.target.speed_khz == Some(0) {
        return Err(AppError::InvalidUserInput {
            detail: "烧录速度必须大于 0 kHz".to_string(),
        });
    }

    let format = validate_firmware(&request.firmware)?;
    Ok(format)
}

fn failed_message(error: &AppError) -> String {
    let response = error.to_response();
    match response.detail {
        Some(detail) => format!("{}：{}", response.message, detail),
        None => response.message,
    }
}

/// 创建后台烧录任务并返回任务 ID。
pub fn flash_firmware(app: &JobEvents, request: FlashRequest) -> Result<JobId> {
    let job_id = new_job_id();

    if let Err(err) = emit_job_event(
        app,
        &job_id,
        JobKind::Flash,
        JobStage::Queued,
        Some(0.0),
        "烧录任务已创建",
    ) {
        let _ = emit_job_event(
            app,
            &job_id,
            JobKind::Flash,
            JobStage::Failed,
            None,
            failed_message(&err),
        );
        return Err(err);
    }

    match run_flash_job(app, &job_id, request) {
        Ok(()) => Ok(job_id),
        Err(err) => {
            let _ = emit_job_event(
                app,
                &job_id,
                JobKind::Flash,
                JobStage::Failed,
                None,
                failed_message(&err),
            );
            Err(err)
        }
    }
}

fn run_flash_job(app: &JobEvents, job_id: &JobId, request: FlashRequest) -> Result<()> {
    let format = validate_flash_request(&request)?;

    emit_job_event(
        app,
        job_id,
        JobKind::Flash,
        JobStage::Preparing,
        Some(0.1),
        format!("固件格式已确认：{format:?}"),
    )?;

    // Validate the image before opening USB or starting any target operation.
    analyze_firmware_usage(&request.firmware)?;

    run_probe_rs_flash(app, job_id, &request, format)?;

    emit_job_event(
        app,
        job_id,
        JobKind::Flash,
        JobStage::Completed,
        Some(1.0),
        if request.options.dry_run {
            "预检查完成"
        } else {
            "烧录完成"
        },
    )?;

    Ok(())
}

fn run_probe_rs_flash(
    app: &JobEvents,
    job_id: &JobId,
    request: &FlashRequest,
    firmware_format: FirmwareFormat,
) -> Result<()> {
    use probe_rs::flashing::{DownloadOptions, download_file_with_options};
    use probe_rs::probe::{DebugProbeSelector, WireProtocol as ProbeWireProtocol, list::Lister};
    emit_job_event(
        app,
        job_id,
        JobKind::Flash,
        JobStage::Connecting,
        Some(0.2),
        "正在连接烧录器",
    )?;

    let probe_identifier = require_probe(request.probe.as_deref())?;
    let target_selector = request
        .target
        .chip
        .as_deref()
        .map(str::trim)
        .filter(|chip| !chip.is_empty());
    let policy = FlashExecutionPolicy::from_options(&request.options);
    let selector: DebugProbeSelector = probe_identifier
        .as_str()
        .parse()
        .map_err(AppError::operation)?;

    let mut probe = Lister::new().open(selector).map_err(AppError::operation)?;

    let protocol = match request.target.protocol {
        WireProtocol::Swd => ProbeWireProtocol::Swd,
        WireProtocol::Jtag => ProbeWireProtocol::Jtag,
    };
    probe
        .select_protocol(protocol)
        .map_err(AppError::operation)?;

    if let Some(speed_khz) = request.target.speed_khz {
        probe.set_speed(speed_khz).map_err(AppError::operation)?;
    }

    let mut session = if request.target.connect_under_reset {
        probe.attach_under_reset(target_selector, policy.permissions())
    } else {
        probe.attach(target_selector, policy.permissions())
    }
    .map_err(AppError::operation)?;

    let format = probe_rs_format(firmware_format, request.firmware.base_address)?;
    let mut options = DownloadOptions::default();
    options.verify = request.options.verify;
    options.dry_run = request.options.dry_run;
    options.skip_erase = request.options.skip_erase;
    options.do_chip_erase = request.options.allow_erase_all;
    // probe-rs emits synthetic Failed events for dry runs. A successful precheck
    // must not be surfaced as a failed write through those progress callbacks.
    if !request.options.dry_run {
        options.progress = probe_progress(app, job_id, JobKind::Flash);
    }

    emit_job_event(
        app,
        job_id,
        JobKind::Flash,
        JobStage::Preparing,
        None,
        if request.options.dry_run {
            "正在预检查烧录计划"
        } else {
            "正在准备烧录计划"
        },
    )?;

    download_file_with_options(
        &mut session,
        Path::new(&request.firmware.path),
        format,
        options,
    )
    .map_err(AppError::operation)?;

    if policy.reset_after {
        emit_job_event(
            app,
            job_id,
            JobKind::Flash,
            JobStage::Resetting,
            None,
            "正在复位目标芯片",
        )?;

        session
            .core(0)
            .and_then(|mut core| core.reset())
            .map_err(AppError::operation)?;
    }

    Ok(())
}

fn probe_rs_format(
    format: FirmwareFormat,
    base_address: Option<u64>,
) -> Result<Box<dyn probe_rs::flashing::ImageLoader>> {
    use probe_rs::flashing::{BinLoader, BinOptions, ElfLoader, ElfOptions, HexLoader};

    Ok(match format {
        FirmwareFormat::Elf => Box::new(ElfLoader(ElfOptions::default())),
        FirmwareFormat::Hex => Box::new(HexLoader),
        FirmwareFormat::Bin => Box::new(BinLoader(BinOptions {
            base_address: Some(base_address.ok_or(AppError::MissingBinBaseAddress)?),
            skip: 0,
        })),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;
    use crate::models::{FirmwareInput, FlashOptions, TargetSelection, WireProtocol};

    fn valid_elf_request() -> FlashRequest {
        FlashRequest {
            firmware: FirmwareInput {
                path: "/tmp/app.elf".to_string(),
                format: Some(FirmwareFormat::Elf),
                base_address: None,
            },
            probe: None,
            target: TargetSelection {
                chip: Some("STM32F103C8".to_string()),
                protocol: WireProtocol::Swd,
                speed_khz: None,
                connect_under_reset: false,
            },
            options: FlashOptions {
                verify: true,
                dry_run: false,
                skip_erase: false,
                allow_erase_all: false,
                reset_after: false,
            },
        }
    }

    #[test]
    fn validate_flash_request_should_accept_valid_elf_request() {
        let format = validate_flash_request(&valid_elf_request()).expect("request should be valid");

        assert_eq!(format, FirmwareFormat::Elf);
    }

    #[test]
    fn validate_flash_request_should_accept_auto_chip() {
        let mut request = valid_elf_request();
        request.target.chip = None;

        let format = validate_flash_request(&request).expect("request should be valid");

        assert_eq!(format, FirmwareFormat::Elf);
    }

    #[test]
    fn dry_run_should_disable_destructive_unlock_and_post_flash_reset() {
        let mut options = valid_elf_request().options;
        options.dry_run = true;
        options.allow_erase_all = true;
        options.reset_after = true;
        assert_eq!(
            FlashExecutionPolicy::from_options(&options),
            FlashExecutionPolicy {
                allow_destructive_unlock: false,
                reset_after: false,
            }
        );
    }

    #[test]
    fn ordinary_programming_should_only_grant_explicit_erase_permission() {
        let mut options = valid_elf_request().options;
        options.reset_after = true;
        assert_eq!(
            FlashExecutionPolicy::from_options(&options),
            FlashExecutionPolicy {
                allow_destructive_unlock: false,
                reset_after: true,
            }
        );
        options.allow_erase_all = true;
        assert!(FlashExecutionPolicy::from_options(&options).allow_destructive_unlock);
    }

    #[test]
    fn empty_firmware_should_fail_before_any_probe_connection_stage() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("empty.bin");
        std::fs::write(&path, []).unwrap();
        let mut request = valid_elf_request();
        request.firmware = FirmwareInput {
            path: path.display().to_string(),
            format: None,
            base_address: Some(0),
        };
        let events = JobEvents::default();
        assert!(flash_firmware(&events, request).is_err());
        let events = events.drain();
        assert!(matches!(events.last().unwrap().stage, JobStage::Failed));
        assert!(!events.iter().any(|event| matches!(
            event.stage,
            JobStage::Connecting | JobStage::Programming | JobStage::Resetting
        )));
    }

    #[test]
    fn malformed_elf_should_fail_before_any_probe_connection_stage() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bad.elf");
        std::fs::write(&path, b"not an ELF image").unwrap();
        let mut request = valid_elf_request();
        request.firmware.path = path.display().to_string();
        let events = JobEvents::default();
        assert!(flash_firmware(&events, request).is_err());
        assert!(
            !events
                .drain()
                .iter()
                .any(|event| matches!(event.stage, JobStage::Connecting))
        );
    }

    #[test]
    fn validate_flash_request_should_reject_missing_bin_base_address() {
        let mut request = valid_elf_request();
        request.firmware.path = "/tmp/app.bin".to_string();
        request.firmware.format = Some(FirmwareFormat::Bin);

        let err = validate_flash_request(&request).expect_err("request should be rejected");

        assert!(matches!(err, AppError::MissingBinBaseAddress));
    }

    #[test]
    fn validate_flash_request_should_reject_format_mismatch() {
        let mut request = valid_elf_request();
        request.firmware.path = "/tmp/app.bin".to_string();
        request.firmware.format = Some(FirmwareFormat::Elf);

        let err = validate_flash_request(&request).expect_err("request should be rejected");

        assert!(matches!(err, AppError::InvalidUserInput { .. }));
    }

    #[test]
    fn validate_flash_request_should_reject_skip_erase_and_allow_erase_all_together() {
        let mut request = valid_elf_request();
        request.options.skip_erase = true;
        request.options.allow_erase_all = true;

        let err = validate_flash_request(&request).expect_err("request should be rejected");

        assert!(
            matches!(err, AppError::InvalidUserInput { detail } if detail == "skip_erase 和 allow_erase_all 不能同时启用")
        );
    }

    #[test]
    fn validate_flash_request_should_reject_zero_speed() {
        let mut request = valid_elf_request();
        request.target.speed_khz = Some(0);

        let err = validate_flash_request(&request).expect_err("request should be rejected");

        assert!(
            matches!(err, AppError::InvalidUserInput { detail } if detail == "烧录速度必须大于 0 kHz")
        );
    }

    #[test]
    fn failed_message_should_include_frontend_safe_invalid_input_detail() {
        let message = failed_message(&AppError::InvalidUserInput {
            detail: "烧录速度必须大于 0 kHz".to_string(),
        });

        assert_eq!(message, "用户输入无效：烧录速度必须大于 0 kHz");
    }

    #[test]
    fn failed_message_should_include_raw_operation_detail() {
        let message = failed_message(&AppError::operation(std::io::Error::other(
            "raw stack detail",
        )));

        assert_eq!(message, "操作失败：raw stack detail");
    }
}
