use crate::{
    error::{AppError, Result},
    models::{
        ConnectRequest, ConnectionInfo, ProbeSummary, TargetCandidate, TargetInformation,
        WireProtocol,
    },
    services::target::arm_target_candidates,
};
use probe_rs::{
    MemoryInterface, Permissions,
    architecture::arm::{
        ArmChipInfo, dp::DpAddress, memory::ArmMemoryInterface, memory::romtable::Component,
        sequences::DefaultArmSequence,
    },
    config::{Registry, RegistryError},
    probe::{DebugProbeInfo, DebugProbeSelector, WireProtocol as ProbeWireProtocol, list::Lister},
};

const CPUID_ADDRESS: u64 = 0xE000_ED00;
const STM32_FLASH_SIZE_KIB_ADDRESS: u64 = 0x1FFF_F7E0;
const STM32_DBGMCU_IDCODE_ADDRESSES: [u64; 5] = [
    0xE004_2000,
    0x4001_5800,
    0xE004_4000,
    0x5C00_1000,
    0xE00E_1000,
];

struct TargetIdentification {
    candidates: Vec<TargetCandidate>,
    target_information: Option<TargetInformation>,
    compatible_target: Option<String>,
}

/// 列出当前系统可见的调试探针。
pub fn list_probes() -> Result<Vec<ProbeSummary>> {
    Ok(Lister::new()
        .list_all()
        .into_iter()
        .map(probe_summary_from_info)
        .collect())
}

/// 解析前端探针选择；为空时要求系统中只有一个探针。
pub fn require_probe(identifier: Option<&str>) -> Result<String> {
    let probes = list_probes()?;

    select_probe_identifier(identifier, &probes)
}

/// 打开探针会话以验证目标连接参数。
pub fn connect_target(request: ConnectRequest) -> Result<ConnectionInfo> {
    if request.target.speed_khz == Some(0) {
        return Err(AppError::InvalidUserInput {
            detail: "烧录速度必须大于 0 kHz".to_string(),
        });
    }

    let probe_identifier = require_probe(request.probe.as_deref())?;
    let target_selector = request
        .target
        .chip
        .as_deref()
        .map(str::trim)
        .filter(|chip| !chip.is_empty());
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

    let session_result = if request.target.connect_under_reset {
        probe.attach_under_reset(target_selector, Permissions::default())
    } else {
        probe.attach(target_selector, Permissions::default())
    };
    let (session, identified_information) = match session_result {
        Ok(session) => (session, None),
        Err(error) if target_selector.is_none() && is_chip_autodetect_failure(&error) => {
            let identification = identify_target_candidates(
                &probe_identifier,
                protocol,
                request.target.speed_khz,
                request.target.connect_under_reset,
            );

            if let Ok(identification) = &identification
                && let Some(compatible_target) = &identification.compatible_target
            {
                let selector: DebugProbeSelector = probe_identifier
                    .as_str()
                    .parse()
                    .map_err(AppError::operation)?;
                let mut probe = Lister::new().open(selector).map_err(AppError::operation)?;
                probe
                    .select_protocol(protocol)
                    .map_err(AppError::operation)?;
                if let Some(speed_khz) = request.target.speed_khz {
                    probe.set_speed(speed_khz).map_err(AppError::operation)?;
                }
                let session = if request.target.connect_under_reset {
                    probe.attach_under_reset(
                        Some(compatible_target.as_str()),
                        Permissions::default(),
                    )
                } else {
                    probe.attach(Some(compatible_target.as_str()), Permissions::default())
                }
                .map_err(AppError::operation)?;

                (session, identification.target_information.clone())
            } else {
                return Err(AppError::TargetIdentifyFailed {
                    detail: target_identify_detail(identification.as_ref(), &error),
                    candidates: identification
                        .as_ref()
                        .map(|value| value.candidates.clone())
                        .unwrap_or_default(),
                    target_information: identification
                        .ok()
                        .and_then(|value| value.target_information),
                });
            }
        }
        Err(error) => return Err(AppError::operation(error)),
    };
    let mut session = session;
    let chip = session.target().name.clone();
    let target_information = identified_information.or_else(|| {
        chip.starts_with("STM32").then(|| {
            session
                .core(0)
                .ok()
                .map(|mut core| target_information_from_core(&mut core, "STM32"))
        })?
    });

    Ok(ConnectionInfo {
        probe: probe_identifier,
        chip,
        protocol: request.target.protocol,
        speed_khz: request.target.speed_khz,
        connect_under_reset: request.target.connect_under_reset,
        target_information,
    })
}

fn identify_target_candidates(
    probe_identifier: &str,
    protocol: ProbeWireProtocol,
    speed_khz: Option<u32>,
    connect_under_reset: bool,
) -> std::result::Result<TargetIdentification, String> {
    let selector: DebugProbeSelector = probe_identifier
        .parse::<DebugProbeSelector>()
        .map_err(|err| err.to_string())?;
    let mut probe = Lister::new()
        .open(selector)
        .map_err(|err| err.to_string())?;
    probe
        .select_protocol(protocol)
        .map_err(|err| err.to_string())?;

    if let Some(speed_khz) = speed_khz {
        probe.set_speed(speed_khz).map_err(|err| err.to_string())?;
    }

    if connect_under_reset {
        probe
            .attach_to_unspecified_under_reset()
            .map_err(|err| err.to_string())?;
    } else {
        probe
            .attach_to_unspecified()
            .map_err(|err| err.to_string())?;
    }

    let Some((chip_info, target_information)) =
        read_arm_chip_info(probe).map_err(|err| err.to_string())?
    else {
        return Ok(TargetIdentification {
            candidates: Vec::new(),
            target_information: None,
            compatible_target: None,
        });
    };

    let registry = Registry::from_builtin_families();
    let compatible_target = compatible_target(&target_information).map(str::to_string);
    let candidates = compatible_target
        .as_ref()
        .map(|name| {
            vec![TargetCandidate {
                name: name.clone(),
                family: "STM32F1 Series".to_string(),
            }]
        })
        .unwrap_or_else(|| arm_target_candidates(&registry, chip_info));

    Ok(TargetIdentification {
        candidates,
        target_information: Some(target_information),
        compatible_target,
    })
}

fn read_arm_chip_info(
    probe: probe_rs::probe::Probe,
) -> std::result::Result<Option<(ArmChipInfo, TargetInformation)>, probe_rs::Error> {
    if !probe.has_arm_debug_interface() {
        return Ok(None);
    }

    let interface = match probe.try_into_arm_debug_interface(DefaultArmSequence::create()) {
        Ok(interface) => interface,
        Err((_returned_probe, error)) => return Err(error.into()),
    };

    let mut interface = interface;
    let dp_address = DpAddress::Default;
    let mut found_chip_info = None;
    for ap in interface.access_ports(dp_address)? {
        if let Ok(mut memory) = interface.memory_interface(&ap) {
            let base_address = memory.base_address()?;
            let component = Component::try_parse(&mut *memory, base_address)
                .map_err(|err| probe_rs::Error::Other(err.to_string()))?;

            if let Component::Class1RomTable(component_id, _) = component
                && let Some(jep106) = component_id.peripheral_id().jep106()
            {
                let manufacturer = jep106.get().unwrap_or("ARM");
                let chip_info = ArmChipInfo {
                    manufacturer: jep106,
                    part: component_id.peripheral_id().part(),
                };
                let device_type = if manufacturer == "STMicroelectronics" {
                    "STM32"
                } else {
                    manufacturer
                };
                found_chip_info = Some((
                    chip_info,
                    target_information_from_arm_memory(&mut *memory, device_type),
                ));
                break;
            }
        }
    }

    let _probe = interface.close();
    Ok(found_chip_info)
}

fn target_information_from_arm_memory(
    memory: &mut dyn ArmMemoryInterface,
    device_type: &str,
) -> TargetInformation {
    let cpuid = memory.read_word_32(CPUID_ADDRESS).ok();
    let idcode = read_stm32_idcode_arm(memory);
    let flash_size_kib = idcode.and_then(|_| read_stm32_flash_size_arm(memory));

    TargetInformation {
        device_type: detected_device_type(idcode, flash_size_kib, device_type),
        device_id: idcode.map(|value| (value & 0x0fff) as u16),
        revision_id: idcode.map(|value| (value >> 16) as u16),
        cpu: cpuid.and_then(cpu_name_from_cpuid).map(str::to_string),
        flash_size_bytes: flash_size_kib.map(|value| u64::from(value) * 1024),
    }
}

fn target_information_from_core(
    core: &mut probe_rs::Core<'_>,
    device_type: &str,
) -> TargetInformation {
    let cpuid = core.read_word_32(CPUID_ADDRESS).ok();
    let idcode = read_stm32_idcode_core(core);
    let flash_size_kib = idcode.and_then(|_| read_stm32_flash_size_core(core));

    TargetInformation {
        device_type: detected_device_type(idcode, flash_size_kib, device_type),
        device_id: idcode.map(|value| (value & 0x0fff) as u16),
        revision_id: idcode.map(|value| (value >> 16) as u16),
        cpu: cpuid.and_then(cpu_name_from_cpuid).map(str::to_string),
        flash_size_bytes: flash_size_kib.map(|value| u64::from(value) * 1024),
    }
}

fn read_stm32_idcode_arm(memory: &mut dyn ArmMemoryInterface) -> Option<u32> {
    STM32_DBGMCU_IDCODE_ADDRESSES.iter().find_map(|address| {
        let value = memory.read_word_32(*address).ok()?;
        is_stm32_like_idcode(value).then_some(value)
    })
}

fn read_stm32_idcode_core(core: &mut probe_rs::Core<'_>) -> Option<u32> {
    STM32_DBGMCU_IDCODE_ADDRESSES.iter().find_map(|address| {
        let value = core.read_word_32(*address).ok()?;
        is_stm32_like_idcode(value).then_some(value)
    })
}

fn read_stm32_flash_size_arm(memory: &mut dyn ArmMemoryInterface) -> Option<u16> {
    valid_flash_size_kib(memory.read_word_16(STM32_FLASH_SIZE_KIB_ADDRESS).ok()?)
}

fn read_stm32_flash_size_core(core: &mut probe_rs::Core<'_>) -> Option<u16> {
    valid_flash_size_kib(core.read_word_16(STM32_FLASH_SIZE_KIB_ADDRESS).ok()?)
}

fn is_stm32_like_idcode(value: u32) -> bool {
    let device_id = value & 0x0fff;
    value != 0 && value != u32::MAX && (0x400..=0x4ff).contains(&device_id)
}

fn valid_flash_size_kib(value: u16) -> Option<u16> {
    (value != 0 && value != u16::MAX).then_some(value)
}

fn detected_device_type(
    idcode: Option<u32>,
    flash_size_kib: Option<u16>,
    fallback: &str,
) -> String {
    match (
        idcode.map(|value| (value & 0x0fff) as u16),
        idcode.map(|value| (value >> 16) as u16),
        flash_size_kib,
    ) {
        (Some(0x414), Some(0x1309), Some(512)) => "GD32F103VE".to_string(),
        (Some(0x414), _, Some(512)) => "STM32/GD32F10x 高密度 512K".to_string(),
        _ => fallback.to_string(),
    }
}

fn compatible_target(information: &TargetInformation) -> Option<&'static str> {
    match (information.device_id, information.flash_size_bytes) {
        (Some(0x414), Some(0x80000)) => Some("STM32F103VE"),
        _ => None,
    }
}

fn cpu_name_from_cpuid(cpuid: u32) -> Option<&'static str> {
    match (cpuid >> 4) & 0x0fff {
        0xC20 => Some("Cortex-M0"),
        0xC60 => Some("Cortex-M0+"),
        0xC23 => Some("Cortex-M3"),
        0xC24 => Some("Cortex-M4"),
        0xC27 => Some("Cortex-M7"),
        0xD20 => Some("Cortex-M23"),
        0xD21 => Some("Cortex-M33"),
        0xD22 => Some("Cortex-M55"),
        0xD23 => Some("Cortex-M85"),
        _ => None,
    }
}

fn is_chip_autodetect_failure(error: &probe_rs::Error) -> bool {
    matches!(
        error,
        probe_rs::Error::ChipNotFound(RegistryError::ChipAutodetectFailed)
    )
}

fn target_identify_detail(
    identification: std::result::Result<&TargetIdentification, &String>,
    error: &probe_rs::Error,
) -> String {
    match identification {
        Ok(identification) if !identification.candidates.is_empty() => {
            format!(
                "自动识别未能唯一确定目标，已缩小到 {} 个候选",
                identification.candidates.len()
            )
        }
        Ok(_) => format!("自动识别未能确定目标，且当前探测信息没有匹配候选：{error}"),
        Err(detail) => format!("自动识别失败，候选探测也失败：{detail}"),
    }
}

fn select_probe_identifier(identifier: Option<&str>, probes: &[ProbeSummary]) -> Result<String> {
    if let Some(identifier) = identifier.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(identifier.to_string());
    }

    match probes {
        [] => Err(AppError::ProbeNotFound),
        [probe] => Ok(probe.identifier.clone()),
        _ => Err(AppError::InvalidUserInput {
            detail: "检测到多个烧录器，请手动选择一个".to_string(),
        }),
    }
}

fn probe_summary_from_info(probe: DebugProbeInfo) -> ProbeSummary {
    ProbeSummary {
        identifier: selector_string_from_info(&probe),
        vendor_id: probe.vendor_id,
        product_id: probe.product_id,
        serial_number: probe.serial_number,
        product: Some(probe.identifier),
    }
}

fn selector_string_from_info(probe: &DebugProbeInfo) -> String {
    let selector = DebugProbeSelector::from(probe);
    let mut value = format!("{:04x}:{:04x}", selector.vendor_id, selector.product_id);

    // probe-rs 0.31 parses VID:PID-interface[:serial]. Its Display impl omits
    // interface, so preserve it here to keep this identifier usable for open.
    if let Some(interface) = selector.interface {
        value.push_str(&format!("-{interface}"));
    }

    if let Some(serial_number) = selector.serial_number {
        value.push(':');
        value.push_str(&serial_number);
    }

    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;

    #[derive(Debug, thiserror::Error)]
    #[error("probe summary")]
    struct ProbeSummaryError {
        #[source]
        source: std::io::Error,
    }

    fn probe(identifier: &str) -> ProbeSummary {
        ProbeSummary {
            identifier: identifier.to_string(),
            vendor_id: 0x1234,
            product_id: 0x5678,
            serial_number: None,
            product: None,
        }
    }

    #[test]
    fn select_probe_identifier_should_return_explicit_trimmed_identifier() {
        let identifier =
            select_probe_identifier(Some("  probe-1  "), &[]).expect("explicit probe should win");

        assert_eq!(identifier, "probe-1");
    }

    #[test]
    fn select_probe_identifier_should_return_probe_not_found_for_empty_probes() {
        let err = select_probe_identifier(None, &[]).expect_err("empty probes should be rejected");

        assert!(matches!(err, AppError::ProbeNotFound));
    }

    #[test]
    fn select_probe_identifier_should_return_only_probe() {
        let identifier =
            select_probe_identifier(None, &[probe("only-probe")]).expect("single probe is valid");

        assert_eq!(identifier, "only-probe");
    }

    #[test]
    fn select_probe_identifier_should_reject_multiple_probes_without_explicit_identifier() {
        let err = select_probe_identifier(None, &[probe("probe-1"), probe("probe-2")])
            .expect_err("multiple probes require explicit selection");

        assert!(matches!(err, AppError::InvalidUserInput { .. }));
    }

    #[test]
    fn cpu_name_from_cpuid_should_identify_cortex_m3() {
        let name = cpu_name_from_cpuid(0x411F_C231);

        assert_eq!(name, Some("Cortex-M3"));
    }

    #[test]
    fn cpu_name_from_cpuid_should_ignore_unknown_part_number() {
        let name = cpu_name_from_cpuid(0x410F_FFF0);

        assert_eq!(name, None);
    }

    #[test]
    fn gd32f103ve_signature_should_select_compatible_probe_rs_target() {
        let device_type = detected_device_type(Some(0x1309_0414), Some(512), "ARM");
        let information = TargetInformation {
            device_type,
            device_id: Some(0x414),
            revision_id: Some(0x1309),
            cpu: Some("Cortex-M3".to_string()),
            flash_size_bytes: Some(512 * 1024),
        };

        assert_eq!(information.device_type, "GD32F103VE");
        assert_eq!(compatible_target(&information), Some("STM32F103VE"));
    }

    #[test]
    fn compatible_high_density_signature_should_not_claim_gd32_without_revision() {
        let device_type = detected_device_type(Some(0x2003_0414), Some(512), "ARM");
        let information = TargetInformation {
            device_type,
            device_id: Some(0x414),
            revision_id: Some(0x2003),
            cpu: Some("Cortex-M3".to_string()),
            flash_size_bytes: Some(512 * 1024),
        };

        assert_eq!(information.device_type, "STM32/GD32F10x 高密度 512K");
        assert_eq!(compatible_target(&information), Some("STM32F103VE"));
    }

    #[test]
    fn operation_error_should_preserve_chinese_summary_and_root_source_message() {
        let error = AppError::operation(ProbeSummaryError {
            source: std::io::Error::other("USB transfer failed"),
        });
        let response = error.to_response();

        assert_eq!(
            (response.message.as_str(), response.detail.as_deref()),
            ("操作失败", Some("USB transfer failed"))
        );
    }
}
