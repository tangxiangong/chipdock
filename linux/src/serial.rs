use crate::model::{self, Preferences};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

pub fn request(p: &Preferences, action: &str) -> Result<Value> {
    let data = |key: &str| -> Result<Value> {
        serde_json::from_str(p.get(key)).with_context(|| format!("{key} JSON 无效"))
    };
    let value = match action {
        "repeat" => {
            json!({"action":"startJob","job":{"kind":"repeat","message":{"text":p.get("send"),"format":p.get("format"),"ending":p.get("ending"),"checksum":p.get("checksum"),"options":crate::backend::send_options(p)?},"intervalMs":p.number("interval")?}})
        }
        "batch" => {
            json!({"action":"startJob","job":{"kind":"batch","packets":data("packets")?,"selected":p.get("selected").split(',').map(str::trim).filter(|s|!s.is_empty()).collect::<Vec<_>>(),"intervalMs":p.number("interval")?}})
        }
        "macro" => {
            json!({"action":"startJob","job":{"kind":"macro","steps":data("macro")?,"cycles":p.number("cycles")?}})
        }
        "file" => {
            json!({"action":"startJob","job":{"kind":"file","path":p.get("file"),"chunkBytes":p.number("chunk")?,"intervalMs":p.number("interval")?}})
        }
        "replies" | "protocol" => json!({"action":action,"rules":data(action)?}),
        "command" => json!({"action":"command","input":p.get("command")}),
        "validatePackets" => json!({"action":action,"packets":data("packets")?}),
        "checksum" => {
            json!({"action":action,"input":p.get("checksum_input"),"format":p.get("format"),"algorithm":p.get("checksum"),"options":crate::backend::send_options(p)?})
        }
        "framing" => {
            let mode = p.get("frame_mode");
            let mut config = json!({"mode":mode});
            match mode {
                "idle" => config["ms"] = json!(p.number("frame_value")?),
                "fixed" => config["bytes"] = json!(p.number("frame_value")?),
                "header" | "tail" => config["hex"] = json!(p.get("frame_value")),
                _ => {}
            }
            json!({"action":action,"config":config})
        }
        "modbus" | "modbusSend" => {
            json!({"action":"modbus","request":{"station":p.number("station")?,"function":p.number("function")?,"address":p.number("register")?,"quantity":p.number("quantity")?,"values":p.get("values").split(',').filter(|s|!s.trim().is_empty()).map(model::number).collect::<Result<Vec<_>>>()?},"timeout_ms":if action=="modbusSend" {p.number("timeout")?}else{0}})
        }
        "capture" | "raw" => {
            json!({"action":"capture","raw":action=="raw","settings":{"path":p.get("capture_path"),"format":p.get("capture_format"),"rollBytes":capture_roll_bytes(p)?}})
        }
        "stopCapture" | "stopRaw" => {
            json!({"action":"capture","raw":action=="stopRaw","settings":null})
        }
        "export" => {
            json!({"action":action,"path":p.get("export"),"format":p.get("capture_format")})
        }
        "exportMacro" => json!({"action":action,"path":p.get("export")}),
        "cancelJob" | "resetCounters" => json!({"action":action}),
        _ => bail!("未知操作"),
    };
    Ok(value)
}

pub fn search(p: &Preferences) -> Result<regex::Regex> {
    let pattern = if p.flag("search_regex") {
        p.get("filter").to_owned()
    } else {
        regex::escape(p.get("filter"))
    };
    Ok(regex::RegexBuilder::new(&pattern)
        .case_insensitive(!p.flag("search_case"))
        .size_limit(1024 * 1024)
        .build()?)
}
pub fn frame_text<'a>(
    frame: &'a chipdock_core::serial::SerialFrameEvent,
    encoding: &str,
) -> &'a str {
    match encoding {
        "hex" => &frame.hex,
        "ascii" => &frame.text.ascii,
        "gbk" => &frame.text.gbk,
        "latin-1" => &frame.text.latin1,
        _ => &frame.text.utf8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checksum_uses_the_shared_serial_engine() {
        let mut p = Preferences::default();
        p.fields.insert("checksum_input".into(), "01 02 FF".into());
        p.fields.insert("send".into(), "unrelated draft".into());
        p.fields.insert("format".into(), "hex".into());
        p.fields.insert("checksum".into(), "sum8".into());
        let id = chipdock_service::serial::serial_create_session().unwrap();
        let value = chipdock_service::serial::serial_workbench(
            id,
            &request(&p, "checksum").unwrap().to_string(),
        )
        .unwrap();
        chipdock_service::serial::serial_close_session(id).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&value).unwrap()["hex"], "02");
    }
    #[test]
    fn invalid_framing_is_rejected_before_dispatch() {
        let mut p = Preferences::default();
        p.fields.insert("frame_mode".into(), "fixed".into());
        p.fields.insert("frame_value".into(), "-1".into());
        assert!(request(&p, "framing").is_err());
    }
    #[test]
    fn serial_widths_cannot_wrap() {
        let mut p = Preferences::default();
        p.fields.insert("baud".into(), "4294967296".into());
        assert!(crate::backend::serial_config(&p).is_err());
    }
    #[test]
    fn saved_protocols_have_valid_default_json() {
        let p = Preferences::default();
        assert!(request(&p, "protocol").is_ok());
        assert!(request(&p, "replies").is_ok());
    }
}

#[cfg(test)]
mod modbus_tests {
    use super::*;
    #[test]
    fn send_and_wait_must_not_silently_become_an_encode_only_request() {
        let p = Preferences::default();
        let value = request(&p, "modbusSend").unwrap();
        assert_eq!(value["timeout_ms"], 1000);
        let id = chipdock_service::serial::serial_create_session().unwrap();
        let result = chipdock_service::serial::serial_workbench(id, &value.to_string());
        chipdock_service::serial::serial_close_session(id).unwrap();
        assert!(result.unwrap_err().to_string().contains("未连接"));
    }
}

fn capture_roll_bytes(p: &Preferences) -> Result<u64> {
    let Some(mib) = p.fields.get("roll_mib") else {
        return p.number("roll");
    };
    let value = mib.parse::<f64>().context("轮转大小必须是 MiB 数值")?;
    let bytes = value * 1024. * 1024.;
    anyhow::ensure!(
        bytes.is_finite() && bytes >= 0. && bytes < (u64::MAX as f64),
        "采集轮转大小超出范围"
    );
    Ok(bytes.round() as u64)
}
#[cfg(test)]
mod capture_size_tests {
    use super::*;
    #[test]
    fn capture_units_preserve_fractional_mib_and_legacy_bytes() {
        let mut p = Preferences::default();
        p.fields.insert("roll_mib".into(), "1.5".into());
        assert_eq!(capture_roll_bytes(&p).unwrap(), 1_572_864);
        p.fields.remove("roll_mib");
        p.fields.insert("roll".into(), "1000001".into());
        assert_eq!(capture_roll_bytes(&p).unwrap(), 1_000_001);
        for invalid in ["NaN", "inf", "-1", "1e99"] {
            p.fields.insert("roll_mib".into(), invalid.into());
            assert!(capture_roll_bytes(&p).is_err());
        }
    }
}
