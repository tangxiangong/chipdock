use crate::error::{AppError, Result};
use std::{fs, path::Path, sync::Once};

pub(super) fn initialize() {
    static INIT: Once = Once::new();
    INIT.call_once(probe_rs_espressif::register_plugin);
}

// A BIN contains flash offsets, not the CPU's memory-mapped flash addresses.
// Prefer the build's manifest; a standalone application header has no partition offset.
pub(super) fn bin_address(path: &Path) -> Result<u64> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let canonical = fs::canonicalize(path).map_err(AppError::IoFailure)?;
    for directory in [Some(parent), parent.parent()].into_iter().flatten() {
        let manifest = directory.join("flasher_args.json");
        if !manifest.is_file() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(manifest).map_err(AppError::IoFailure)?)
                .map_err(|e| invalid(format!("ESP 烧录配置无效：{e}")))?;
        if let Some(files) = value.get("flash_files").and_then(|v| v.as_object()) {
            for (address, file) in files {
                if let Some(file) = file.as_str()
                    && fs::canonicalize(directory.join(file)).ok().as_ref() == Some(&canonical)
                {
                    return super::firmware::parse_address(address);
                }
            }
        }
    }
    let data = fs::read(path).map_err(AppError::IoFailure)?;
    if image_chip(&data).is_some()
        && data.get(32..36) == Some(&[0x32, 0x54, 0xcd, 0xab])
        && valid_image(&data)
    {
        for name in [
            "partition-table.bin",
            "partition_table/partition-table.bin",
            "partitions.bin",
        ] {
            let table = parent.join(name);
            if !table.is_file() {
                continue;
            }
            let table = esp_idf_part::PartitionTable::try_from_bytes(
                fs::read(table).map_err(AppError::IoFailure)?,
            )
            .map_err(|error| invalid(format!("ESP 分区表无效：{error}")))?;
            let apps: Vec<_> = table
                .partitions()
                .iter()
                .filter(|entry| entry.ty() == esp_idf_part::Type::App)
                .collect();
            if let [app] = apps.as_slice()
                && app.offset() >= 0x10000
                && data.len() <= app.size() as usize
            {
                return Ok(u64::from(app.offset()));
            }
            return Err(invalid(
                "分区表中没有唯一且足够大的应用分区；请使用 flasher_args.json 或手动输入地址。"
                    .into(),
            ));
        }
    }
    // Merged images can start at zero or at the bootloader's physical offset.
    for start in [0usize, 0x1000, 0x2000] {
        let Some(chip) = image_chip(data.get(start..).unwrap_or_default()) else {
            continue;
        };
        let boot_offset = match chip {
            0 | 2 => 0x1000usize,
            18 | 23 | 28 | 32 => 0x2000,
            _ => 0,
        };
        let image = &data[start..];
        let bootloader = image.get(32..36) == Some(&[0x50, 0, 0, 0]);
        if bootloader && start <= boot_offset && data[..start].iter().all(|b| *b == 0xff) {
            let base = boot_offset - start;
            // Require a valid segment layout/checksum before interpreting an image as a bootloader.
            if valid_image(image) {
                return Ok(base as u64);
            }
        }
    }
    Err(invalid("无法自动确定 BIN 烧录地址。应用 BIN 不包含分区偏移；请保留同目录 flasher_args.json，使用合并 BIN，或手动输入地址。".into()))
}

pub(super) fn image_chip(data: &[u8]) -> Option<u16> {
    if data.len() < 24 || data[0] != 0xe9 || !(1..=16).contains(&data[1]) || data[23] > 1 {
        return None;
    }
    let chip = u16::from_le_bytes([data[12], data[13]]);
    matches!(chip, 0 | 2 | 5 | 9 | 12 | 13 | 16 | 18 | 20 | 23 | 28 | 32).then_some(chip)
}

fn valid_image(data: &[u8]) -> bool {
    if image_chip(data).is_none() {
        return false;
    }
    let mut offset = 24usize;
    let mut checksum = 0xef;
    for _ in 0..data[1] {
        let Some(header) = data.get(offset..offset + 8) else {
            return false;
        };
        let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        offset += 8;
        let Some(end) = offset.checked_add(length) else {
            return false;
        };
        let Some(segment) = data.get(offset..end) else {
            return false;
        };
        for byte in segment {
            checksum ^= byte;
        }
        offset = end;
    }
    let checksum_offset = offset | 15;
    data.get(checksum_offset) == Some(&checksum)
        && (data[23] == 0 || data.len().saturating_sub(checksum_offset + 1) >= 32)
}

pub(super) fn validate_bin_target(path: &Path, target: &str) -> Result<()> {
    let data = fs::read(path).map_err(AppError::IoFailure)?;
    for start in [0usize, 0x1000, 0x2000] {
        let Some(image) = data.get(start..) else {
            continue;
        };
        let Some(chip) = image_chip(image) else {
            continue;
        };
        if !data[..start].iter().all(|byte| *byte == 0xff) {
            continue;
        }
        let expected = match chip {
            0 => "esp32",
            2 => "esp32s2",
            5 => "esp32c3",
            9 => "esp32s3",
            12 => "esp32c2",
            13 => "esp32c6",
            16 => "esp32h2",
            18 => "esp32p4",
            20 => "esp32c61",
            23 => "esp32c5",
            28 => "esp32h4",
            32 => "esp32s31",
            _ => continue,
        };
        if !target
            .split('-')
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case(expected))
        {
            return Err(invalid(format!(
                "BIN 固件适用于 {expected}，当前目标为 {target}，已停止烧录。"
            )));
        }
        if !valid_image(image) {
            return Err(invalid(
                "ESP BIN 镜像已截断或校验和无效，已停止烧录。".into(),
            ));
        }
        break;
    }
    Ok(())
}

fn invalid(detail: String) -> AppError {
    AppError::InvalidUserInput { detail }
}

pub(super) fn protocol(
    requested: crate::models::WireProtocol,
    chip: Option<&str>,
    probe: &str,
) -> crate::models::WireProtocol {
    if chip.is_some_and(|name| name.trim().to_ascii_lowercase().starts_with("esp32"))
        || probe.to_ascii_lowercase().starts_with("303a:1001")
    {
        crate::models::WireProtocol::Jtag
    } else {
        requested
    }
}

// A compressed transfer page can expand to megabytes of 0xFF in a merged BIN.
// The upstream 2-second physical-page timeout is too short for that work.
pub(super) fn flash_registry() -> Result<probe_rs::config::Registry> {
    initialize();
    let mut registry = probe_rs::config::Registry::from_builtin_families();
    let families: Vec<_> = registry
        .families()
        .iter()
        .filter(|family| family.name.to_ascii_lowercase().starts_with("esp32"))
        .cloned()
        .collect();
    for mut family in families {
        for algorithm in &mut family.flash_algorithms {
            algorithm.flash_properties.program_page_timeout =
                algorithm.flash_properties.program_page_timeout.max(120_000);
        }
        registry
            .add_target_family(family)
            .map_err(AppError::operation)?;
    }
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(chip: u16, app: bool) -> Vec<u8> {
        let mut bytes = vec![0; 48];
        bytes[0] = 0xe9;
        bytes[1] = 1;
        bytes[12..14].copy_from_slice(&chip.to_le_bytes());
        bytes[28] = 4;
        if app {
            bytes[32..36].copy_from_slice(&[0x32, 0x54, 0xcd, 0xab]);
        } else {
            bytes[32] = 0x50;
        }
        bytes[47] = bytes[32..36].iter().fold(0xef, |a, b| a ^ b);
        bytes
    }

    #[test]
    fn detects_bootloader_offsets_and_merged_padding_without_guessing_application_offset() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("firmware.bin");
        for (chip, offset) in [
            (0, 0x1000),
            (2, 0x1000),
            (9, 0),
            (5, 0),
            (12, 0),
            (13, 0),
            (20, 0),
            (23, 0x2000),
            (18, 0x2000),
        ] {
            fs::write(&path, image(chip, false)).unwrap();
            assert_eq!(bin_address(&path).unwrap(), offset);
            let mut merged = vec![0xff; offset as usize];
            merged.extend(image(chip, false));
            fs::write(&path, merged).unwrap();
            assert_eq!(bin_address(&path).unwrap(), 0);
        }
        fs::write(&path, image(9, true)).unwrap();
        assert!(bin_address(&path).is_err());
        fs::write(
            dir.path().join("flasher_args.json"),
            r#"{"flash_files":{"0x20000":"firmware.bin"}}"#,
        )
        .unwrap();
        assert_eq!(bin_address(&path).unwrap(), 0x20000);
    }

    #[test]
    fn rejects_truncated_or_corrupt_images() {
        let mut bytes = image(9, false);
        assert!(valid_image(&bytes));
        bytes[47] ^= 1;
        assert!(!valid_image(&bytes));
        assert!(!valid_image(&bytes[..25]));
    }

    #[test]
    fn resolves_unique_application_partition_and_rejects_ambiguous_ota() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.bin");
        fs::write(&path, image(9, true)).unwrap();
        let table = dir.path().join("partition-table.bin");
        let binary = esp_idf_part::PartitionTable::try_from_str("factory,app,factory,0x20000,1M")
            .unwrap()
            .to_bin()
            .unwrap();
        fs::write(&table, &binary).unwrap();
        assert_eq!(bin_address(&path).unwrap(), 0x20000);
        let mut corrupt = binary.clone();
        corrupt[4] ^= 1;
        fs::write(&table, corrupt).unwrap();
        assert!(bin_address(&path).is_err());
        fs::write(&table, &binary[..32]).unwrap();
        assert!(bin_address(&path).is_err());
        let ota = esp_idf_part::PartitionTable::try_from_str(
            "ota_0,app,ota_0,0x20000,1M\nota_1,app,ota_1,0x120000,1M",
        )
        .unwrap()
        .to_bin()
        .unwrap();
        fs::write(&table, ota).unwrap();
        assert!(bin_address(&path).is_err());
    }

    #[test]
    fn rejects_wrong_chip_and_truncated_bin_before_flashing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("firmware.bin");
        fs::write(&path, image(9, true)).unwrap();
        assert!(validate_bin_target(&path, "esp32s3").is_ok());
        assert!(validate_bin_target(&path, "esp32c3").is_err());
        assert!(validate_bin_target(&path, "STM32F103C8").is_err());
        let mut truncated = image(9, true);
        truncated[23] = 1;
        fs::write(&path, truncated).unwrap();
        assert!(validate_bin_target(&path, "esp32s3").is_err());
        fs::write(&path, [1, 2, 3, 4]).unwrap();
        assert!(validate_bin_target(&path, "STM32F103C8").is_ok());
    }

    #[test]
    fn flash_registry_extends_only_esp_programming_timeouts() {
        let original = probe_rs::config::Registry::from_builtin_families();
        let registry = flash_registry().unwrap();
        let esp = registry.get_target_by_name("esp32s3").unwrap();
        assert!(
            esp.flash_algorithms
                .iter()
                .all(|a| a.flash_properties.program_page_timeout >= 120_000)
        );
        let arm = registry.get_target_by_name("STM32F103C8").unwrap();
        let before = original.get_target_by_name("STM32F103C8").unwrap();
        assert_eq!(
            arm.flash_algorithms[0]
                .flash_properties
                .program_page_timeout,
            before.flash_algorithms[0]
                .flash_properties
                .program_page_timeout
        );
        assert!(matches!(
            protocol(crate::models::WireProtocol::Swd, None, "303a:1001:device"),
            crate::models::WireProtocol::Jtag
        ));
        assert!(matches!(
            protocol(
                crate::models::WireProtocol::Swd,
                Some("esp32c6"),
                "external"
            ),
            crate::models::WireProtocol::Jtag
        ));
        assert!(matches!(
            protocol(crate::models::WireProtocol::Swd, None, "0483:3748"),
            crate::models::WireProtocol::Swd
        ));
    }

    #[test]
    fn registers_all_esp_targets_once() {
        initialize();
        initialize();
        let registry = probe_rs::config::Registry::from_builtin_families();
        for name in [
            "esp32", "esp32s2", "esp32s3", "esp32c2", "esp32c3", "esp32c6", "esp32h2", "esp32p4",
        ] {
            assert!(!registry.search_chips(name).is_empty(), "{name}");
        }
    }
}
