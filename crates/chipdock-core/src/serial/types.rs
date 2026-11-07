use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "csharp", interoptopus::ffi)]
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Parity {
    #[default]
    None,
    Odd,
    Even,
    Mark,
    Space,
}

#[cfg_attr(feature = "csharp", interoptopus::ffi)]
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FlowControl {
    #[default]
    None,
    Software,
    Hardware,
}

#[cfg_attr(feature = "csharp", interoptopus::ffi)]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SerialConfig {
    pub baud_rate: u32,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: Parity,
    pub flow_control: FlowControl,
}

impl SerialConfig {
    pub fn validate(&self) -> Result<()> {
        if self.baud_rate == 0 {
            bail!("波特率必须大于 0");
        }
        if !(5..=8).contains(&self.data_bits) {
            bail!("数据位必须是 5、6、7 或 8");
        }
        if !matches!(self.stop_bits, 1 | 2 | 15) {
            bail!("停止位必须是 1、1.5 或 2");
        }
        if !cfg!(windows)
            && (self.stop_bits == 15 || matches!(self.parity, Parity::Mark | Parity::Space))
        {
            bail!("当前平台不支持 Mark / Space 校验和 1.5 停止位");
        }
        if cfg!(windows)
            && ((self.data_bits == 5 && self.stop_bits == 2)
                || (self.data_bits != 5 && self.stop_bits == 15))
        {
            bail!("1.5 停止位仅支持 5 数据位，2 停止位需要至少 6 数据位");
        }
        Ok(())
    }
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            baud_rate: 115_200,
            data_bits: 8,
            stop_bits: 1,
            parity: Parity::None,
            flow_control: FlowControl::None,
        }
    }
}

#[cfg_attr(feature = "csharp", interoptopus::ffi)]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortInfo {
    pub path: String,
    pub display_name: String,
    pub manufacturer: Option<String>,
    pub serial_number: Option<String>,
    pub vid: Option<String>,
    pub pid: Option<String>,
}

#[cfg_attr(feature = "csharp", interoptopus::ffi)]
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ControlLine {
    Rts,
    Dtr,
}

#[cfg_attr(feature = "csharp", interoptopus::ffi)]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialStatus {
    pub connected: bool,
    pub selected_protocol: Option<String>,
    pub job_kind: Option<String>,
    pub job_done: u64,
    pub job_total: u64,
    pub job_error: Option<String>,
    pub uptime_ms: u64,
    pub rx_per_second: u64,
    pub tx_per_second: u64,
    pub logging: bool,
    pub capturing: bool,
    pub capture_error: Option<String>,
    pub feedback: Option<String>,
    pub reconnecting: bool,
    pub generation: u64,
    pub rts: Option<bool>,
    pub dtr: Option<bool>,
    pub path: Option<String>,
    pub config: Option<SerialConfig>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub opened_at: Option<String>,
    pub last_error: Option<String>,
}

#[cfg_attr(feature = "csharp", interoptopus::ffi)]
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SendFormat {
    Hex,
    Ascii,
    Gbk,
    #[serde(rename = "utf-8")]
    Utf8,
    #[serde(rename = "latin-1")]
    Latin1,
}

#[cfg_attr(feature = "csharp", interoptopus::ffi)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChecksumAlgorithm {
    #[default]
    None,
    Sum8,
    OddParity,
    EvenParity,
    Xor8,
    Crc16Modbus,
    Crc16Ccitt,
    Crc32,
    Lrc,
}

#[cfg_attr(feature = "csharp", interoptopus::ffi)]
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineEnding {
    None,
    Lf,
    Cr,
    Crlf,
}

impl LineEnding {
    pub fn bytes(self) -> &'static [u8] {
        match self {
            Self::None => b"",
            Self::Lf => b"\n",
            Self::Cr => b"\r",
            Self::Crlf => b"\r\n",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_115200_8n1() {
        let config = SerialConfig::default();
        assert_eq!(config.baud_rate, 115_200);
        assert_eq!(config.data_bits, 8);
        assert_eq!(config.stop_bits, 1);
        assert_eq!(config.parity, Parity::None);
    }

    #[test]
    fn invalid_serial_config_is_rejected() {
        let config = SerialConfig {
            data_bits: 9,
            ..SerialConfig::default()
        };
        assert!(config.validate().is_err());
    }
}

#[cfg_attr(feature = "csharp", interoptopus::ffi)]
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SendOptions {
    pub hex_line_ending: bool,
    pub skip_prefix: u32,
    pub skip_suffix: u32,
}
