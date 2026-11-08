use super::*;
use std::collections::{BTreeMap, BTreeSet};
use windows::{
    Win32::Devices::{DeviceAndDriverInstallation::*, Properties::*},
    core::{GUID, PCWSTR},
};
#[derive(Clone)]
pub(super) struct Device {
    pub instance: String,
    pub parent: String,
    pub id: driver_rules::Identity,
    pub family: Family,
    pub service: String,
    pub serial: String,
    pub unique: bool,
}
pub(super) struct DeviceSet(pub HDEVINFO);
impl Drop for DeviceSet {
    fn drop(&mut self) {
        unsafe {
            let _ = SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}
pub(super) fn strings(bytes: &[u8]) -> Vec<String> {
    let units: Vec<_> = bytes
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    units
        .split(|c| *c == 0)
        .filter(|s| !s.is_empty())
        .map(String::from_utf16_lossy)
        .collect()
}
impl DeviceSet {
    pub fn new() -> Result<Self> {
        let usb = wide("USB");
        Ok(Self(unsafe {
            SetupDiGetClassDevsW(
                None,
                PCWSTR(usb.as_ptr()),
                None,
                DIGCF_PRESENT | DIGCF_ALLCLASSES,
            )
        }?))
    }
    pub fn open(&self, instance: &str) -> Result<SP_DEVINFO_DATA> {
        let mut info = SP_DEVINFO_DATA {
            cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
            ..Default::default()
        };
        unsafe {
            SetupDiOpenDeviceInfoW(
                self.0,
                PCWSTR(wide(instance).as_ptr()),
                None,
                0,
                Some(&mut info),
            )?;
        }
        Ok(info)
    }
    pub fn strings(
        &self,
        info: &SP_DEVINFO_DATA,
        property: SETUP_DI_REGISTRY_PROPERTY,
    ) -> Result<Vec<String>> {
        let mut buffer = [0u8; 8192];
        let mut size = 0;
        match unsafe {
            SetupDiGetDeviceRegistryPropertyW(
                self.0,
                info,
                property,
                None,
                Some(&mut buffer),
                Some(&mut size),
            )
        } {
            Ok(()) => Ok(strings(&buffer[..size as usize])),
            Err(error) if error.code() == ERROR_INVALID_DATA.to_hresult() => Ok(vec![]),
            Err(error) => Err(error.into()),
        }
    }
}
fn instance(dev: u32) -> Result<String> {
    let mut buffer = [0u16; 1024];
    let code = unsafe { CM_Get_Device_IDW(dev, &mut buffer, 0) };
    ensure!(code == CR_SUCCESS, "无法读取设备标识: {code:?}");
    Ok(text(&buffer))
}
pub(super) fn list() -> Result<Vec<Device>> {
    let set = DeviceSet::new()?;
    let mut result = Vec::new();
    let mut physical: BTreeMap<(u16, u16), BTreeSet<String>> = BTreeMap::new();
    for index in 0.. {
        let mut info = SP_DEVINFO_DATA {
            cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
            ..Default::default()
        };
        if let Err(error) = unsafe { SetupDiEnumDeviceInfo(set.0, index, &mut info) } {
            if error.code() == ERROR_NO_MORE_ITEMS.to_hresult() {
                break;
            }
            return Err(error.into());
        }
        let instance = instance(info.DevInst)?;
        let Some(id) = driver_rules::identity(&instance) else {
            continue;
        };
        if id.interface.is_none() {
            physical
                .entry((id.vendor, id.product))
                .or_default()
                .insert(instance.clone());
        }
        let mut description = [0u8; 8192];
        let mut length = 8192;
        let mut property_type = DEVPROPTYPE::default();
        let key = DEVPROPKEY {
            fmtid: GUID::from_u128(0x540b947e_8b40_45bc_a8a2_6a0b894cbda2),
            pid: 4,
        };
        let code = unsafe {
            CM_Get_DevNode_PropertyW(
                info.DevInst,
                &key,
                &mut property_type,
                Some(description.as_mut_ptr()),
                &mut length,
                0,
            )
        };
        let description = if code == CR_SUCCESS && property_type == DEVPROP_TYPE_STRING {
            strings(&description[..length as usize])
                .into_iter()
                .next()
                .unwrap_or_default()
        } else {
            String::new()
        };
        let Some(family) = driver_rules::debug_interface(
            &instance,
            &set.strings(&info, SPDRP_COMPATIBLEIDS)?,
            &description,
        ) else {
            continue;
        };
        let parent = if id.interface.is_some() {
            let mut parent = 0;
            ensure!(
                unsafe { CM_Get_Parent(&mut parent, info.DevInst, 0) } == CR_SUCCESS,
                "无法读取物理设备身份"
            );
            self::instance(parent)?
        } else {
            instance.clone()
        };
        physical
            .entry((id.vendor, id.product))
            .or_default()
            .insert(parent.clone());
        let service = set
            .strings(&info, SPDRP_SERVICE)?
            .into_iter()
            .next()
            .unwrap_or_default();
        if service.eq_ignore_ascii_case("usbccgp") {
            continue;
        }
        let serial = parent.rsplit('\\').next().unwrap_or("").to_owned();
        result.push(Device {
            instance,
            parent,
            id,
            family,
            service,
            serial,
            unique: false,
        });
    }
    for device in &mut result {
        device.unique = physical
            .get(&(device.id.vendor, device.id.product))
            .is_some_and(|s| s.len() == 1);
    }
    result.sort_by(|a, b| a.instance.cmp(&b.instance));
    Ok(result)
}
