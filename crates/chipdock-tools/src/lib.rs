use chipdock_control::{ApiError, ErrorCode, FlowControl, Parity, SendMode, SerialConfig};
use clap::{Args, ValueEnum};
use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum ParityOption {
    #[default]
    None,
    Odd,
    Even,
    Mark,
    Space,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum FlowOption {
    #[default]
    None,
    Software,
    Hardware,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum FormatOption {
    #[default]
    Text,
    Hex,
    Command,
}

impl From<FormatOption> for SendMode {
    fn from(value: FormatOption) -> Self {
        match value {
            FormatOption::Text => Self::Text,
            FormatOption::Hex => Self::Hex,
            FormatOption::Command => Self::Command,
        }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Args)]
#[serde(default)]
pub struct ConnectionOptions {
    #[arg(long, default_value_t = 115_200)]
    pub baud_rate: u32,
    #[arg(long, default_value_t = 8)]
    pub data_bits: u8,
    /// Stop bits: 1, 1.5 or 2.
    #[arg(long, default_value = "1")]
    pub stop_bits: String,
    #[arg(long, value_enum, default_value = "none")]
    pub parity: ParityOption,
    #[arg(long, value_enum, default_value = "none")]
    pub flow_control: FlowOption,
    #[arg(long)]
    pub reconnect: bool,
}

impl Default for ConnectionOptions {
    fn default() -> Self {
        Self {
            baud_rate: 115_200,
            data_bits: 8,
            stop_bits: "1".into(),
            parity: Default::default(),
            flow_control: Default::default(),
            reconnect: false,
        }
    }
}

impl TryFrom<ConnectionOptions> for SerialConfig {
    type Error = ApiError;

    fn try_from(options: ConnectionOptions) -> Result<Self, Self::Error> {
        let stop_bits = match options.stop_bits.as_str() {
            "1" => 1,
            "1.5" => 15,
            "2" => 2,
            _ => {
                return Err(ApiError::new(
                    ErrorCode::InvalidRequest,
                    "停止位须为 1、1.5 或 2",
                ));
            }
        };
        let parity = match options.parity {
            ParityOption::None => Parity::None,
            ParityOption::Odd => Parity::Odd,
            ParityOption::Even => Parity::Even,
            ParityOption::Mark => Parity::Mark,
            ParityOption::Space => Parity::Space,
        };
        let flow_control = match options.flow_control {
            FlowOption::None => FlowControl::None,
            FlowOption::Software => FlowControl::Software,
            FlowOption::Hardware => FlowControl::Hardware,
        };
        Ok(Self {
            baud_rate: options.baud_rate,
            data_bits: options.data_bits,
            stop_bits,
            parity,
            flow_control,
        })
    }
}
