mod devices;
mod install;
use super::driver_rules::{self, Family};
use ::windows::Win32::{
    Foundation::*,
    System::Threading::*,
    UI::{Shell::*, WindowsAndMessaging::SW_HIDE},
};
use ::windows::core::PCWSTR;
use anyhow::{Context, Result, ensure};
use chipdock_core::models::ProbeSummary;
use chipdock_service::api::ChipDockCore;
use std::time::Duration;

pub(super) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub(super) fn text(s: &[u16]) -> String {
    String::from_utf16_lossy(&s[..s.iter().position(|c| *c == 0).unwrap_or(s.len())])
}
pub fn merge_probes(mut probes: Vec<ProbeSummary>) -> Result<Vec<ProbeSummary>> {
    let devices = devices::list()?;
    for d in &devices {
        if !d.unique
            || d.family == Family::Glasgow
            || probes
                .iter()
                .any(|p| p.vendor_id == d.id.vendor && p.product_id == d.id.product)
        {
            continue;
        }
        probes.push(ProbeSummary {
            identifier: format!(
                "{:04x}:{:04x}{}",
                d.id.vendor,
                d.id.product,
                if d.family == Family::CmsisDap {
                    d.id.interface.map(|n| format!("-{n}")).unwrap_or_default()
                } else {
                    String::new()
                }
            ),
            vendor_id: d.id.vendor,
            product_id: d.id.product,
            serial_number: None,
            product: Some(format!("{:?}（检查驱动）", d.family)),
        });
    }
    Ok(probes)
}
fn serial_matches(windows: &str, probe: &str, stlink: bool) -> bool {
    windows.eq_ignore_ascii_case(probe)
        || (stlink
            && crate::model::decode_hex(probe)
                .ok()
                .and_then(|b| String::from_utf8(b).ok())
                .is_some_and(|p| windows.eq_ignore_ascii_case(&p)))
}
fn selected(selector: &str) -> Result<Vec<devices::Device>> {
    let parts: Vec<_> = selector.trim().splitn(3, ':').collect();
    ensure!(parts.len() >= 2, "请选择一个探针");
    let vendor = u16::from_str_radix(parts[0], 16)?;
    let product_parts: Vec<_> = parts[1].split('-').collect();
    let product = u16::from_str_radix(product_parts[0], 16)?;
    let interface = product_parts.get(1).map(|n| n.parse::<u8>()).transpose()?;
    let glasgow = driver_rules::family(vendor, product) == Some(Family::Glasgow);
    if glasgow {
        ensure!(
            ChipDockCore::new()
                .list_probes()?
                .iter()
                .any(|p| p.identifier == selector),
            "Glasgow 必须使用 probe-rs 当前枚举出的 applet 接口对"
        );
    }
    let pair = parts
        .get(2)
        .map(|s| s.split(':').collect::<Vec<_>>())
        .unwrap_or_default();
    let serial = pair.first().copied();
    let devices = devices::list()?;
    let mut matches: Vec<_> = devices
        .into_iter()
        .filter(|d| {
            d.id.vendor == vendor
                && d.id.product == product
                && serial.is_none_or(|s| serial_matches(&d.serial, s, d.family == Family::StLink))
        })
        .collect();
    ensure!(!matches.is_empty(), "未找到允许修复的调试接口");
    ensure!(
        serial.is_some() || matches.iter().all(|d| d.unique),
        "无法唯一识别物理设备，请指定序列号"
    );
    let parent = matches[0].parent.clone();
    ensure!(
        matches.iter().all(|d| d.parent == parent),
        "存在多个物理设备"
    );
    if glasgow {
        ensure!(
            pair.len() == 3,
            "Glasgow 必须使用 probe-rs 枚举出的 applet 接口对"
        );
        let input = pair[1].parse::<u8>()?;
        let output = pair[2].parse::<u8>()?;
        ensure!(input != output, "Glasgow 接口对无效");
        matches.retain(|d| d.id.interface == Some(input) || d.id.interface == Some(output));
        ensure!(matches.len() == 2, "Glasgow 接口对已改变");
    } else {
        if let Some(n) = interface {
            matches.retain(|d| d.family == Family::StLink || d.id.interface.unwrap_or(0) == n);
        }
        if matches
            .iter()
            .any(|d| d.family == Family::Ch347 && d.id.interface == Some(4))
        {
            matches.retain(|d| d.id.interface == Some(4));
        }
        ensure!(matches.len() == 1, "调试接口选择不明确");
    }
    Ok(matches)
}
pub fn repair(selector: &str) -> Result<String> {
    let before = selected(selector)?;
    for device in &before {
        ensure!(
            driver_rules::replaceable(device.family, &device.service),
            "所选接口绑定了受保护的驱动"
        );
        // Preflight the inbox driver before asking Windows for elevation.
        install::preflight(device)?;
        let executable = wide(&std::env::current_exe()?.to_string_lossy());
        let encoded = device
            .instance
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let selected = selector
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let params = wide(&format!("--install-probe-driver {encoded} {selected}"));
        let verb = wide("runas");
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpVerb: PCWSTR(verb.as_ptr()),
            lpFile: PCWSTR(executable.as_ptr()),
            lpParameters: PCWSTR(params.as_ptr()),
            nShow: SW_HIDE.0,
            ..Default::default()
        };
        // All strings and the result structure outlive this synchronous OS call.
        unsafe {
            ShellExecuteExW(&mut info)?;
            ensure!(!info.hProcess.is_invalid(), "驱动安装进程未启动");
            WaitForSingleObject(info.hProcess, INFINITE);
            let mut code = 0;
            let result = GetExitCodeProcess(info.hProcess, &mut code);
            let _ = CloseHandle(info.hProcess);
            result?;
            ensure!(code == 0, "驱动安装未成功或需要重启，请查看 Windows 提示");
        }
    }
    let mut refreshed = Vec::new();
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(250));
        refreshed = ChipDockCore::new().list_probes()?;
        if refreshed
            .iter()
            .any(|p| p.vendor_id == before[0].id.vendor && p.product_id == before[0].id.product)
        {
            break;
        }
    }
    let after = selected(selector)?;
    ensure!(
        before
            .iter()
            .map(|d| &d.instance)
            .eq(after.iter().map(|d| &d.instance)),
        "驱动修复期间设备身份发生改变"
    );
    let candidates: Vec<_> = refreshed
        .iter()
        .filter(|p| {
            p.vendor_id == before[0].id.vendor
                && p.product_id == before[0].id.product
                && p.serial_number.as_deref().is_none_or(|s| {
                    serial_matches(
                        &before[0].serial,
                        s.split(':').next().unwrap_or(s),
                        before[0].family == Family::StLink,
                    )
                })
        })
        .collect();
    ensure!(
        candidates.len() == 1,
        "驱动修复后 probe-rs 未能唯一识别设备，请重新扫描"
    );
    Ok(candidates[0].identifier.clone())
}
pub fn install_from_arguments() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(2).collect();
    ensure!(args.len() == 2, "驱动安装参数无效");
    let selector = String::from_utf8(crate::model::decode_hex(&args[1])?)?;
    let instance = String::from_utf8(crate::model::decode_hex(&args[0])?)?;
    let mut process = ::windows::Win32::System::SystemInformation::IMAGE_FILE_MACHINE::default();
    let mut native = ::windows::Win32::System::SystemInformation::IMAGE_FILE_MACHINE::default();
    unsafe {
        IsWow64Process2(GetCurrentProcess(), &mut process, Some(&mut native))?;
    }
    ensure!(process.0 == 0, "驱动安装需要与 Windows 系统架构一致的程序");
    let device = selected(&selector)?
        .into_iter()
        .find(|d| d.instance == instance)
        .context("所选调试接口已断开")?;
    ensure!(
        driver_rules::replaceable(device.family, &device.service),
        "拒绝更换受保护驱动"
    );
    install::install(&device)
}
