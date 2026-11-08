use super::{
    devices::{Device, DeviceSet},
    *,
};
use windows::{
    Win32::{Devices::DeviceAndDriverInstallation::*, System::Registry::*},
    core::{BOOL, PCWSTR},
};

fn build_driver_list(set: &DeviceSet, info: &mut SP_DEVINFO_DATA) -> Result<()> {
    let mut params = SP_DEVINSTALL_PARAMS_W {
        cbSize: std::mem::size_of::<SP_DEVINSTALL_PARAMS_W>() as u32,
        ..Default::default()
    };
    unsafe {
        SetupDiGetDeviceInstallParamsW(set.0, Some(info), &mut params)?;
    }
    params.Flags |= DI_ENUMSINGLEINF | DI_QUIETINSTALL;
    params.FlagsEx |= DI_FLAGSEX_ALLOWEXCLUDEDDRVS;
    let mut directory = [0u16; 260];
    let length = unsafe {
        ::windows::Win32::System::SystemInformation::GetWindowsDirectoryW(Some(&mut directory))
    };
    ensure!(
        length > 0 && (length as usize) < directory.len(),
        "无法定位 Windows 系统目录"
    );
    let path = wide(&format!("{}\\INF\\winusb.inf", text(&directory)));
    ensure!(path.len() <= params.DriverPath.len(), "系统路径过长");
    params.DriverPath[..path.len()].copy_from_slice(&path);
    unsafe {
        SetupDiSetDeviceInstallParamsW(set.0, Some(info), &params)?;
        SetupDiBuildDriverInfoList(set.0, Some(info), SPDIT_CLASSDRIVER)?;
    }
    Ok(())
}
fn find_driver(set: &DeviceSet, info: &SP_DEVINFO_DATA) -> Result<SP_DRVINFO_DATA_V2_W> {
    for index in 0.. {
        let mut driver = SP_DRVINFO_DATA_V2_W {
            cbSize: std::mem::size_of::<SP_DRVINFO_DATA_V2_W>() as u32,
            ..Default::default()
        };
        if let Err(error) = unsafe {
            SetupDiEnumDriverInfoW(set.0, Some(info), SPDIT_CLASSDRIVER, index, &mut driver)
        } {
            if error.code() == ERROR_NO_MORE_ITEMS.to_hresult() {
                break;
            }
            return Err(error.into());
        }
        let mut detail = SP_DRVINFO_DETAIL_DATA_W {
            cbSize: std::mem::size_of::<SP_DRVINFO_DETAIL_DATA_W>() as u32,
            ..Default::default()
        };
        if let Err(error) = unsafe {
            SetupDiGetDriverInfoDetailW(
                set.0,
                Some(info),
                &driver,
                Some(&mut detail),
                std::mem::size_of::<SP_DRVINFO_DETAIL_DATA_W>() as u32,
                None,
            )
        } {
            if error.code() != ERROR_INSUFFICIENT_BUFFER.to_hresult() {
                return Err(error.into());
            }
        }
        if text(&detail.SectionName)
            .split('.')
            .next()
            .is_some_and(|s| s.eq_ignore_ascii_case("WINUSB"))
        {
            return Ok(driver);
        }
    }
    anyhow::bail!("找不到 Windows 内置 WinUSB 驱动")
}
pub(super) fn preflight(device: &Device) -> Result<()> {
    if device.service.eq_ignore_ascii_case("WinUSB") {
        return Ok(());
    }
    let set = DeviceSet::new()?;
    let mut info = set.open(&device.instance)?;
    build_driver_list(&set, &mut info)?;
    let result = find_driver(&set, &info).map(|_| ());
    unsafe {
        let _ = SetupDiDestroyDriverInfoList(set.0, Some(&info), SPDIT_CLASSDRIVER);
    }
    result
}
struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}
fn values(key: HKEY, name: &str) -> Result<Vec<String>> {
    let mut buffer = [0u8; 8192];
    let mut size = 8192;
    let mut kind = REG_VALUE_TYPE::default();
    let code = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(wide(name).as_ptr()),
            None,
            Some(&mut kind),
            Some(buffer.as_mut_ptr()),
            Some(&mut size),
        )
    };
    if code == ERROR_FILE_NOT_FOUND {
        return Ok(vec![]);
    }
    code.ok()?;
    ensure!(
        kind == REG_SZ || kind == REG_MULTI_SZ,
        "设备接口 GUID 类型无效"
    );
    Ok(devices::strings(&buffer[..size as usize]))
}
fn add_guid(set: &DeviceSet, info: &SP_DEVINFO_DATA, family: Family) -> Result<()> {
    let key = Key(unsafe {
        SetupDiCreateDevRegKeyW(
            set.0,
            info,
            DICS_FLAG_GLOBAL.0,
            0,
            DIREG_DEV,
            None,
            PCWSTR::null(),
        )
    }?);
    let guid = match family {
        Family::StLink => "{DBCE1CD9-A320-4B51-A365-A0C3F3C5FB29}",
        Family::CmsisDap => "{CDB3B5AD-293B-4663-AA36-1AAE46463776}",
        _ => "{D8733B38-1A21-4CA1-AF1F-A4F66CE27351}",
    };
    let mut guids = vec![guid.to_owned()];
    for value in values(key.0, "DeviceInterfaceGUIDs")?
        .into_iter()
        .chain(values(key.0, "DeviceInterfaceGUID")?)
    {
        if let Ok(value) = uuid::Uuid::parse_str(&value)
            && !value.is_nil()
        {
            let value = format!("{{{value}}}");
            if !guids.iter().any(|s| s.eq_ignore_ascii_case(&value)) {
                guids.push(value);
            }
        }
    }
    let data: Vec<u8> = guids
        .iter()
        .flat_map(|s| wide(s))
        .chain(Some(0))
        .flat_map(u16::to_le_bytes)
        .collect();
    unsafe {
        RegSetValueExW(
            key.0,
            PCWSTR(wide("DeviceInterfaceGUIDs").as_ptr()),
            None,
            REG_MULTI_SZ,
            Some(&data),
        )
    }
    .ok()?;
    Ok(())
}
pub(super) fn install(device: &Device) -> Result<()> {
    let set = DeviceSet::new()?;
    let mut info = set.open(&device.instance)?;
    let current = set
        .strings(&info, SPDRP_SERVICE)?
        .into_iter()
        .next()
        .unwrap_or_default();
    ensure!(
        current.eq_ignore_ascii_case(&device.service),
        "驱动状态已改变，请重新扫描"
    );
    if !current.eq_ignore_ascii_case("WinUSB") {
        build_driver_list(&set, &mut info)?;
        let result = (|| -> Result<()> {
            let mut driver = find_driver(&set, &info)?;
            unsafe {
                SetupDiSetSelectedDriverW(set.0, Some(&mut info), Some(&mut driver))?;
            }
            add_guid(&set, &info, device.family)?;
            let mut reboot = BOOL::default();
            unsafe {
                DiInstallDevice(
                    None,
                    set.0,
                    &info,
                    Some(&driver),
                    DIINSTALLDEVICE_FLAGS(0),
                    Some(&mut reboot),
                )?;
            }
            ensure!(!reboot.as_bool(), "驱动已安装，请重新启动 Windows 后连接");
            Ok(())
        })();
        unsafe {
            let _ = SetupDiDestroyDriverInfoList(set.0, Some(&info), SPDIT_CLASSDRIVER);
        }
        result?;
    } else {
        add_guid(&set, &info, device.family)?;
    }
    let change = SP_PROPCHANGE_PARAMS {
        ClassInstallHeader: SP_CLASSINSTALL_HEADER {
            cbSize: std::mem::size_of::<SP_CLASSINSTALL_HEADER>() as u32,
            InstallFunction: DIF_PROPERTYCHANGE,
        },
        StateChange: DICS_PROPCHANGE,
        Scope: DICS_FLAG_CONFIGSPECIFIC,
        HwProfile: 0,
    };
    unsafe {
        SetupDiSetClassInstallParamsW(
            set.0,
            Some(&info),
            Some(&change.ClassInstallHeader),
            std::mem::size_of::<SP_PROPCHANGE_PARAMS>() as u32,
        )?;
        SetupDiCallClassInstaller(DIF_PROPERTYCHANGE, set.0, Some(&info))?;
    }
    Ok(())
}
