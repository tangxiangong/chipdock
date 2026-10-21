use anyhow::{Result, bail};
use encoding_rs::{CoderResult, Decoder, GBK, UTF_8};
use serde::Serialize;

use super::types::SendFormat;

#[cfg_attr(feature = "csharp", interoptopus::ffi)]
#[derive(Clone, Debug, Serialize, uniffi::Record)]
pub struct DecodedText {
    pub ascii: String,
    pub gbk: String,
    #[serde(rename = "utf-8")]
    pub utf8: String,
}

// Keep both decoders alive for a connection so switching the display preserves
// character boundaries, even when the terminal drops older records.
pub struct ReceiveDecoder {
    gbk: Decoder,
    utf8: Decoder,
}

impl Default for ReceiveDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl ReceiveDecoder {
    pub fn new() -> Self {
        Self {
            gbk: GBK.new_decoder_without_bom_handling(),
            utf8: UTF_8.new_decoder_without_bom_handling(),
        }
    }

    pub fn finish(mut self) -> Option<DecodedText> {
        let text = self.decode(&[], true);
        if text.gbk.is_empty() && text.utf8.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    pub fn decode(&mut self, bytes: &[u8], last: bool) -> DecodedText {
        DecodedText {
            ascii: bytes
                .iter()
                .map(|&byte| {
                    if byte.is_ascii() {
                        char::from(byte)
                    } else {
                        '\u{fffd}'
                    }
                })
                .collect(),
            gbk: decode_chunk(&mut self.gbk, bytes, last),
            utf8: decode_chunk(&mut self.utf8, bytes, last),
        }
    }
}

fn decode_chunk(decoder: &mut Decoder, mut bytes: &[u8], last: bool) -> String {
    let mut text = String::new();
    loop {
        text.reserve(bytes.len().saturating_mul(3).saturating_add(16));
        let (result, read, _) = decoder.decode_to_string(bytes, &mut text, last);
        bytes = &bytes[read..];
        if result == CoderResult::InputEmpty {
            return text;
        }
    }
}

pub fn encode_text(input: &str, format: SendFormat) -> Result<Vec<u8>> {
    match format {
        SendFormat::Ascii => {
            if !input.is_ascii() {
                bail!("ASCII 只能发送 U+0000–U+007F 范围内的字符，请切换 GBK 或 UTF-8");
            }
            Ok(input.as_bytes().to_vec())
        }
        SendFormat::Gbk => {
            let (bytes, _, had_errors) = GBK.encode(input);
            if had_errors {
                bail!("发送内容包含 GBK 无法编码的字符，请切换 UTF-8");
            }
            Ok(bytes.into_owned())
        }
        SendFormat::Utf8 => Ok(input.as_bytes().to_vec()),
        SendFormat::Hex => bail!("HEX 输入必须按十六进制字节解析"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_known_bytes_and_rejects_unrepresentable_characters() {
        assert_eq!(
            encode_text("中文", SendFormat::Gbk).unwrap(),
            [0xd6, 0xd0, 0xce, 0xc4]
        );
        assert_eq!(
            encode_text("中文🙂", SendFormat::Utf8).unwrap(),
            "中文🙂".as_bytes()
        );
        assert_eq!(
            encode_text("ABC\r\n", SendFormat::Ascii).unwrap(),
            b"ABC\r\n"
        );
        assert!(encode_text("中文", SendFormat::Ascii).is_err());
        assert!(encode_text("🙂", SendFormat::Gbk).is_err());
    }

    #[test]
    fn decodes_every_utf8_and_gbk_split_without_replacement() {
        for (bytes, expected, gbk) in [
            ("A中文🙂Z".as_bytes(), "A中文🙂Z", false),
            (&b"A\xd6\xd0\xce\xc4Z"[..], "A中文Z", true),
        ] {
            for split in 0..=bytes.len() {
                let mut decoder = ReceiveDecoder::new();
                let first = decoder.decode(&bytes[..split], false);
                let second = decoder.decode(&bytes[split..], true);
                let actual = if gbk {
                    first.gbk + &second.gbk
                } else {
                    first.utf8 + &second.utf8
                };
                assert_eq!(actual, expected, "split at {split}");
            }
        }
    }

    #[test]
    fn ascii_is_seven_bit_and_invalid_input_is_visible() {
        let text = ReceiveDecoder::new().decode(&[0x41, 0x80, 0xff], true);
        assert_eq!(text.ascii, "A��");
        assert_eq!(text.utf8, "A��");
    }

    #[test]
    fn finishing_reports_truncated_characters_without_inventing_ascii() {
        let mut decoder = ReceiveDecoder::new();
        let partial = decoder.decode(&[0xe4], false);
        assert_eq!(partial.utf8, "");
        assert_eq!(partial.gbk, "");
        let tail = decoder.finish().unwrap();
        assert_eq!(tail.utf8, "�");
        assert_eq!(tail.gbk, "�");
        assert_eq!(tail.ascii, "");
    }

    #[test]
    fn complete_stream_has_no_extra_record_on_finish() {
        let mut decoder = ReceiveDecoder::new();
        decoder.decode(b"ABC", false);
        assert!(decoder.finish().is_none());
        assert!(ReceiveDecoder::new().finish().is_none());
    }

    #[test]
    fn byte_at_a_time_reception_preserves_multibyte_characters() {
        let mut decoder = ReceiveDecoder::new();
        let mut text = String::new();
        for byte in "中文🙂".as_bytes() {
            text.push_str(&decoder.decode(&[*byte], false).utf8);
        }
        assert_eq!(text, "中文🙂");
    }

    #[test]
    fn new_connection_does_not_inherit_pending_bytes() {
        let mut old = ReceiveDecoder::new();
        assert_eq!(old.decode(&[0xd6], false).gbk, "");
        assert_eq!(ReceiveDecoder::new().decode(b"A", false).gbk, "A");
    }
}
