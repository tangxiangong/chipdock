//! Shared display/export rules so terminal and record views cannot disagree.
use crate::{
    model::Preferences,
    serial::{frame_text, search},
};
use anyhow::{Result, bail, ensure};
use chipdock_core::serial::{Direction, SerialFrameEvent};
use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use serde_json::Value;

pub fn matches(frame: &SerialFrameEvent, p: &Preferences, query: &Regex) -> bool {
    let direction = match frame.direction {
        Direction::Receive => "receive",
        Direction::Send => "send",
    };
    (p.get("direction") == "all" || p.get("direction") == direction)
        && query.is_match(&display_text(frame, p))
}
pub fn filtered<'a>(
    frames: impl Iterator<Item = &'a SerialFrameEvent>,
    p: &Preferences,
) -> Result<Vec<&'a SerialFrameEvent>> {
    let query = search(p)?;
    Ok(frames.filter(|f| matches(f, p, &query)).collect())
}
pub fn display_text(frame: &SerialFrameEvent, p: &Preferences) -> String {
    if p.flag("mixed") {
        format!(
            "{}\n{}",
            frame_text(
                frame,
                if p.get("display") == "hex" {
                    "utf-8"
                } else {
                    p.get("display")
                }
            ),
            frame.hex
        )
    } else {
        frame_text(frame, p.get("display")).into()
    }
}

#[derive(Deserialize)]
struct ColorRule {
    pattern: String,
    #[serde(default)]
    regex: bool,
    color: String,
}
pub struct Colors {
    rules: Vec<(Regex, u32)>,
    tx: Option<u32>,
}
pub fn color(value: &str) -> Result<Option<u32>> {
    Ok(Some(match value {
        "" | "default" => return Ok(None),
        "red" => 0xcd5c5c,
        "orange" => 0xff8c00,
        "green" => 0x2e8b57,
        "blue" => 0x1e90ff,
        "purple" => 0x9370db,
        s => {
            let hex = s.strip_prefix('#').unwrap_or(s);
            ensure!(hex.len() == 6, "颜色必须是已知名称或六位 RGB");
            u32::from_str_radix(hex, 16)?
        }
    }))
}
impl Colors {
    pub fn new(source: &str, tx: &str) -> Result<Self> {
        let rules: Vec<ColorRule> = serde_json::from_str(source)?;
        ensure!(rules.len() <= 128, "着色规则过多");
        let mut compiled = Vec::new();
        for rule in rules {
            ensure!(
                !rule.pattern.is_empty() && rule.pattern.encode_utf16().count() <= 256,
                "规则需要 1–256 字符"
            );
            let pattern = if rule.regex {
                rule.pattern
            } else {
                regex::escape(&rule.pattern)
            };
            let expression = RegexBuilder::new(&pattern)
                .case_insensitive(!rule.regex)
                .size_limit(1024 * 1024)
                .build()?;
            let color =
                color(&rule.color)?.ok_or_else(|| anyhow::anyhow!("接收规则需要明确颜色"))?;
            compiled.push((expression, color));
        }
        Ok(Self {
            rules: compiled,
            tx: color(tx)?,
        })
    }
    pub fn for_frame(&self, frame: &SerialFrameEvent, text: &str) -> Option<u32> {
        if matches!(frame.direction, Direction::Send) {
            self.tx
        } else {
            self.rules
                .iter()
                .find(|(r, _)| r.is_match(text))
                .map(|(_, c)| *c)
        }
    }
}
fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}
pub fn records(frames: &[&SerialFrameEvent], format: &str, encoding: &str) -> Result<Vec<u8>> {
    match format {
        "json" => Ok(serde_json::to_vec_pretty(frames)?),
        "bin" => Ok(frames
            .iter()
            .flat_map(|f| f.bytes.iter().copied())
            .collect()),
        "txt" => Ok(frames
            .iter()
            .map(|f| {
                format!(
                    "{} {:?} {}\r\n",
                    f.timestamp,
                    f.direction,
                    frame_text(f, encoding)
                )
            })
            .collect::<String>()
            .into_bytes()),
        "csv" => {
            let mut csv = "sequence,timestamp,direction,hex,text\r\n".to_owned();
            for f in frames {
                csv.push_str(&format!(
                    "{},{},{},{},{}\r\n",
                    f.sequence,
                    quote(&f.timestamp),
                    quote(match f.direction {
                        Direction::Send => "send",
                        Direction::Receive => "receive",
                    }),
                    quote(&f.hex),
                    quote(frame_text(f, encoding))
                ));
            }
            Ok(csv.into_bytes())
        }
        _ => bail!("未知记录格式"),
    }
}
pub fn plot_csv(points: &[Value], field: &str) -> Vec<u8> {
    let mut csv = "sample,sequence,timestamp,field,value\r\n".to_owned();
    for (index, p) in points.iter().enumerate() {
        if let Some(values) = p["values"].as_object() {
            for (name, value) in values {
                if (field.is_empty() || field == name)
                    && let Some(value) = value.as_f64()
                {
                    csv.push_str(&format!(
                        "{},{},{},{},{}\r\n",
                        index + 1,
                        p["seq"],
                        quote(p["timestamp"].as_str().unwrap_or("")),
                        quote(name),
                        value
                    ));
                }
            }
        }
    }
    csv.into_bytes()
}

pub fn append_plot(points: &mut Vec<Value>, cursor: &mut u64, dropped: &mut u64, batch: &Value) {
    *cursor = batch["nextCursor"].as_u64().unwrap_or(*cursor);
    *dropped = dropped.saturating_add(batch["droppedFrames"].as_u64().unwrap_or(0));
    if let Some(values) = batch["points"].as_array() {
        points.extend(values.clone());
        if points.len() > 4096 {
            points.drain(..points.len() - 4096);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame(direction: Direction) -> SerialFrameEvent {
        serde_json::from_value(serde_json::json!({"direction":direction,"bytes":[65,34,66],"hex":"41 22 42","text":{"ascii":"A\"B","gbk":"A\"B","utf-8":"A\"B","latin-1":"A\"B"},"timestamp":"2026-11-07","source":"serial","sequence":7,"parsed":[]})).unwrap()
    }
    #[test]
    fn record_filter_applies_direction() {
        let rx = frame(Direction::Receive);
        let tx = frame(Direction::Send);
        let mut p = Preferences::default();
        p.fields.insert("direction".into(), "receive".into());
        assert_eq!(filtered([&rx, &tx].into_iter(), &p).unwrap().len(), 1);
    }
    #[test]
    fn literal_colors_are_not_regex_and_tx_is_separate() {
        let rules = Colors::new(
            r##"[{"pattern":"A.B","regex":false,"color":"red"}]"##,
            "blue",
        )
        .unwrap();
        assert_eq!(rules.for_frame(&frame(Direction::Receive), "AXB"), None);
        assert_eq!(
            rules.for_frame(&frame(Direction::Receive), "A.B"),
            Some(0xcd5c5c)
        );
        assert_eq!(
            rules.for_frame(&frame(Direction::Send), "A.B"),
            Some(0x1e90ff)
        );
    }
    #[test]
    fn exports_escape_csv_and_preserve_binary() {
        let f = frame(Direction::Receive);
        assert!(
            String::from_utf8(records(&[&f], "csv", "utf-8").unwrap())
                .unwrap()
                .contains("\"A\"\"B\"")
        );
        assert_eq!(records(&[&f], "bin", "utf-8").unwrap(), [65, 34, 66]);
    }
    #[test]
    fn plot_csv_keeps_sequence_and_selected_field() {
        let points = vec![
            serde_json::json!({"seq":99,"timestamp":"t","values":{"temperature":1.5,"voltage":3.3}}),
        ];
        let csv = String::from_utf8(plot_csv(&points, "voltage")).unwrap();
        assert!(csv.contains("1,99,\"t\",\"voltage\",3.3"));
        assert!(!csv.contains("temperature"));
    }
}
