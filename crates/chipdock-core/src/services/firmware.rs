use crate::{
    error::{AppError, Result},
    models::{FirmwareFormat, FirmwareInput, FirmwarePreview, FirmwareUsage, FirmwareUsageSegment},
};
use ihex::Record;
use object::{
    Endianness,
    elf::{FileHeader32, FileHeader64, PT_LOAD},
    read::elf::{FileHeader, ProgramHeader},
};
use std::{fs, io::Read, path::Path};

/// 根据文件扩展名推断固件格式。
pub fn detect_format(path: &Path) -> Result<FirmwareFormat> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| AppError::UnsupportedFirmwareFormat {
            path: path.display().to_string(),
        })?;

    match extension.as_str() {
        "elf" | "axf" => Ok(FirmwareFormat::Elf),
        "hex" => Ok(FirmwareFormat::Hex),
        "bin" => Ok(FirmwareFormat::Bin),
        _ => Err(AppError::UnsupportedFirmwareFormat {
            path: path.display().to_string(),
        }),
    }
}

/// 校验固件输入并返回最终采用的固件格式。
pub fn validate_firmware(input: &FirmwareInput) -> Result<FirmwareFormat> {
    if input.path.trim().is_empty() {
        return Err(AppError::InvalidUserInput {
            detail: "固件路径不能为空".to_string(),
        });
    }

    let detected = detect_format(Path::new(&input.path))?;

    if let Some(explicit) = input.format
        && explicit != detected
    {
        return Err(AppError::InvalidUserInput {
            detail: format!("固件格式与文件扩展名不匹配：选择为 {explicit:?}，检测为 {detected:?}"),
        });
    }

    if detected == FirmwareFormat::Bin {
        let address = input.base_address.ok_or(AppError::MissingBinBaseAddress)?;
        validate_flash_address(address)?;
    }

    Ok(detected)
}

/// 分析固件文件中实际会写入 Flash 的地址范围和字节数。
pub fn analyze_firmware_usage(input: &FirmwareInput) -> Result<FirmwareUsage> {
    let format = validate_firmware(input)?;
    let path = Path::new(&input.path);
    // Windows cannot open a directory as a regular file. Classify it before
    // opening so every platform reports the same invalid-input error.
    if !fs::metadata(path).map_err(AppError::IoFailure)?.is_file() {
        return Err(AppError::InvalidUserInput {
            detail: "固件路径必须是普通文件".to_string(),
        });
    }
    let mut segments = match format {
        FirmwareFormat::Bin => analyze_bin_usage(path, input.base_address)?,
        FirmwareFormat::Hex => analyze_hex_usage(path)?,
        FirmwareFormat::Elf => analyze_elf_usage(path)?,
    };

    if segments.is_empty() {
        return Err(AppError::InvalidUserInput {
            detail: "固件中没有可写入的数据".to_string(),
        });
    }
    merge_segments(&mut segments);

    Ok(usage_from_segments(format, segments))
}

/// 读取固件起始位置的实际文件字节，保留空洞，限制预览内存占用。
pub fn read_firmware_preview(input: &FirmwareInput, length: u32) -> Result<FirmwarePreview> {
    if !(1..=65_536).contains(&length) {
        return Err(AppError::InvalidUserInput {
            detail: "固件预览长度须为 1–65536 字节".to_string(),
        });
    }
    let usage = analyze_firmware_usage(input)?;
    let start_address = usage
        .start_address
        .ok_or_else(|| AppError::InvalidUserInput {
            detail: "固件中没有可预览的数据".to_string(),
        })?;
    let count = usage.span_bytes.min(u64::from(length)) as usize;
    let mut bytes = vec![None; count];
    let path = Path::new(&input.path);

    match usage.format {
        FirmwareFormat::Bin => {
            let mut data = vec![0; count];
            fs::File::open(path)
                .and_then(|mut file| file.read_exact(&mut data))
                .map_err(AppError::IoFailure)?;
            copy_preview_bytes(start_address, &mut bytes, start_address, &data)?;
        }
        FirmwareFormat::Hex => {
            let data = fs::read_to_string(path).map_err(AppError::IoFailure)?;
            let mut base = 0;
            for record in ihex::Reader::new(&data) {
                match record.map_err(|error| AppError::InvalidUserInput {
                    detail: format!("HEX 解析失败：{error}"),
                })? {
                    Record::Data { offset, value } => {
                        copy_preview_bytes(
                            start_address,
                            &mut bytes,
                            base + u64::from(offset),
                            &value,
                        )?;
                    }
                    Record::ExtendedSegmentAddress(address) => base = u64::from(address) << 4,
                    Record::ExtendedLinearAddress(address) => base = u64::from(address) << 16,
                    Record::EndOfFile
                    | Record::StartSegmentAddress { .. }
                    | Record::StartLinearAddress(_) => {}
                }
            }
        }
        FirmwareFormat::Elf => {
            let data = fs::read(path).map_err(AppError::IoFailure)?;
            match object::FileKind::parse(&*data).map_err(elf_parse_error)? {
                object::FileKind::Elf32 => {
                    let header =
                        FileHeader32::<Endianness>::parse(&*data).map_err(elf_parse_error)?;
                    preview_elf_segments(header, &data, start_address, &mut bytes)?;
                }
                object::FileKind::Elf64 => {
                    let header =
                        FileHeader64::<Endianness>::parse(&*data).map_err(elf_parse_error)?;
                    preview_elf_segments(header, &data, start_address, &mut bytes)?;
                }
                _ => {
                    return Err(AppError::InvalidUserInput {
                        detail: "固件不是 ELF 文件".to_string(),
                    });
                }
            }
        }
    }
    Ok(FirmwarePreview {
        usage,
        start_address,
        bytes,
    })
}

fn copy_preview_bytes(
    start: u64,
    preview: &mut [Option<u8>],
    address: u64,
    data: &[u8],
) -> Result<()> {
    let low = start.max(address);
    let high =
        segment_end(start, preview.len() as u64)?.min(segment_end(address, data.len() as u64)?);
    if low < high {
        let count = (high - low) as usize;
        let target = (low - start) as usize;
        let source = (low - address) as usize;
        for (destination, value) in preview[target..target + count]
            .iter_mut()
            .zip(&data[source..source + count])
        {
            *destination = Some(*value);
        }
    }
    Ok(())
}

fn preview_elf_segments<T: FileHeader>(
    header: &T,
    data: &[u8],
    start: u64,
    preview: &mut [Option<u8>],
) -> Result<()> {
    let endian = header.endian().map_err(elf_parse_error)?;
    for segment in header
        .program_headers(endian, data)
        .map_err(elf_parse_error)?
    {
        if segment.p_type(endian) == PT_LOAD {
            let bytes = segment
                .data(endian, data)
                .map_err(|_| AppError::InvalidUserInput {
                    detail: "ELF 解析失败：无法读取 loadable segment 数据".to_string(),
                })?;
            copy_preview_bytes(start, preview, segment.p_paddr(endian).into(), bytes)?;
        }
    }
    Ok(())
}

/// 解析用户输入的十进制或 `0x` 前缀十六进制地址。
pub fn parse_address(value: &str) -> Result<u64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidFirmwareAddress {
            detail: "地址不能为空".to_string(),
        });
    }

    let parsed = if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16)
    } else {
        trimmed.parse::<u64>()
    };

    parsed.map_err(|err| AppError::InvalidFirmwareAddress {
        detail: format!("无法解析地址 `{trimmed}`: {err}"),
    })
}

fn validate_flash_address(address: u64) -> Result<()> {
    if !address.is_multiple_of(4) {
        return Err(AppError::InvalidFirmwareAddress {
            detail: "Flash 地址必须 4 字节对齐".to_string(),
        });
    }

    Ok(())
}

fn analyze_bin_usage(path: &Path, base_address: Option<u64>) -> Result<Vec<FirmwareUsageSegment>> {
    let base_address = base_address.ok_or(AppError::MissingBinBaseAddress)?;
    let file = fs::File::open(path).map_err(AppError::IoFailure)?;
    let metadata = file.metadata().map_err(AppError::IoFailure)?;
    if !metadata.is_file() {
        return Err(AppError::InvalidUserInput {
            detail: "固件路径必须是普通文件".to_string(),
        });
    }
    let size = metadata.len();
    if size == 0 {
        return Err(AppError::InvalidUserInput {
            detail: "固件文件为空".to_string(),
        });
    }
    let end = segment_end(base_address, size)?;

    Ok(vec![FirmwareUsageSegment {
        start: base_address,
        end,
        size,
    }])
}

fn analyze_hex_usage(path: &Path) -> Result<Vec<FirmwareUsageSegment>> {
    let data = fs::read_to_string(path).map_err(AppError::IoFailure)?;
    let mut base_address = 0u64;
    let mut segments = Vec::new();

    for record in ihex::Reader::new(&data) {
        match record.map_err(|err| AppError::InvalidUserInput {
            detail: format!("HEX 解析失败：{err}"),
        })? {
            Record::Data { offset, value } => {
                let start = base_address + u64::from(offset);
                push_segment(&mut segments, start, value.len() as u64)?;
            }
            Record::ExtendedSegmentAddress(address) => {
                base_address = u64::from(address) * 16;
            }
            Record::ExtendedLinearAddress(address) => {
                base_address = u64::from(address) << 16;
            }
            Record::EndOfFile
            | Record::StartSegmentAddress { .. }
            | Record::StartLinearAddress(_) => {}
        }
    }

    Ok(segments)
}

fn analyze_elf_usage(path: &Path) -> Result<Vec<FirmwareUsageSegment>> {
    let data = fs::read(path).map_err(AppError::IoFailure)?;
    let file_kind = object::FileKind::parse(&*data).map_err(|err| AppError::InvalidUserInput {
        detail: format!("ELF 解析失败：{err}"),
    })?;

    match file_kind {
        object::FileKind::Elf32 => {
            let header = FileHeader32::<Endianness>::parse(&*data).map_err(elf_parse_error)?;
            extract_elf_segments(header, &data)
        }
        object::FileKind::Elf64 => {
            let header = FileHeader64::<Endianness>::parse(&*data).map_err(elf_parse_error)?;
            extract_elf_segments(header, &data)
        }
        _ => Err(AppError::InvalidUserInput {
            detail: "固件不是 ELF 文件".to_string(),
        }),
    }
}

fn extract_elf_segments<T: FileHeader>(
    header: &T,
    data: &[u8],
) -> Result<Vec<FirmwareUsageSegment>> {
    let endian = header.endian().map_err(elf_parse_error)?;
    let mut segments = Vec::new();

    for segment in header
        .program_headers(endian, data)
        .map_err(elf_parse_error)?
    {
        if segment.p_type(endian) != PT_LOAD {
            continue;
        }
        let segment_data = segment
            .data(endian, data)
            .map_err(|_| AppError::InvalidUserInput {
                detail: "ELF 解析失败：无法读取 loadable segment 数据".to_string(),
            })?;
        if segment_data.is_empty() {
            continue;
        }

        push_segment(
            &mut segments,
            segment.p_paddr(endian).into(),
            segment_data.len() as u64,
        )?;
    }

    Ok(segments)
}

fn usage_from_segments(
    format: FirmwareFormat,
    mut segments: Vec<FirmwareUsageSegment>,
) -> FirmwareUsage {
    segments.sort_by_key(|segment| segment.start);

    let used_bytes = segments.iter().map(|segment| segment.size).sum();
    let start_address = segments.first().map(|segment| segment.start);
    let end_address = segments.iter().map(|segment| segment.end).max();
    let span_bytes = match (start_address, end_address) {
        (Some(start), Some(end)) => end.saturating_sub(start),
        _ => 0,
    };

    FirmwareUsage {
        format,
        used_bytes,
        span_bytes,
        start_address,
        end_address,
        segments,
    }
}

fn segment_end(start: u64, size: u64) -> Result<u64> {
    start
        .checked_add(size)
        .ok_or_else(|| AppError::InvalidFirmwareAddress {
            detail: "固件数据的地址范围溢出".to_string(),
        })
}

fn push_segment(segments: &mut Vec<FirmwareUsageSegment>, start: u64, size: u64) -> Result<()> {
    if size == 0 {
        return Ok(());
    }

    let end = segment_end(start, size)?;
    segments.push(FirmwareUsageSegment { start, end, size });
    Ok(())
}

fn merge_segments(segments: &mut Vec<FirmwareUsageSegment>) {
    segments.sort_by_key(|segment| segment.start);
    let mut merged: Vec<FirmwareUsageSegment> = Vec::new();

    for segment in segments.drain(..) {
        if let Some(last) = merged.last_mut()
            && segment.start <= last.end
        {
            last.end = last.end.max(segment.end);
            last.size = last.end.saturating_sub(last.start);
            continue;
        }

        merged.push(segment);
    }

    *segments = merged;
}

fn elf_parse_error(error: object::Error) -> AppError {
    AppError::InvalidUserInput {
        detail: format!("ELF 解析失败：{error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_should_bound_bin_bytes_without_truncating_usage() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("preview.bin");
        let data: Vec<u8> = (0..=255).cycle().take(8192).collect();
        fs::write(&file, &data).unwrap();
        let input = FirmwareInput {
            path: file.display().to_string(),
            format: None,
            base_address: Some(0x0800_0000),
        };
        let preview = read_firmware_preview(&input, 4096).unwrap();
        assert_eq!(
            preview.bytes,
            data[..4096].iter().copied().map(Some).collect::<Vec<_>>()
        );
        assert_eq!(
            (preview.start_address, preview.usage.used_bytes),
            (0x0800_0000, 8192)
        );
    }

    #[test]
    fn preview_should_not_pad_short_bin_files_with_zeroes() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("short.bin");
        fs::write(&file, [0, 0x41, 0xff]).unwrap();
        let input = FirmwareInput {
            path: file.display().to_string(),
            format: None,
            base_address: Some(0),
        };
        let preview = read_firmware_preview(&input, 4096).unwrap();
        assert_eq!(preview.bytes, vec![Some(0), Some(0x41), Some(0xff)]);
    }

    #[test]
    fn preview_should_preserve_hex_holes_and_linear_addresses() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("sparse.hex");
        fs::write(
            &file,
            ":020000040800F2\n:020000000001FD\n:01000300FFFD\n:00000001FF\n",
        )
        .unwrap();
        let input = FirmwareInput {
            path: file.display().to_string(),
            format: None,
            base_address: None,
        };
        let preview = read_firmware_preview(&input, 4096).unwrap();
        assert_eq!(
            (preview.start_address, preview.bytes),
            (0x0800_0000, vec![Some(0), Some(1), None, Some(0xff)])
        );
    }

    #[test]
    fn preview_should_apply_hex_segment_addresses_and_later_overlaps() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("segment.hex");
        fs::write(
            &file,
            ":020000021234B6\n:020002000102F9\n:01000300FFFD\n:00000001FF\n",
        )
        .unwrap();
        let input = FirmwareInput {
            path: file.display().to_string(),
            format: None,
            base_address: None,
        };
        let preview = read_firmware_preview(&input, 4096).unwrap();
        assert_eq!(
            (preview.start_address, preview.bytes),
            (0x12342, vec![Some(1), Some(0xff)])
        );
    }

    #[test]
    fn preview_should_use_elf_physical_addresses_and_only_file_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("preview.axf");
        let mut elf = vec![0u8; 124];
        elf[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        elf[16..18].copy_from_slice(&2u16.to_le_bytes());
        elf[18..20].copy_from_slice(&183u16.to_le_bytes());
        elf[20..24].copy_from_slice(&1u32.to_le_bytes());
        elf[32..40].copy_from_slice(&64u64.to_le_bytes());
        elf[52..54].copy_from_slice(&64u16.to_le_bytes());
        elf[54..56].copy_from_slice(&56u16.to_le_bytes());
        elf[56..58].copy_from_slice(&1u16.to_le_bytes());
        elf[64..68].copy_from_slice(&PT_LOAD.0.to_le_bytes());
        elf[72..80].copy_from_slice(&120u64.to_le_bytes());
        elf[80..88].copy_from_slice(&0x2000_0000u64.to_le_bytes());
        elf[88..96].copy_from_slice(&0x0800_0000u64.to_le_bytes());
        elf[96..104].copy_from_slice(&4u64.to_le_bytes());
        elf[104..112].copy_from_slice(&16u64.to_le_bytes());
        elf[120..].copy_from_slice(&[0, 0x41, 0x7f, 0xff]);
        fs::write(&file, elf).unwrap();
        let input = FirmwareInput {
            path: file.display().to_string(),
            format: None,
            base_address: None,
        };
        let preview = read_firmware_preview(&input, 4096).unwrap();
        assert_eq!(
            (preview.start_address, preview.bytes),
            (
                0x0800_0000,
                vec![Some(0), Some(0x41), Some(0x7f), Some(0xff)]
            )
        );
    }

    #[test]
    fn preview_should_reject_unbounded_lengths_before_opening_files() {
        let input = FirmwareInput {
            path: "/missing/preview.bin".to_string(),
            format: None,
            base_address: Some(0),
        };
        for length in [0, 65_537] {
            assert!(matches!(
                read_firmware_preview(&input, length),
                Err(AppError::InvalidUserInput { .. })
            ));
        }
    }

    #[test]
    fn preview_should_read_arm_elf32_load_segments() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("arm.elf");
        let mut elf = vec![0u8; 88];
        elf[..7].copy_from_slice(b"\x7fELF\x01\x01\x01");
        elf[16..18].copy_from_slice(&2u16.to_le_bytes());
        elf[18..20].copy_from_slice(&40u16.to_le_bytes());
        elf[20..24].copy_from_slice(&1u32.to_le_bytes());
        elf[28..32].copy_from_slice(&52u32.to_le_bytes());
        elf[40..42].copy_from_slice(&52u16.to_le_bytes());
        elf[42..44].copy_from_slice(&32u16.to_le_bytes());
        elf[44..46].copy_from_slice(&1u16.to_le_bytes());
        elf[52..56].copy_from_slice(&PT_LOAD.0.to_le_bytes());
        elf[56..60].copy_from_slice(&84u32.to_le_bytes());
        elf[60..64].copy_from_slice(&0x2000_0000u32.to_le_bytes());
        elf[64..68].copy_from_slice(&0x0800_0020u32.to_le_bytes());
        elf[68..72].copy_from_slice(&4u32.to_le_bytes());
        elf[72..76].copy_from_slice(&16u32.to_le_bytes());
        elf[84..].copy_from_slice(&[0xaa, 0x55, 0, 0xff]);
        fs::write(&file, elf).unwrap();
        let input = FirmwareInput {
            path: file.display().to_string(),
            format: None,
            base_address: None,
        };
        let preview = read_firmware_preview(&input, 4096).unwrap();
        assert_eq!(
            (preview.start_address, preview.bytes),
            (
                0x0800_0020,
                vec![Some(0xaa), Some(0x55), Some(0), Some(0xff)]
            )
        );
    }

    #[test]
    fn detect_format_should_accept_elf_extension() {
        let format = detect_format(Path::new("/tmp/app.elf")).unwrap();
        assert_eq!(format, FirmwareFormat::Elf);
    }

    #[test]
    fn detect_format_should_accept_hex_extension() {
        let format = detect_format(Path::new("/tmp/app.hex")).unwrap();
        assert_eq!(format, FirmwareFormat::Hex);
    }

    #[test]
    fn detect_format_should_accept_bin_extension() {
        let format = detect_format(Path::new("/tmp/app.bin")).unwrap();
        assert_eq!(format, FirmwareFormat::Bin);
    }

    #[test]
    fn detect_format_should_accept_uppercase_extension() {
        let format = detect_format(Path::new("/tmp/APP.HEX")).unwrap();
        assert_eq!(format, FirmwareFormat::Hex);
    }

    #[test]
    fn detect_format_should_reject_unsupported_extension() {
        let err = detect_format(Path::new("/tmp/app.txt")).unwrap_err();
        assert!(matches!(err, AppError::UnsupportedFirmwareFormat { .. }));
    }

    #[test]
    fn validate_firmware_should_reject_bin_without_base_address() {
        let input = FirmwareInput {
            path: "/tmp/app.bin".to_string(),
            format: None,
            base_address: None,
        };
        let err = validate_firmware(&input).unwrap_err();
        assert!(matches!(err, AppError::MissingBinBaseAddress));
    }

    #[test]
    fn validate_firmware_should_reject_explicit_format_mismatch() {
        let input = FirmwareInput {
            path: "/tmp/app.bin".to_string(),
            format: Some(FirmwareFormat::Elf),
            base_address: None,
        };
        let err = validate_firmware(&input).unwrap_err();
        assert!(matches!(err, AppError::InvalidUserInput { .. }));
    }

    #[test]
    fn validate_firmware_should_accept_bin_with_zero_base_address() {
        let input = FirmwareInput {
            path: "/tmp/app.bin".to_string(),
            format: None,
            base_address: Some(0),
        };
        let format = validate_firmware(&input).unwrap();
        assert_eq!(format, FirmwareFormat::Bin);
    }

    #[test]
    fn validate_firmware_should_reject_unaligned_bin_base_address() {
        let input = FirmwareInput {
            path: "/tmp/app.bin".to_string(),
            format: None,
            base_address: Some(3),
        };
        let err = validate_firmware(&input).unwrap_err();
        assert!(matches!(err, AppError::InvalidFirmwareAddress { .. }));
    }

    #[test]
    fn validate_firmware_should_ignore_non_bin_base_address() {
        let input = FirmwareInput {
            path: "/tmp/app.elf".to_string(),
            format: None,
            base_address: Some(3),
        };
        let format = validate_firmware(&input).unwrap();
        assert_eq!(format, FirmwareFormat::Elf);
    }

    #[test]
    fn parse_address_should_accept_hex_prefix() {
        let address = parse_address("0x08000000").unwrap();
        assert_eq!(address, 0x0800_0000);
    }

    #[test]
    fn parse_address_should_reject_empty_string() {
        let err = parse_address("").unwrap_err();
        assert!(matches!(err, AppError::InvalidFirmwareAddress { .. }));
    }

    #[test]
    fn analyze_firmware_usage_should_count_bin_bytes_from_base_address() {
        let path = write_temp_file("usage.bin", &[1, 2, 3, 4, 5]);
        let input = FirmwareInput {
            path,
            format: None,
            base_address: Some(0x0800_0000),
        };

        let usage = analyze_firmware_usage(&input).unwrap();

        assert_eq!(usage.used_bytes, 5);
        assert_eq!(usage.start_address, Some(0x0800_0000));
        assert_eq!(usage.end_address, Some(0x0800_0005));
    }

    #[test]
    fn analyze_firmware_usage_should_merge_hex_data_ranges() {
        let hex = b":020000040800F2\n:0400000001020304F2\n:0400040005060708DE\n:00000001FF\n";
        let path = write_temp_file("usage.hex", hex);
        let input = FirmwareInput {
            path,
            format: None,
            base_address: None,
        };

        let usage = analyze_firmware_usage(&input).unwrap();

        assert_eq!(usage.used_bytes, 8);
        assert_eq!(usage.segments.len(), 1);
        assert_eq!(usage.start_address, Some(0x0800_0000));
        assert_eq!(usage.end_address, Some(0x0800_0008));
    }

    #[test]
    fn analyze_bin_should_reject_empty_files() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("empty.bin");
        fs::write(&file, []).unwrap();
        let input = FirmwareInput {
            path: file.display().to_string(),
            format: None,
            base_address: Some(0),
        };
        assert!(matches!(
            analyze_firmware_usage(&input),
            Err(AppError::InvalidUserInput { .. })
        ));
    }

    #[test]
    fn analyze_firmware_usage_should_reject_directories_with_supported_extensions() {
        let directory = tempfile::tempdir().unwrap();
        for extension in ["bin", "hex", "elf"] {
            let file = directory.path().join(format!("directory.{extension}"));
            fs::create_dir(&file).unwrap();
            let input = FirmwareInput {
                path: file.display().to_string(),
                format: None,
                base_address: Some(0),
            };
            let error = analyze_firmware_usage(&input).unwrap_err();
            assert!(
                matches!(error, AppError::InvalidUserInput { .. }),
                "unexpected error for {extension}: {error:?}"
            );
        }
    }

    #[test]
    fn analyze_bin_should_reject_address_overflow_instead_of_saturating() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("overflow.bin");
        fs::write(&file, [1, 2, 3, 4]).unwrap();
        let input = FirmwareInput {
            path: file.display().to_string(),
            format: None,
            base_address: Some(u64::MAX - 3),
        };
        assert!(matches!(
            analyze_firmware_usage(&input),
            Err(AppError::InvalidFirmwareAddress { .. })
        ));
    }

    #[test]
    fn analyze_hex_should_reject_eof_without_data() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("empty.hex");
        fs::write(&file, ":00000001FF\n").unwrap();
        let input = FirmwareInput {
            path: file.display().to_string(),
            format: None,
            base_address: None,
        };
        assert!(matches!(
            analyze_firmware_usage(&input),
            Err(AppError::InvalidUserInput { .. })
        ));
    }

    #[test]
    fn unordered_overlapping_segments_should_count_written_addresses_once() {
        let mut segments = Vec::new();
        push_segment(&mut segments, 8, 8).unwrap();
        push_segment(&mut segments, 0, 12).unwrap();
        push_segment(&mut segments, 0, 4).unwrap();
        merge_segments(&mut segments);
        let usage = usage_from_segments(FirmwareFormat::Hex, segments);
        assert_eq!(usage.used_bytes, 16);
        assert_eq!(usage.segments.len(), 1);
    }

    #[test]
    fn elf_segments_should_reject_address_overflow() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("overflow.elf");
        let mut elf = vec![0u8; 124];
        elf[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        elf[16..18].copy_from_slice(&2u16.to_le_bytes());
        elf[18..20].copy_from_slice(&183u16.to_le_bytes());
        elf[20..24].copy_from_slice(&1u32.to_le_bytes());
        elf[32..40].copy_from_slice(&64u64.to_le_bytes());
        elf[52..54].copy_from_slice(&64u16.to_le_bytes());
        elf[54..56].copy_from_slice(&56u16.to_le_bytes());
        elf[56..58].copy_from_slice(&1u16.to_le_bytes());
        elf[64..68].copy_from_slice(&PT_LOAD.0.to_le_bytes());
        elf[72..80].copy_from_slice(&120u64.to_le_bytes());
        elf[88..96].copy_from_slice(&(u64::MAX - 3).to_le_bytes());
        elf[96..104].copy_from_slice(&4u64.to_le_bytes());
        elf[104..112].copy_from_slice(&4u64.to_le_bytes());
        elf[120..].copy_from_slice(&[1, 2, 3, 4]);
        fs::write(&file, elf).unwrap();
        let input = FirmwareInput {
            path: file.display().to_string(),
            format: None,
            base_address: None,
        };
        assert!(matches!(
            analyze_firmware_usage(&input),
            Err(AppError::InvalidFirmwareAddress { .. })
        ));
    }

    fn write_temp_file(name: &str, data: &[u8]) -> String {
        let path = std::env::temp_dir().join(format!("chipdock-{}-{name}", std::process::id()));
        std::fs::write(&path, data).expect("test fixture should be written");
        path.display().to_string()
    }
}
