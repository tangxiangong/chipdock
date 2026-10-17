use crate::{
    error::{AppError, Result},
    models::{MemoryAccessInfo, MemoryRegionKind, MemoryRegionLayout, TargetCandidate},
};
use probe_rs::architecture::arm::ArmChipInfo;
use probe_rs::config::{MemoryRegion, Registry};
use std::collections::HashSet;

const DEFAULT_CHIP_LIMIT: usize = 20;
const MAX_CHIP_LIMIT: usize = 100;

/// 根据关键字搜索 probe-rs 内置芯片型号。
pub fn search_chips(query: &str, limit: usize) -> Result<Vec<String>> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidUserInput {
            detail: "芯片搜索关键词不能为空".to_string(),
        });
    }

    let limit = normalized_limit(limit);
    let chips = Registry::from_builtin_families().search_chips(trimmed);

    Ok(dedupe_chip_names(chips).into_iter().take(limit).collect())
}

/// 解析目标芯片型号；为空时返回用户输入错误。
pub fn require_chip(chip: Option<&str>) -> Result<String> {
    chip.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| AppError::InvalidUserInput {
            detail: "未选择芯片；自动识别失败时必须手动选择 chip".to_string(),
        })
}

/// 查询指定芯片型号的内存布局。
pub fn target_memory_map(chip: &str) -> Result<Vec<MemoryRegionLayout>> {
    let chip = require_chip(Some(chip))?;
    let target = Registry::from_builtin_families()
        .get_target_by_name(&chip)
        .map_err(|err| AppError::InvalidUserInput {
            detail: err.to_string(),
        })?;

    Ok(target
        .memory_map
        .into_iter()
        .map(memory_region_layout)
        .collect())
}

fn normalized_limit(limit: usize) -> usize {
    if limit == 0 {
        DEFAULT_CHIP_LIMIT
    } else {
        limit.min(MAX_CHIP_LIMIT)
    }
}

fn dedupe_chip_names(chips: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    chips
        .into_iter()
        .filter(|chip| seen.insert(chip.clone()))
        .collect()
}

pub(crate) fn arm_target_candidates(
    registry: &Registry,
    chip_info: ArmChipInfo,
) -> Vec<TargetCandidate> {
    let mut seen = HashSet::new();
    registry
        .families()
        .iter()
        .filter(|family| family.manufacturer == Some(chip_info.manufacturer))
        .flat_map(|family| {
            family
                .variants
                .iter()
                .filter(move |variant| variant.part == Some(chip_info.part))
                .map(move |variant| TargetCandidate {
                    name: variant.name.clone(),
                    family: family.name.clone(),
                })
        })
        .filter(|candidate| seen.insert(candidate.name.clone()))
        .collect()
}

fn memory_region_layout(region: MemoryRegion) -> MemoryRegionLayout {
    let range = region.address_range();
    let size = range.end.saturating_sub(range.start);

    match region {
        MemoryRegion::Nvm(region) => {
            let access = access_info(region.access());
            MemoryRegionLayout {
                name: region.name,
                kind: MemoryRegionKind::Nvm,
                start: range.start,
                end: range.end,
                size,
                cores: region.cores,
                is_alias: region.is_alias,
                access,
            }
        }
        MemoryRegion::Ram(region) => {
            let access = access_info(region.access());
            MemoryRegionLayout {
                name: region.name,
                kind: MemoryRegionKind::Ram,
                start: range.start,
                end: range.end,
                size,
                cores: region.cores,
                is_alias: false,
                access,
            }
        }
        MemoryRegion::Generic(region) => {
            let access = access_info(region.access());
            MemoryRegionLayout {
                name: region.name,
                kind: MemoryRegionKind::Generic,
                start: range.start,
                end: range.end,
                size,
                cores: region.cores,
                is_alias: false,
                access,
            }
        }
    }
}

fn access_info(access: probe_rs::config::MemoryAccess) -> MemoryAccessInfo {
    MemoryAccessInfo {
        read: access.read,
        write: access.write,
        execute: access.execute,
        boot: access.boot,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;
    use probe_rs::architecture::arm::ArmChipInfo;

    #[test]
    fn require_chip_should_return_explicit_chip() {
        let chip = require_chip(Some("STM32F103C8Tx")).expect("explicit chip should be valid");

        assert_eq!(chip, "STM32F103C8Tx");
    }

    #[test]
    fn require_chip_should_reject_blank_chip() {
        let err = require_chip(Some("   ")).expect_err("blank chip should be rejected");

        assert!(matches!(err, AppError::InvalidUserInput { .. }));
    }

    #[test]
    fn search_chips_should_reject_blank_query() {
        let err = search_chips("   ", 10).expect_err("blank query should be rejected");

        assert!(matches!(err, AppError::InvalidUserInput { .. }));
    }

    #[test]
    fn search_chips_should_respect_limit_for_builtin_registry() {
        let chips = search_chips("STM32F103", 3).expect("built-in registry search should work");

        assert!(chips.len() <= 3);
    }

    #[test]
    fn search_chips_should_use_default_limit_when_limit_is_zero() {
        let chips = search_chips("STM32F103", 0).expect("built-in registry search should work");

        assert!(chips.len() <= DEFAULT_CHIP_LIMIT);
    }

    #[test]
    fn search_chips_should_clamp_huge_limit() {
        let chips =
            search_chips("STM32", usize::MAX).expect("built-in registry search should work");

        assert!(chips.len() <= MAX_CHIP_LIMIT);
    }

    #[test]
    fn target_memory_map_should_return_known_chip_regions() {
        let regions =
            target_memory_map("STM32F103C8").expect("built-in STM32F103C8 target should exist");

        assert!(
            regions
                .iter()
                .any(|region| region.kind == MemoryRegionKind::Nvm)
        );
        assert!(
            regions
                .iter()
                .any(|region| region.kind == MemoryRegionKind::Ram)
        );
    }

    #[test]
    fn dedupe_chip_names_should_preserve_first_occurrence_order() {
        let chips = dedupe_chip_names(vec![
            "STM32F103C8".to_string(),
            "STM32F103RB".to_string(),
            "STM32F103C8".to_string(),
            "STM32F103ZE".to_string(),
        ]);

        assert_eq!(
            chips,
            vec![
                "STM32F103C8".to_string(),
                "STM32F103RB".to_string(),
                "STM32F103ZE".to_string()
            ]
        );
    }

    #[test]
    fn arm_target_candidates_should_match_same_manufacturer_and_part() {
        let registry = Registry::from_builtin_families();
        let chip_info = first_arm_chip_info(&registry);

        let candidates = arm_target_candidates(&registry, chip_info);

        assert!(!candidates.is_empty(), "expected narrowed candidates");
    }

    #[test]
    fn arm_target_candidates_should_exclude_other_parts() {
        let registry = Registry::from_builtin_families();
        let chip_info = first_arm_chip_info(&registry);

        let candidates = arm_target_candidates(&registry, chip_info);

        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.name != "Generic ARMv7-M"),
            "generic targets without matching chip part must not be included",
        );
    }

    fn first_arm_chip_info(registry: &Registry) -> ArmChipInfo {
        for family in registry
            .families()
            .iter()
            .filter(|family| family.manufacturer.is_some())
        {
            for variant in &family.variants {
                let Some(part) = variant.part else {
                    continue;
                };
                return ArmChipInfo {
                    manufacturer: family
                        .manufacturer
                        .expect("test filtered families with manufacturers"),
                    part,
                };
            }
        }

        panic!("builtin registry should contain at least one ARM part");
    }
}
