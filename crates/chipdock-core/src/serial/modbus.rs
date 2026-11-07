use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

use super::{bytes_to_hex, checksum, types::ChecksumAlgorithm};

const MAX_ADU: usize = 256;
const CRC: ChecksumAlgorithm = ChecksumAlgorithm::Crc16Modbus;

#[derive(Deserialize)]
pub struct Request {
    pub station: u8,
    pub function: u8,
    pub address: u16,
    #[serde(default)]
    pub quantity: u16,
    #[serde(default)]
    pub values: Vec<u16>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reply {
    pub hex: String,
    pub values: Vec<u16>,
    pub exception: Option<u8>,
}

impl Request {
    pub fn encode(&self) -> Result<Vec<u8>> {
        ensure!((1..=247).contains(&self.station), "Modbus 站号须为 1–247");
        let mut bytes = vec![self.station, self.function];
        bytes.extend(self.address.to_be_bytes());
        match self.function {
            1..=4 => {
                let max = if self.function <= 2 { 2000 } else { 125 };
                ensure!(
                    (1..=max).contains(&self.quantity) && self.values.is_empty(),
                    "读取数量须为 1–{max}"
                );
                bytes.extend(self.quantity.to_be_bytes());
            }
            5 | 6 => {
                let &[value] = self.values.as_slice() else {
                    bail!("请输入一个写入值")
                };
                let value = if self.function == 5 {
                    ensure!(value <= 1, "线圈值须为 0 或 1");
                    if value == 1 { 0xff00 } else { 0 }
                } else {
                    value
                };
                bytes.extend(value.to_be_bytes());
            }
            15 | 16 => {
                let count = self.values.len();
                let max = if self.function == 15 { 1968 } else { 123 };
                ensure!((1..=max).contains(&count), "写入数量须为 1–{max}");
                bytes.extend((count as u16).to_be_bytes());
                if self.function == 15 {
                    ensure!(
                        self.values.iter().all(|&value| value <= 1),
                        "线圈值须为 0 或 1"
                    );
                    bytes.push(count.div_ceil(8) as u8);
                    bytes.extend(self.values.chunks(8).map(|chunk| {
                        chunk
                            .iter()
                            .enumerate()
                            .fold(0, |bits, (bit, value)| bits | ((*value as u8) << bit))
                    }));
                } else {
                    bytes.push((count * 2) as u8);
                    bytes.extend(self.values.iter().flat_map(|value| value.to_be_bytes()));
                }
            }
            _ => bail!("不支持此 Modbus 功能码"),
        }
        let count = match self.function {
            1..=4 => self.quantity as usize,
            15 | 16 => self.values.len(),
            _ => 1,
        };
        ensure!(
            self.address as usize + count <= 65536,
            "Modbus 地址范围溢出"
        );
        checksum::append_checksum(&mut bytes, CRC);
        Ok(bytes)
    }

    pub(super) fn receive(&self, request: &[u8], buffer: &mut Vec<u8>) -> Option<Reply> {
        let mut consumed = 0;
        while let Some(bytes) = buffer.get(consumed..).filter(|bytes| bytes.len() >= 2) {
            let exception = bytes[1] == self.function | 0x80;
            if bytes[0] != self.station || (!exception && bytes[1] != self.function) {
                consumed += 1;
                continue;
            }
            let size = match (exception, self.function) {
                (true, _) => 5,
                (_, 1 | 2) => (self.quantity as usize).div_ceil(8) + 5,
                (_, 3 | 4) => self.quantity as usize * 2 + 5,
                _ => 8,
            };
            if (1..=4).contains(&self.function)
                && !exception
                && bytes.get(2).is_some_and(|n| *n as usize != size - 5)
            {
                consumed += 1;
                continue;
            }
            let Some(frame) = bytes.get(..size) else {
                break;
            };
            if checksum::calculate(&frame[..size - 2], CRC) != frame[size - 2..]
                || (!exception && self.function >= 5 && frame[..6] != request[..6])
            {
                consumed += 1;
                continue;
            }
            let values = if exception {
                vec![]
            } else {
                match self.function {
                    1 | 2 => (0..self.quantity as usize)
                        .map(|bit| u16::from((frame[3 + bit / 8] >> (bit % 8)) & 1))
                        .collect(),
                    3 | 4 => frame[3..size - 2]
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .copied()
                        .map(u16::from_be_bytes)
                        .collect(),
                    _ => vec![u16::from_be_bytes([frame[4], frame[5]])],
                }
            };
            return Some(Reply {
                hex: bytes_to_hex(frame),
                values,
                exception: exception.then_some(frame[2]),
            });
        }
        let keep_from = consumed.max(buffer.len().saturating_sub(MAX_ADU));
        buffer.drain(..keep_from);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_registers_validates_station_crc_and_decodes_exception() {
        let request = Request {
            station: 1,
            function: 3,
            address: 0,
            quantity: 1,
            values: vec![],
        };
        let command = request.encode().unwrap();
        assert_eq!(command, [1, 3, 0, 0, 0, 1, 0x84, 0x0a]);
        for (body, expected, exception) in [
            (vec![1, 3, 2, 0, 42], vec![42], None),
            (vec![1, 0x83, 2], vec![], Some(2)),
        ] {
            let mut response = body;
            checksum::append_checksum(&mut response, CRC);
            let mut buffer = vec![0xff, 1];
            assert!(request.receive(&command, &mut buffer).is_none());
            buffer.extend_from_slice(&response[1..]);
            let reply = request.receive(&command, &mut buffer).unwrap();
            assert_eq!(reply.values, expected);
            assert_eq!(reply.exception, exception);
        }
    }
}
