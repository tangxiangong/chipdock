//! Declarative serial commands. All operations are pure and never access a device.
use super::{bytes_to_hex, checksum::append_checksum, parse_hex, types::ChecksumAlgorithm};
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

const LIMIT: usize = 4096;
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub name: String,
    pub template: String,
    #[serde(default)]
    pub display: String,
    #[serde(default)]
    pub receive: bool,
    #[serde(default)]
    pub checksum_start: usize,
    #[serde(default)]
    pub checksum_end: Option<usize>,
    #[serde(default)]
    pub response: String,
    #[serde(default = "timeout")]
    pub timeout_ms: u64,
    /// Nonempty delimiter selects a UTF-8 text frame, exposed as {text}.
    #[serde(default)]
    pub delimiter: String,
}
fn timeout() -> u64 {
    1000
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    rules: Vec<Rule>,
    #[serde(default)]
    input: String,
    #[serde(default)]
    buffer: String,
    #[serde(default)]
    incoming: String,
    #[serde(default)]
    operation: String,
}
#[derive(Serialize, Default)]
struct Response {
    hex: String,
    buffer: String,
    records: Vec<Record>,
    response: String,
    timeout_ms: u64,
}
#[derive(Serialize)]
struct Record {
    name: String,
    hex: String,
    message: String,
    valid: bool,
}
#[derive(Clone)]
enum Part {
    Byte(u8),
    Field {
        name: String,
        signed: bool,
        width: usize,
        little: bool,
        min: i64,
        max: i64,
    },
}
struct Compiled {
    rule: Rule,
    parts: Vec<Part>,
    checksum: ChecksumAlgorithm,
    payload: usize,
    length: usize,
    delimiter: Vec<u8>,
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn compile(rule: &Rule) -> Result<Compiled> {
    ensure!(
        identifier(&rule.name),
        "名称仅允许字母、数字、横线和下划线（1–64 字符）"
    );
    ensure!(
        rule.template.len() <= 16384 && rule.display.len() <= 1024,
        "模板过长"
    );
    ensure!(
        (10..=3600000).contains(&rule.timeout_ms),
        "反馈超时须为 10–3600000 毫秒"
    );
    let delimiter = parse_hex(&rule.delimiter)?;
    if !delimiter.is_empty() {
        ensure!(
            rule.receive && delimiter.len() <= 16 && rule.template.is_empty(),
            "文本接收规则须使用空模板及 1–16 字节分隔符"
        );
        ensure!(
            rule.checksum_start == 0 && rule.checksum_end.is_none(),
            "文本规则不支持校验范围"
        );
        validate_display(&rule.display, &["text"])?;
        return Ok(Compiled {
            rule: rule.clone(),
            parts: vec![],
            checksum: ChecksumAlgorithm::None,
            payload: 0,
            length: 0,
            delimiter,
        });
    }
    let mut parts = Vec::new();
    let mut names = Vec::new();
    let mut checksum = ChecksumAlgorithm::None;
    let mut checked = false;
    for token in rule.template.split_whitespace() {
        ensure!(!checked, "校验字段必须位于帧尾");
        if let Some(inner) = token.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
            let fields: Vec<_> = inner.split(':').collect();
            ensure!(
                fields.len() >= 2,
                "字段格式应为 {{speed:u16be}} 或 {{speed:u16be:0:3000}}"
            );
            if fields[0] == "checksum" {
                ensure!(fields.len() == 2, "校验字段格式错误");
                checksum = match fields[1] {
                    "sum8" => ChecksumAlgorithm::Sum8,
                    "xor8" => ChecksumAlgorithm::Xor8,
                    "crc16modbus" => ChecksumAlgorithm::Crc16Modbus,
                    _ => bail!("未知校验算法"),
                };
                checked = true;
                continue;
            }
            ensure!(
                identifier(fields[0]) && !names.contains(&fields[0]),
                "字段名称无效或重复"
            );
            let (signed, width, little) = match fields[1] {
                "u8" => (false, 1, false),
                "i8" => (true, 1, false),
                "u16be" => (false, 2, false),
                "u16le" => (false, 2, true),
                "i16be" => (true, 2, false),
                "i16le" => (true, 2, true),
                "u32be" => (false, 4, false),
                "u32le" => (false, 4, true),
                "i32be" => (true, 4, false),
                "i32le" => (true, 4, true),
                _ => bail!("未知字段类型：{}", fields[1]),
            };
            let bits = width * 8;
            let (mut min, mut max) = if signed {
                (-(1i64 << (bits - 1)), (1i64 << (bits - 1)) - 1)
            } else {
                (0, (1i64 << bits) - 1)
            };
            if fields.len() == 4 {
                let low = number(fields[2])?;
                let high = number(fields[3])?;
                ensure!(
                    low >= min && high <= max && low <= high,
                    "字段范围超出类型或上下限颠倒"
                );
                min = low;
                max = high;
            } else {
                ensure!(fields.len() == 2, "字段范围须同时填写最小值和最大值");
            }
            names.push(fields[0]);
            parts.push(Part::Field {
                name: fields[0].into(),
                signed,
                width,
                little,
                min,
                max,
            });
        } else {
            ensure!(
                token.len() == 2 && token.bytes().all(|b| b.is_ascii_hexdigit()),
                "固定字节须为两位 HEX：{token}"
            );
            parts.push(Part::Byte(u8::from_str_radix(token, 16)?));
        }
    }
    let payload = parts
        .iter()
        .map(|p| match p {
            Part::Byte(_) => 1,
            Part::Field { width, .. } => *width,
        })
        .sum::<usize>();
    let length = payload
        + match checksum {
            ChecksumAlgorithm::None => 0,
            ChecksumAlgorithm::Crc16Modbus => 2,
            _ => 1,
        };
    ensure!(payload > 0 && length <= LIMIT, "帧长度须为 1–4096 字节");
    ensure!(
        !rule.receive || matches!(parts.first(), Some(Part::Byte(_))),
        "二进制接收模板必须以固定帧头开始"
    );
    ensure!(
        rule.checksum_start < rule.checksum_end.unwrap_or(payload)
            && rule.checksum_end.unwrap_or(payload) <= payload,
        "校验范围无效（起始包含，结束不包含）"
    );
    validate_display(&rule.display, &names)?;
    Ok(Compiled {
        rule: rule.clone(),
        parts,
        checksum,
        payload,
        length,
        delimiter,
    })
}
fn validate_display(display: &str, names: &[&str]) -> Result<()> {
    let mut rest = display;
    while let Some((before, tail)) = rest.split_once('{') {
        ensure!(!before.contains('}'), "显示模板括号不匹配");
        let (name, after) = tail
            .split_once('}')
            .ok_or_else(|| anyhow::anyhow!("显示模板括号不匹配"))?;
        ensure!(names.contains(&name), "显示模板引用未知字段：{name}");
        rest = after;
    }
    ensure!(!rest.contains('}'), "显示模板括号不匹配");
    Ok(())
}
fn number(s: &str) -> Result<i64> {
    let (negative, raw) = s.strip_prefix('-').map_or((false, s), |v| (true, v));
    ensure!(!raw.starts_with(['+', '-']), "整数只能包含一个前导符号");
    let n = if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        ensure!(
            !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit()),
            "无效十六进制整数"
        );
        i64::from_str_radix(hex, 16)?
    } else {
        raw.parse::<i64>()?
    };
    if negative {
        n.checked_neg()
            .ok_or_else(|| anyhow::anyhow!("整数超出范围"))
    } else {
        Ok(n)
    }
}
impl Compiled {
    fn check(&self, payload: &[u8]) -> Vec<u8> {
        let mut checked = payload
            [self.rule.checksum_start..self.rule.checksum_end.unwrap_or(self.payload)]
            .to_vec();
        let before = checked.len();
        append_checksum(&mut checked, self.checksum);
        checked[before..].to_vec()
    }
    fn encode(&self, args: &[&str]) -> Result<Vec<u8>> {
        let count = self
            .parts
            .iter()
            .filter(|p| matches!(p, Part::Field { .. }))
            .count();
        ensure!(
            args.len() == count,
            "{} 需要 {} 个参数，实际 {} 个",
            self.rule.name,
            count,
            args.len()
        );
        let mut args = args.iter();
        let mut bytes = Vec::new();
        for part in &self.parts {
            match part {
                Part::Byte(b) => bytes.push(*b),
                Part::Field {
                    name,
                    width,
                    little,
                    min,
                    max,
                    ..
                } => {
                    let value = number(args.next().copied().unwrap_or_default())
                        .map_err(|_| anyhow::anyhow!("{name} 必须是整数"))?;
                    ensure!(
                        (*min..=*max).contains(&value),
                        "{name} 超出允许范围 {min}～{max}"
                    );
                    let raw = if *little {
                        value.to_le_bytes()
                    } else {
                        value.to_be_bytes()
                    };
                    bytes.extend_from_slice(if *little {
                        &raw[..*width]
                    } else {
                        &raw[8 - width..]
                    });
                }
            }
        }
        bytes.extend(self.check(&bytes));
        Ok(bytes)
    }
    fn prefix(&self, bytes: &[u8]) -> bool {
        if !self.delimiter.is_empty() {
            return true;
        }
        let mut offset = 0;
        for part in &self.parts {
            match part {
                Part::Byte(b) => {
                    if bytes.get(offset).is_some_and(|v| v != b) {
                        return false;
                    }
                    offset += 1;
                }
                Part::Field { width, .. } => offset += width,
            }
            if offset >= bytes.len() {
                break;
            }
        }
        true
    }
    fn decode(&self, bytes: &[u8]) -> Result<String> {
        if !self.delimiter.is_empty() {
            let text = std::str::from_utf8(&bytes[..bytes.len() - self.delimiter.len()])?;
            return Ok(if self.rule.display.is_empty() {
                text.into()
            } else {
                self.rule.display.replace("{text}", text)
            });
        }
        ensure!(
            self.check(&bytes[..self.payload]) == bytes[self.payload..],
            "校验失败"
        );
        let mut offset = 0;
        let mut display = self.rule.display.clone();
        let mut fields = Vec::new();
        for part in &self.parts {
            match part {
                Part::Byte(_) => offset += 1,
                Part::Field {
                    name,
                    signed,
                    width,
                    little,
                    min,
                    max,
                } => {
                    let raw = &bytes[offset..offset + width];
                    let mut value = 0i64;
                    if *little {
                        for b in raw.iter().rev() {
                            value = (value << 8) | i64::from(*b);
                        }
                    } else {
                        for b in raw {
                            value = (value << 8) | i64::from(*b);
                        }
                    }
                    if *signed && value & (1 << (width * 8 - 1)) != 0 {
                        value -= 1 << (width * 8);
                    }
                    ensure!(
                        (*min..=*max).contains(&value),
                        "{name} 超出允许范围 {min}～{max}"
                    );
                    display = display.replace(&format!("{{{name}}}"), &value.to_string());
                    fields.push(format!("{name}={value}"));
                    offset += width;
                }
            }
        }
        Ok(if display.is_empty() {
            fields.join("  ")
        } else {
            display
        })
    }
}
/// JSON boundary shared by native clients; errors never fall back to raw transmission.
pub fn execute(json: &str) -> Result<String> {
    ensure!(json.len() <= 1024 * 1024, "协议请求过大");
    let request: Request = serde_json::from_str(json)?;
    ensure!(request.rules.len() <= 128, "最多支持 128 条规则");
    let mut rules = Vec::new();
    for rule in &request.rules {
        ensure!(
            !rules.iter().any(|r: &Compiled| r.rule.name == rule.name),
            "规则名称重复：{}",
            rule.name
        );
        rules.push(compile(rule)?);
    }
    for rule in &rules {
        if !rule.rule.response.is_empty() {
            ensure!(
                !rule.rule.receive
                    && rules
                        .iter()
                        .any(|r| r.rule.receive && r.rule.name == rule.rule.response),
                "关联接收规则不存在：{}",
                rule.rule.response
            );
        }
    }
    let mut result = Response::default();
    match request.operation.as_str() {
        "validate" => {}
        "encode" => {
            let mut words = request.input.split_whitespace();
            let name = words.next().unwrap_or_default();
            let rule = rules
                .iter()
                .find(|r| !r.rule.receive && r.rule.name == name)
                .ok_or_else(|| anyhow::anyhow!("未知发送命令：{name}"))?;
            result.hex = bytes_to_hex(&rule.encode(&words.collect::<Vec<_>>())?);
            result.response = rule.rule.response.clone();
            result.timeout_ms = rule.rule.timeout_ms;
        }
        "decode" => {
            let mut bytes = parse_hex(&request.buffer)?;
            bytes.extend(parse_hex(&request.incoming)?);
            ensure!(bytes.len() <= 65536, "接收输入超过 64 KiB");
            let rules: Vec<_> = rules.iter().filter(|r| r.rule.receive).collect();
            let mut offset = 0;
            while offset < bytes.len() {
                let tail = &bytes[offset..];
                let mut candidates = Vec::new();
                let mut partial = false;
                for rule in &rules {
                    if !rule.prefix(tail) {
                        continue;
                    }
                    let length = if rule.delimiter.is_empty() {
                        Some(rule.length)
                    } else {
                        tail.windows(rule.delimiter.len())
                            .position(|w| w == rule.delimiter)
                            .map(|n| n + rule.delimiter.len())
                    };
                    match length {
                        Some(n) if n <= tail.len() && n <= LIMIT => candidates.push((*rule, n)),
                        _ => partial = true,
                    }
                }
                // A complete valid frame must not be held by another rule's unfinished frame.
                // Only candidates passing checksum and field validation can conflict.
                let valid_candidates: Vec<_> = candidates
                    .iter()
                    .copied()
                    .filter(|(rule, length)| rule.decode(&tail[..*length]).is_ok())
                    .collect();
                if !valid_candidates.is_empty() {
                    candidates = valid_candidates;
                } else if partial && tail.len() < LIMIT {
                    break;
                }
                if candidates.len() > 1 {
                    result.records.push(Record {
                        name: String::new(),
                        hex: bytes_to_hex(&tail[..1]),
                        message: format!(
                            "规则冲突：{}",
                            candidates
                                .iter()
                                .map(|(r, _)| r.rule.name.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                        valid: false,
                    });
                    offset += 1;
                } else if let Some((rule, n)) = candidates.first() {
                    let decoded = rule.decode(&tail[..*n]);
                    let valid = decoded.is_ok();
                    result.records.push(Record {
                        name: rule.rule.name.clone(),
                        hex: bytes_to_hex(&tail[..*n]),
                        message: decoded.unwrap_or_else(|e| e.to_string()),
                        valid,
                    });
                    offset += if valid || !rule.delimiter.is_empty() {
                        *n
                    } else {
                        1
                    };
                } else {
                    result.records.push(Record {
                        name: String::new(),
                        hex: bytes_to_hex(&tail[..1]),
                        message: "未匹配数据".into(),
                        valid: false,
                    });
                    offset += 1;
                }
            }
            result.buffer = bytes_to_hex(&bytes[offset..]);
        }
        _ => bail!("未知协议操作"),
    }
    Ok(serde_json::to_string(&result)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    fn rules() -> Value {
        json!([
            {"name":"motor-speed", "template":"AA BB {speed:u16be:0:3000} {checksum:sum8}", "response":"speed"},
            {"name":"speed", "template":"CC DD {speed:u16be} {checksum:sum8}", "receive":true, "display":"速度：{speed} rpm"}
        ])
    }
    fn call(operation: &str, input: &str, buffer: &str, incoming: &str) -> Result<Value> {
        Ok(serde_json::from_str(&execute(&json!({"rules":rules(),"operation":operation,"input":input,"buffer":buffer,"incoming":incoming}).to_string())?)?)
    }
    #[test]
    fn malformed_signs_return_errors_without_panicking() {
        for input in ["--9223372036854775808", "--1", "-+1", "0x-1"] {
            assert!(number(input).is_err(), "{input}");
        }
    }

    #[test]
    fn complete_binary_frames_do_not_wait_for_text_delimiters() {
        let request = json!({"rules":[
            {"name":"binary","template":"CC DD {v:u16be} {checksum:sum8}","receive":true},
            {"name":"line","template":"","delimiter":"0D 0A","receive":true}
        ],"operation":"decode","incoming":"CC DD 00 2D D6"});
        let result: Value = serde_json::from_str(&execute(&request.to_string()).unwrap()).unwrap();
        assert_eq!(result["buffer"], "");
        assert_eq!(result["records"][0]["name"], "binary");
        assert_eq!(result["records"][0]["valid"], true);
    }

    #[test]
    fn checksum_failure_is_not_a_conflicting_match() {
        let request = json!({"rules":[
            {"name":"sum","template":"AA {v:u8} {checksum:sum8}","receive":true},
            {"name":"xor","template":"AA {v:u8} {checksum:xor8}","receive":true}
        ],"operation":"decode","incoming":"AA 02 AC"});
        let result: Value = serde_json::from_str(&execute(&request.to_string()).unwrap()).unwrap();
        assert_eq!(result["records"][0]["name"], "sum");
        assert_eq!(result["records"][0]["valid"], true);
    }

    #[test]
    fn command_encodes_and_rejects_invalid_arguments() {
        assert_eq!(
            call("encode", "motor-speed 45", "", "").unwrap()["hex"],
            "AA BB 00 2D 92"
        );
        for input in [
            "motor-speed -1",
            "motor-speed 3001",
            "motor-speed",
            "motor-speed 1 2",
            "unknown 1",
        ] {
            assert!(call("encode", input, "", "").is_err());
        }
    }
    #[test]
    fn split_and_coalesced_frames_preserve_tail() {
        let first = call("decode", "", "", "CC DD 00").unwrap();
        assert_eq!(first["buffer"], "CC DD 00");
        let next = call(
            "decode",
            "",
            first["buffer"].as_str().unwrap(),
            "2D D6 CC DD 00 2D D6 CC",
        )
        .unwrap();
        assert_eq!(next["records"].as_array().unwrap().len(), 2);
        assert_eq!(next["buffer"], "CC");
        assert_eq!(next["records"][0]["message"], "速度：45 rpm");
    }
    #[test]
    fn corrupt_frame_recovers_and_reports_error() {
        let result = call("decode", "", "", "CC DD 00 2D 00 CC DD 00 2D D6").unwrap();
        assert_eq!(result["records"][0]["valid"], false);
        assert!(
            result["records"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["valid"] == true)
        );
    }
    #[test]
    fn signed_endian_and_checksum_range() {
        let rule: Rule = serde_json::from_value(
            json!({"name":"test","template":"AA {v:i16le} {checksum:xor8}","checksum_start":1}),
        )
        .unwrap();
        let c = compile(&rule).unwrap();
        let bytes = c.encode(&["-2"]).unwrap();
        assert_eq!(bytes, [0xAA, 0xFE, 0xFF, 1]);
        assert_eq!(c.decode(&bytes).unwrap(), "v=-2");
    }
    #[test]
    fn crc_and_integer_wire_formats_match_protocol_vectors() {
        let crc: Rule = serde_json::from_value(
            json!({"name":"crc","template":"31 32 33 34 35 36 37 38 39 {checksum:crc16modbus}"}),
        )
        .unwrap();
        assert_eq!(
            compile(&crc).unwrap().encode(&[]).unwrap()[9..],
            [0x37, 0x4b]
        );
        for (kind, value, expected) in [
            ("u32le", "0x12345678", vec![0x78, 0x56, 0x34, 0x12]),
            ("u32be", "4294967295", vec![255, 255, 255, 255]),
            ("i32be", "-2147483648", vec![128, 0, 0, 0]),
            ("i8", "-128", vec![128]),
        ] {
            let rule: Rule =
                serde_json::from_value(json!({"name":"value","template":format!("{{v:{kind}}}")}))
                    .unwrap();
            assert_eq!(compile(&rule).unwrap().encode(&[value]).unwrap(), expected);
        }
        for template in [
            "AA {v:u16}",
            "AA {v:u8} {v:u8}",
            "AA {checksum:sum8} FF",
            "AA {v:u8:0:256}",
        ] {
            let rule: Rule =
                serde_json::from_value(json!({"name":"bad","template":template})).unwrap();
            assert!(compile(&rule).is_err());
        }
    }

    #[test]
    fn conflict_text_and_bounds() {
        let text = json!({"rules":[{"name":"line","template":"","receive":true,"delimiter":"0D 0A","display":"文本：{text}"}],"operation":"decode","incoming":"48 69 0D 0A"});
        let result: Value = serde_json::from_str(&execute(&text.to_string()).unwrap()).unwrap();
        assert_eq!(result["records"][0]["message"], "文本：Hi");
        let conflict = json!({"rules":[{"name":"a","template":"AA {v:u8}","receive":true},{"name":"b","template":"AA {v:u8}","receive":true}],"operation":"decode","incoming":"AA 01"});
        assert!(execute(&conflict.to_string()).unwrap().contains("规则冲突"));
        let long = "AA ".repeat(LIMIT + 1);
        assert!(
            compile(&Rule {
                name: "a".into(),
                template: long,
                timeout_ms: 1000,
                ..Default::default()
            })
            .is_err()
        );
    }
}
