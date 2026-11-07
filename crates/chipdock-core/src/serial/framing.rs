use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::Instant;

use super::parse_hex;

const MAX_FRAME: usize = 4096;

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum Framing {
    #[default]
    Read,
    Idle {
        ms: u64,
    },
    Header {
        hex: String,
    },
    Tail {
        hex: String,
    },
    Fixed {
        bytes: usize,
    },
}

#[derive(Default)]
pub(super) struct Framer {
    mode: Framing,
    marker: Vec<u8>,
    pending: Vec<u8>,
    deadline: Option<Instant>,
}

impl Framer {
    pub fn new(mode: Framing) -> Result<Self> {
        let marker = match &mode {
            Framing::Header { hex } | Framing::Tail { hex } => {
                let marker = parse_hex(hex)?;
                ensure!((1..=32).contains(&marker.len()), "分帧标记须为 1–32 字节");
                marker
            }
            Framing::Idle { ms } => {
                ensure!((1..=10_000).contains(ms), "静默间隔须为 1–10000 毫秒");
                Vec::new()
            }
            Framing::Fixed { bytes } => {
                ensure!(
                    (1..=MAX_FRAME).contains(bytes),
                    "固定帧长度须为 1–4096 字节"
                );
                Vec::new()
            }
            Framing::Read => Vec::new(),
        };
        Ok(Self {
            mode,
            marker,
            ..Default::default()
        })
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    pub fn flush(&mut self) -> Vec<u8> {
        self.deadline = None;
        std::mem::take(&mut self.pending)
    }

    pub fn push(&mut self, bytes: &[u8], now: Instant) -> Vec<Vec<u8>> {
        if matches!(self.mode, Framing::Read) {
            return vec![bytes.to_vec()];
        }
        let mut frames = Vec::new();
        if self.deadline.is_some_and(|deadline| now >= deadline) && !self.pending.is_empty() {
            frames.push(self.flush());
        }
        for &byte in bytes {
            self.pending.push(byte);
            match self.mode {
                Framing::Fixed { bytes } if self.pending.len() == bytes => {
                    frames.push(self.flush())
                }
                Framing::Tail { .. } if self.pending.ends_with(&self.marker) => {
                    frames.push(self.flush())
                }
                Framing::Header { .. } if self.pending.ends_with(&self.marker) => {
                    let start = self.pending.len() - self.marker.len();
                    if start != 0 {
                        let next = self.pending.split_off(start);
                        frames.push(std::mem::replace(&mut self.pending, next));
                    }
                }
                _ => {}
            }
            if self.pending.len() >= MAX_FRAME {
                // Keep a possible marker prefix at the split so a later chunk can resynchronize.
                let keep = self.marker.len().saturating_sub(1);
                let next = self.pending.split_off(MAX_FRAME - keep);
                frames.push(std::mem::replace(&mut self.pending, next));
            }
        }
        if let Framing::Idle { ms } = self.mode {
            self.deadline = Some(now + Duration::from_millis(ms));
        }
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_markers_survive_chunk_boundaries_and_adjacent_frames() {
        for (mode, first, next, expected) in [
            (
                Framing::Header {
                    hex: "AA BB".into(),
                },
                &b"\xaa"[..],
                &b"\xbb\x01\xaa\xbb\x02"[..],
                &b"\xaa\xbb\x01"[..],
            ),
            (
                Framing::Tail {
                    hex: "0D 0A".into(),
                },
                &b"A\r"[..],
                &b"\nB"[..],
                &b"A\r\n"[..],
            ),
        ] {
            let mut framer = Framer::new(mode).unwrap();
            assert!(framer.push(first, Instant::now()).is_empty());
            assert_eq!(framer.push(next, Instant::now()), [expected]);
        }
    }
}
