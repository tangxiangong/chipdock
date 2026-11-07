use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use super::{Direction, SerialFrameEvent, parse_hex};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    mode: Mode,
    #[serde(default)]
    fields: Vec<String>,
    #[serde(default)]
    binary: Vec<BinaryField>,
    #[serde(default)]
    template: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Numbers,
    Kv,
    Json,
    Binary,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BinaryField {
    name: String,
    offset: usize,
    #[serde(rename = "type")]
    kind: BinaryType,
    #[serde(default)]
    little_endian: bool,
    #[serde(default = "one")]
    scale: f64,
    #[serde(default)]
    bias: f64,
}

fn one() -> f64 {
    1.0
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum BinaryType {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    F32,
    F64,
}

impl BinaryType {
    fn width(self) -> usize {
        match self {
            Self::U8 | Self::I8 => 1,
            Self::U16 | Self::I16 => 2,
            Self::F64 => 8,
            _ => 4,
        }
    }
}

impl BinaryField {
    fn extract(&self, bytes: &[u8]) -> Option<f64> {
        let width = self.kind.width();
        let slice = bytes.get(self.offset..self.offset + width)?;
        let mut raw = [0; 8];
        raw[..width].copy_from_slice(slice);
        if !self.little_endian {
            raw[..width].reverse();
        }
        let bits = u64::from_le_bytes(raw);
        let value = match self.kind {
            BinaryType::U8 => f64::from(bits as u8),
            BinaryType::I8 => f64::from(bits as i8),
            BinaryType::U16 => f64::from(bits as u16),
            BinaryType::I16 => f64::from(bits as i16),
            BinaryType::U32 => f64::from(bits as u32),
            BinaryType::I32 => f64::from(bits as i32),
            BinaryType::F32 => f64::from(f32::from_bits(bits as u32)),
            BinaryType::F64 => f64::from_bits(bits),
        };
        let value = value * self.scale + self.bias;
        value.is_finite().then_some(value)
    }
}

#[derive(Serialize)]
pub struct Point {
    seq: u64,
    timestamp: String,
    values: BTreeMap<String, f64>,
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.fields.len() <= 16 && self.binary.len() <= 16 && self.template.len() <= 64,
            "曲线最多 16 个字段"
        );
        let mut names = std::collections::HashSet::new();
        for name in &self.fields {
            ensure!(
                !name.is_empty() && name.len() <= 64 && names.insert(name),
                "字段名称须为 1–64 字节且不可重复"
            );
        }
        if matches!(self.mode, Mode::Binary) {
            ensure!(!self.binary.is_empty(), "请选择至少一个二进制字段");
            names.clear();
            for field in &self.binary {
                ensure!(
                    !field.name.is_empty()
                        && field.name.len() <= 64
                        && names.insert(&field.name)
                        && field.offset < 4096
                        && field.scale.is_finite()
                        && field.bias.is_finite(),
                    "二进制字段名称、偏移或比例无效"
                );
            }
        }
        Ok(())
    }

    pub fn parse(&self, bytes: &[u8]) -> BTreeMap<String, f64> {
        if matches!(self.mode, Mode::Binary) {
            return self
                .binary
                .iter()
                .filter_map(|field| {
                    field
                        .extract(bytes)
                        .map(|value| (field.name.clone(), value))
                })
                .collect();
        }
        let Ok(text) = std::str::from_utf8(bytes) else {
            return BTreeMap::new();
        };
        let mut values = BTreeMap::new();
        match self.mode {
            Mode::Numbers => {
                for (index, part) in text
                    .split(separator)
                    .filter(|part| !part.is_empty())
                    .take(16)
                    .enumerate()
                {
                    let field = format!("v{}", index + 1);
                    if self.selected(&field)
                        && let Some(value) = number(part)
                    {
                        values.insert(field, value);
                    }
                }
            }
            Mode::Kv => {
                for part in text.split(separator) {
                    if let Some((key, value)) =
                        part.split_once('=').or_else(|| part.split_once(':'))
                        && self.selected(key)
                        && key.len() <= 64
                        && let Some(value) = number(value)
                    {
                        values.insert(key.to_owned(), value);
                    }
                    if values.len() == 16 {
                        break;
                    }
                }
            }
            Mode::Json => {
                let Ok(data) = serde_json::from_str::<Value>(text) else {
                    return values;
                };
                if self.fields.is_empty() {
                    if let Some(object) = data.as_object() {
                        for (name, value) in object
                            .iter()
                            .filter(|(name, _)| name.len() <= 64)
                            .filter_map(|(name, value)| value.as_f64().map(|value| (name, value)))
                            .take(16)
                        {
                            values.insert(name.clone(), value);
                        }
                    }
                } else {
                    for name in &self.fields {
                        let value = if name.starts_with('/') {
                            data.pointer(name)
                        } else {
                            name.split('.')
                                .try_fold(&data, |value, field| value.get(field))
                        };
                        if let Some(value) = value.and_then(Value::as_f64) {
                            values.insert(name.clone(), value);
                        }
                    }
                }
            }
            Mode::Binary => unreachable!("binary handled above"),
        }
        values
    }

    fn selected(&self, field: &str) -> bool {
        self.fields.is_empty() || self.fields.iter().any(|name| name == field)
    }

    pub fn frame(&self, frame: &SerialFrameEvent) -> Vec<Point> {
        if !matches!(frame.direction, Direction::Receive) {
            return vec![];
        }
        let point = |bytes: &[u8]| {
            let values = self.parse(bytes);
            (!values.is_empty()).then(|| Point {
                seq: frame.sequence,
                timestamp: frame.timestamp.clone(),
                values,
            })
        };
        if self.template.is_empty() {
            return point(&frame.bytes).into_iter().collect();
        }
        frame
            .parsed
            .iter()
            .filter(|record| record.valid && record.name == self.template)
            .take(16)
            .filter_map(|record| {
                parse_hex(&record.payload_hex)
                    .ok()
                    .and_then(|bytes| point(&bytes))
            })
            .collect()
    }
}

fn separator(character: char) -> bool {
    character.is_whitespace() || matches!(character, ',' | ';')
}
fn number(value: &str) -> Option<f64> {
    value.parse::<f64>().ok().filter(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plot_parses_selected_fields_and_binary_scale() {
        for (settings, bytes, field, expected) in [
            (
                r#"{"mode":"numbers","fields":["v2"]}"#,
                &b"4,-12.5,9"[..],
                "v2",
                -12.5,
            ),
            (r#"{"mode":"kv"}"#, &b"x=20 y:-1"[..], "y", -1.0),
            (
                r#"{"mode":"json","fields":["sensor.temp"]}"#,
                &br#"{"sensor":{"temp":3}}"#[..],
                "sensor.temp",
                3.0,
            ),
            (
                r#"{"mode":"binary","binary":[{"name":"sensor","offset":1,"type":"i16","littleEndian":true,"scale":0.5,"bias":10}]}"#,
                &b"\x00\xfc\xff"[..],
                "sensor",
                8.0,
            ),
        ] {
            let settings: Settings = serde_json::from_str(settings).unwrap();
            settings.validate().unwrap();
            assert_eq!(settings.parse(bytes)[field], expected);
        }
    }
}
