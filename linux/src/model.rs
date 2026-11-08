use anyhow::{Context, Result, bail};
use chipdock_core::models::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub fields: BTreeMap<String, String>,
    pub flags: BTreeMap<String, bool>,
    pub profiles: BTreeMap<String, Profile>,
    pub recent: Vec<String>,
    pub history: Vec<String>,
    pub command_history: Vec<String>,
    pub recent_bases: BTreeMap<String, String>,
    pub theme: String,
    pub ipc: u8,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Profile {
    pub target: TargetSelection,
    pub probe: Option<String>,
    pub options: FlashOptions,
    pub base: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            fields: FIELDS
                .iter()
                .chain(EDITORS.iter())
                .map(|(key, _, value)| (key.to_string(), value.to_string()))
                .collect(),
            flags: [
                ("verify", true),
                ("default_verify", true),
                ("default_reset", true),
                ("follow_logs", true),
                ("reset", true),
                ("under_reset", false),
                ("dry_run", false),
                ("reconnect", false),
                ("hex_ending", false),
                ("auto_scroll", true),
                ("timestamps", true),
            ]
            .into_iter()
            .map(|(k, v)| (k.into(), v))
            .collect(),
            profiles: BTreeMap::new(),
            recent: vec![],
            history: vec![],
            command_history: vec![],
            recent_bases: BTreeMap::new(),
            theme: "system".into(),
            ipc: 0,
        }
    }
}
pub const FIELDS: &[(&str, &str, &str)] = &[
    ("firmware", "固件文件", ""),
    ("base", "BIN 基地址（留空自动）", ""),
    ("chip", "芯片型号（留空自动识别）", ""),
    ("probe", "探针", ""),
    ("speed", "调试速度 kHz", "4000"),
    ("wire", "调试协议", "swd"),
    ("erase", "擦除策略", "automatic"),
    ("address", "读取地址", "0x08000000"),
    ("length", "读取长度", "256"),
    ("profile", "配置名称", "默认配置"),
    ("log_search", "搜索日志", ""),
    ("port", "串口", ""),
    ("baud", "波特率", "115200"),
    ("bits", "数据位", "8"),
    ("stop", "停止位（15 表示 1.5）", "1"),
    ("parity", "校验位", "none"),
    ("flow", "流控", "none"),
    ("send", "发送内容", ""),
    ("format", "发送编码", "utf-8"),
    ("ending", "行尾", "none"),
    ("checksum", "发送校验", "none"),
    ("prefix", "跳过开头字节", "0"),
    ("suffix", "跳过结尾字节", "0"),
    ("interval", "间隔 ms", "1000"),
    ("filter", "搜索接收内容", ""),
    ("direction", "方向", "all"),
    ("display", "接收编码", "utf-8"),
    ("file", "文件路径", ""),
    ("chunk", "文件分块字节数", "4096"),
    ("selected", "发送包名（逗号分隔，留空全部）", ""),
    ("cycles", "宏循环次数", "1"),
    ("frame_mode", "分帧方式", "read"),
    ("frame_value", "分帧参数（毫秒／字节数／HEX）", ""),
    ("station", "Modbus 站号", "1"),
    ("function", "功能码", "3"),
    ("register", "寄存器地址", "0"),
    ("quantity", "读取数量", "1"),
    ("values", "写入值（逗号分隔）", ""),
    ("timeout", "响应超时 ms", "1000"),
    ("capture_path", "采集文件路径", ""),
    ("capture_format", "采集格式", "csv"),
    ("roll", "轮转字节数", "10485760"),
    ("export", "导出路径", ""),
    ("tx_color", "TX 颜色（名称或 #RRGGBB）", "default"),
    ("plot_sample", "曲线试算样例", ""),
    ("plot_field", "只显示字段（留空全部）", ""),
    ("command", "协议命令", ""),
    ("protocol_rule", "查看布局的协议规则名", ""),
    ("protocol_sample", "接收样例 HEX", ""),
];
pub const EDITORS: &[(&str, &str, &str)] = &[
    (
        "packets",
        "常用包库",
        r#"[{"name":"PING","group":"调试","note":"","order":0,"enabled":true,"text":"PING","format":"utf-8"}]"#,
    ),
    (
        "macro",
        "宏命令",
        r#"[{"send":{"text":"PING","format":"utf-8"},"wait":{"kind":"text","pattern":"OK"},"timeoutMs":1000}]"#,
    ),
    ("replies", "自动应答规则", "[]"),
    ("protocol", "协议模板", "[]"),
    ("colors", "记录着色规则（pattern、color）", "[]"),
    ("plot", "曲线解析配置", r#"{"mode":"numbers","fields":[]}"#),
];
impl Preferences {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).context("设置文件无效，原文件未覆盖")?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().context("设置目录无效")?;
        std::fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut file, self)?;
        file.as_file().sync_all()?;
        file.persist(path)?;
        Ok(())
    }
    pub fn get(&self, key: &str) -> &str {
        self.fields.get(key).map(String::as_str).unwrap_or("")
    }
    pub fn flag(&self, key: &str) -> bool {
        self.flags.get(key).copied().unwrap_or(matches!(
            key,
            "verify"
                | "reset"
                | "default_verify"
                | "default_reset"
                | "auto_scroll"
                | "timestamps"
                | "follow_logs"
        ))
    }
    pub fn number(&self, key: &str) -> Result<u64> {
        number(self.get(key)).with_context(|| format!("{key} 参数无效"))
    }
    pub fn target(&self) -> Result<TargetSelection> {
        let speed: u32 = self
            .number("speed")?
            .try_into()
            .context("调试速度超出范围")?;
        anyhow::ensure!(speed > 0, "调试速度必须大于 0");
        Ok(TargetSelection {
            chip: optional(self.get("chip")),
            protocol: match self.get("wire") {
                "swd" => WireProtocol::Swd,
                "jtag" => WireProtocol::Jtag,
                _ => bail!("未知调试协议"),
            },
            speed_khz: Some(speed),
            connect_under_reset: self.flag("under_reset"),
        })
    }
    pub fn unique_profile_name(&self, name: &str) -> String {
        let name = name.trim();
        let mut candidate = name.to_owned();
        let mut index = 2;
        while self.profiles.contains_key(&candidate) {
            candidate = format!("{name} ({index})");
            index += 1;
        }
        candidate
    }
    pub fn firmware(&self) -> Result<FirmwareInput> {
        Ok(FirmwareInput {
            path: self.get("firmware").into(),
            format: None,
            base_address: optional(self.get("base")).map(|s| number(&s)).transpose()?,
        })
    }
    pub fn options(&self) -> FlashOptions {
        FlashOptions {
            verify: self.flag("verify"),
            reset_after: self.flag("reset"),
            dry_run: self.flag("dry_run"),
            skip_erase: self.get("erase") == "skip",
            allow_erase_all: self.get("erase") == "full",
        }
    }
}
pub fn optional(s: &str) -> Option<String> {
    (!s.trim().is_empty()).then(|| s.trim().to_owned())
}
pub fn number(s: &str) -> Result<u64> {
    let s = s.trim();
    Ok(
        if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            u64::from_str_radix(hex, 16)?
        } else {
            s.parse()?
        },
    )
}
pub fn preferences_path() -> Result<PathBuf> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
    };
    Ok(base
        .context("无法定位用户设置目录")?
        .join("ChipDock/GPUI/preferences.json"))
}
pub fn hex_dump(address: u64, bytes: &[Option<u8>]) -> String {
    bytes
        .chunks(16)
        .enumerate()
        .map(|(row, chunk)| {
            let hex = chunk
                .iter()
                .map(|b| b.map_or("--".into(), |b| format!("{b:02X}")))
                .collect::<Vec<_>>()
                .join(" ");
            let ascii: String = chunk
                .iter()
                .map(|b| match b {
                    Some(b) if (32..127).contains(b) => *b as char,
                    _ => '·',
                })
                .collect();
            format!("{:08X}  {hex:47}  {ascii}", address + (row * 16) as u64)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn decode_hex(s: &str) -> Result<Vec<u8>> {
    anyhow::ensure!(
        s.is_ascii() && s.len().is_multiple_of(2),
        "无效的十六进制数据"
    );
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(Into::into))
        .collect()
}

impl Preferences {
    pub fn apply_connection(
        &mut self,
        path: &str,
        config: &chipdock_core::serial::types::SerialConfig,
    ) {
        self.fields.insert("port".into(), path.into());
        for (key, value) in [
            ("baud", config.baud_rate.to_string()),
            ("bits", config.data_bits.to_string()),
            ("stop", config.stop_bits.to_string()),
            (
                "parity",
                serde_json::to_value(&config.parity)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default(),
            ),
            (
                "flow",
                serde_json::to_value(&config.flow_control)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default(),
            ),
        ] {
            self.fields.insert(key.into(), value);
        }
    }
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    if let Ok(metadata) = std::fs::metadata(path) {
        anyhow::ensure!(metadata.is_file(), "导出目标不是文件");
        file.as_file().set_permissions(metadata.permissions())?;
    }
    std::io::Write::write_all(&mut file, bytes)?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_addresses_are_rejected() {
        assert!(number("-1").is_err());
        assert!(number("0x").is_err());
        assert_eq!(number("0X08000000").unwrap(), 0x08000000);
    }
    #[test]
    fn holes_remain_visible_in_hex_dump() {
        assert!(hex_dump(16, &[Some(65), None]).contains("41 --"));
    }
    #[test]
    fn corrupt_preferences_are_not_replaced() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("settings.json");
        std::fs::write(&p, b"broken").unwrap();
        assert!(Preferences::load(&p).is_err());
        assert_eq!(std::fs::read(&p).unwrap(), b"broken");
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn zero_debug_speed_is_rejected_before_probe_access() {
        let mut p = Preferences::default();
        p.fields.insert("speed".into(), "0".into());
        assert!(p.target().is_err());
    }
    #[test]
    fn saving_same_profile_name_cannot_destroy_existing_configuration() {
        let mut p = Preferences::default();
        p.profiles.insert(
            "board".into(),
            Profile {
                target: p.target().unwrap(),
                probe: None,
                options: p.options(),
                base: String::new(),
            },
        );
        assert_eq!(p.unique_profile_name("board"), "board (2)");
        assert!(p.profiles.contains_key("board"));
    }
    #[test]
    fn reconnect_status_updates_connection_fields_without_replacing_draft() {
        let mut p = Preferences::default();
        p.fields.insert("send".into(), "draft".into());
        let config = chipdock_core::serial::types::SerialConfig {
            baud_rate: 9600,
            ..Default::default()
        };
        p.apply_connection("VIRTUAL-B", &config);
        assert_eq!(p.get("port"), "VIRTUAL-B");
        assert_eq!(p.get("baud"), "9600");
        assert_eq!(p.get("send"), "draft");
    }
    #[test]
    fn recent_bin_address_and_new_defaults_survive_storage() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        let mut p = Preferences::default();
        p.recent_bases
            .insert("firmware.bin".into(), "0x08004000".into());
        p.save(&path).unwrap();
        let loaded = Preferences::load(&path).unwrap();
        assert_eq!(loaded.recent_bases["firmware.bin"], "0x08004000");
        assert!(loaded.flag("default_verify"));
    }
    #[test]
    fn failed_export_does_not_replace_a_directory() {
        let d = tempfile::tempdir().unwrap();
        let target = d.path().join("folder");
        std::fs::create_dir(&target).unwrap();
        assert!(write_atomic(&target, b"data").is_err());
        assert!(target.is_dir());
    }
}
