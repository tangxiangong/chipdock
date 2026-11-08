use anyhow::Result;
use chipdock_core::models::ProbeSummary;
use chipdock_service::api::ChipDockCore;
#[cfg(any(windows, test))]
mod driver_rules;
#[cfg(windows)]
mod windows;

pub fn list_probes(core: &ChipDockCore) -> Result<Vec<ProbeSummary>> {
    let probes = core.list_probes()?;
    #[cfg(windows)]
    {
        windows::merge_probes(probes)
    }
    #[cfg(not(windows))]
    {
        Ok(probes)
    }
}

pub fn repair_probe(selector: &str) -> Result<String> {
    #[cfg(windows)]
    {
        windows::repair(selector)
    }
    #[cfg(not(windows))]
    {
        let _ = selector;
        anyhow::bail!("驱动修复仅适用于 Windows；Linux 请配置 udev 设备权限")
    }
}

pub fn handle_driver_install() -> Option<Result<()>> {
    if std::env::args().nth(1).as_deref() != Some("--install-probe-driver") {
        return None;
    }
    #[cfg(windows)]
    {
        Some(windows::install_from_arguments())
    }
    #[cfg(not(windows))]
    {
        Some(Err(anyhow::anyhow!("当前平台不支持 WinUSB 驱动安装")))
    }
}
