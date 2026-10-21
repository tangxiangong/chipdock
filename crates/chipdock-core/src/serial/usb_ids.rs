use std::{collections::HashMap, sync::LazyLock};

use serde::Deserialize;

#[derive(Deserialize)]
struct Vendor {
    name: String,
    devices: HashMap<String, Product>,
}

#[derive(Deserialize)]
struct Product {
    devname: String,
}

// Keep one offline catalog in the shared core so both native clients resolve
// the same IDs without a separate resource copy or runtime filesystem access.
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../usb-device.json"
));
static CATALOG: LazyLock<Result<HashMap<String, Vendor>, serde_json::Error>> =
    LazyLock::new(|| serde_json::from_str(SOURCE));

pub(super) fn names(
    vid: u16,
    pid: u16,
    manufacturer: Option<String>,
    product: Option<String>,
) -> (Option<String>, Option<String>) {
    let manufacturer = manufacturer.filter(|name| !name.trim().is_empty());
    let product = product.filter(|name| !name.trim().is_empty());
    if manufacturer.is_some() && product.is_some() {
        return (manufacturer, product);
    }
    // Device discovery must still work if a future catalog edit is malformed.
    let Ok(catalog) = &*CATALOG else {
        return (manufacturer, product);
    };
    let Some(vendor) = catalog.get(&format!("{vid:04x}")) else {
        return (manufacturer, product);
    };
    let manufacturer = manufacturer.or_else(|| label(&vendor.name));
    let product = product.or_else(|| {
        vendor
            .devices
            .get(&format!("{pid:04x}"))
            .and_then(|device| label(&device.devname))
    });
    (manufacturer, product)
}

fn label(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    // The bundled catalog stores these HTML escapes in otherwise plain labels.
    Some(
        value
            .replace("&#039;", "'")
            .replace("&#39;", "'")
            .replace("&quot;", "\"")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_is_valid_and_resolves_common_serial_adapters() {
        assert!(CATALOG.is_ok());
        for (vid, pid, manufacturer, product) in [
            (
                0x1a86,
                0x7523,
                "QinHeng Electronics",
                "CH340 serial converter",
            ),
            (
                0x0403,
                0x6001,
                "Future Technology Devices International, Ltd",
                "FT232 Serial (UART) IC",
            ),
            (0x10c4, 0xea60, "Silicon Labs", "CP210x UART Bridge"),
        ] {
            let result = names(vid, pid, None, None);
            assert_eq!(result, (Some(manufacturer.into()), Some(product.into())));
        }
    }

    #[test]
    fn system_metadata_takes_precedence_and_blank_fields_use_the_catalog() {
        let result = names(
            0x1a86,
            0x7523,
            Some("My vendor".into()),
            Some("My device".into()),
        );
        assert_eq!(result, (Some("My vendor".into()), Some("My device".into())));
        let result = names(0x1a86, 0x7523, Some(" \t".into()), Some(String::new()));
        assert_eq!(
            result,
            (
                Some("QinHeng Electronics".into()),
                Some("CH340 serial converter".into())
            )
        );
    }

    #[test]
    fn unknown_ids_preserve_available_metadata_and_vendor_only_matches() {
        assert_eq!(names(0xffff, 0xffff, None, None), (None, None));
        assert_eq!(
            names(0xffff, 0xffff, Some("Custom".into()), None),
            (Some("Custom".into()), None)
        );
        assert_eq!(
            names(0x1001, 0xffff, None, None),
            (Some("Ritronics Components (S) Pte., Ltd".into()), None)
        );
    }

    #[test]
    fn catalog_labels_render_html_escapes_as_plain_text() {
        assert_eq!(
            label("A &amp; B&#039;s &quot;UART&quot; &gt; 0"),
            Some("A & B's \"UART\" > 0".into())
        );
        assert_eq!(label(" &amp;quot; "), Some("&quot;".into()));
        assert_eq!(label("  "), None);
    }
}
