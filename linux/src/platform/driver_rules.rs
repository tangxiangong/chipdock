//! USB interface policy mirrors the native client's allowlist; it never authorizes parents.
//!

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    StLink,
    JLink,
    Ftdi,
    WchLink,
    Ch347,
    Glasgow,
    EspJtag,
    CmsisDap,
}

#[derive(Clone, Debug)]
pub struct Identity {
    pub vendor: u16,
    pub product: u16,
    pub interface: Option<u8>,
}

pub fn identity(instance: &str) -> Option<Identity> {
    let parts: Vec<_> = instance.split('\\').collect();
    if parts.len() != 3 || !parts[0].eq_ignore_ascii_case("USB") || parts[2].is_empty() {
        return None;
    }
    let ids: Vec<_> = parts[1].split('&').collect();
    if !(2..=3).contains(&ids.len()) {
        return None;
    }
    fn value(s: &str, prefix: &str) -> Option<u16> {
        let s = s.to_ascii_uppercase();
        u16::from_str_radix(s.strip_prefix(prefix)?, 16).ok()
    }
    Some(Identity {
        vendor: value(ids[0], "VID_")?,
        product: value(ids[1], "PID_")?,
        interface: if ids.len() == 3 {
            Some(value(ids[2], "MI_")?.try_into().ok()?)
        } else {
            None
        },
    })
}

pub fn family(vendor: u16, product: u16) -> Option<Family> {
    match (vendor, product) {
        (
            0x0483,
            0x3748 | 0x374b | 0x374a | 0x3742 | 0x3752 | 0x374e | 0x374f | 0x3753 | 0x3754 | 0x3757,
        ) => Some(Family::StLink),
        (0x1366, p)
            if matches!(
                p,
                0x0101 | 0x0102 | 0x0103 | 0x0104 | 0x0105 | 0x0107 | 0x0108
            ) || (p & 0x1000 != 0 && p & 0x0030 != 0) =>
        {
            Some(Family::JLink)
        }
        (0x0403, 0x6010 | 0x6011 | 0x6014) | (0x15ba, 0x0003 | 0x0004 | 0x002a | 0x002b) => {
            Some(Family::Ftdi)
        }
        (0x1a86, 0x8010) => Some(Family::WchLink),
        (0x1a86, 0x55de | 0x55dd | 0x55e8) => Some(Family::Ch347),
        (0x20b7, 0x9db1) => Some(Family::Glasgow),
        (0x303a, 0x1001) => Some(Family::EspJtag),
        _ => None,
    }
}

pub fn debug_interface(instance: &str, compatible: &[String], description: &str) -> Option<Family> {
    let id = identity(instance)?;
    if compatible.iter().any(|s| {
        let s = s.to_ascii_uppercase();
        s == "USB\\COMPOSITE"
            || ["02", "03", "08", "09", "0A"]
                .iter()
                .any(|c| s.starts_with(&format!("USB\\CLASS_{c}")))
    }) {
        return None;
    }
    let vendor = compatible.iter().any(|s| {
        s.to_ascii_uppercase()
            .starts_with("USB\\CLASS_FF&SUBCLASS_00")
    });
    let family = family(id.vendor, id.product);
    match family {
        Some(Family::StLink)
            if id.interface == Some(0) || (id.interface.is_none() && id.product == 0x3748) =>
        {
            family
        }
        Some(Family::JLink | Family::WchLink)
            if id.interface.is_none() || id.interface == Some(0) =>
        {
            family
        }
        Some(Family::Ftdi)
            if id.interface == Some(0) || (id.interface.is_none() && id.product == 0x6014) =>
        {
            family
        }
        Some(Family::Ch347) if matches!(id.interface, Some(2 | 4)) => family,
        Some(Family::EspJtag) if id.interface == Some(2) => family,
        Some(Family::Glasgow) if vendor && id.interface.is_some() => family,
        None if vendor
            && ["CMSIS-DAP", "CMSIS_DAP", "CMSIS DAP", "CMSISDAP"]
                .iter()
                .any(|s| description.to_ascii_uppercase().contains(s)) =>
        {
            Some(Family::CmsisDap)
        }
        _ => None,
    }
}

pub fn replaceable(family: Family, service: &str) -> bool {
    let service = service.to_ascii_uppercase();
    service.is_empty()
        || ["WINUSB", "LIBUSB0", "LIBUSBK"].contains(&service.as_str())
        || match family {
            Family::StLink => service.starts_with("STTUB"),
            Family::JLink => service == "JLINK",
            Family::Ftdi => service == "FTDIBUS",
            Family::WchLink | Family::Ch347 => {
                ["WCHLINK", "WCHLINKRV", "CH341WDM", "CH347"].contains(&service.as_str())
            }
            _ => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn esp_only_repairs_jtag_child() {
        for n in 0..4 {
            assert_eq!(
                debug_interface(&format!("USB\\VID_303A&PID_1001&MI_{n:02X}\\A"), &[], ""),
                if n == 2 { Some(Family::EspJtag) } else { None }
            );
        }
    }

    #[test]
    fn protected_classes_override_allowlist() {
        for class in ["02", "03", "08", "09", "0A"] {
            assert_eq!(
                debug_interface(
                    "USB\\VID_0483&PID_3748\\A",
                    &[format!("USB\\Class_{class}")],
                    ""
                ),
                None
            );
        }
    }

    #[test]
    fn never_replaces_composite_or_serial_driver() {
        assert!(!replaceable(Family::EspJtag, "usbccgp"));
        assert!(!replaceable(Family::Ftdi, "usbser"));
        assert!(replaceable(Family::StLink, "STTub30"));
    }

    #[test]
    fn ftdi_channel_b_is_not_debug() {
        assert_eq!(
            debug_interface("USB\\VID_0403&PID_6010&MI_01\\A", &[], ""),
            None
        );
    }
}
