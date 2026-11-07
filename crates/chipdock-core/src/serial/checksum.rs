use super::types::ChecksumAlgorithm;

pub(super) fn append_checksum(bytes: &mut Vec<u8>, algorithm: ChecksumAlgorithm) {
    bytes.extend(calculate(bytes, algorithm));
}

pub fn width(algorithm: ChecksumAlgorithm) -> usize {
    match algorithm {
        ChecksumAlgorithm::None => 0,
        ChecksumAlgorithm::Crc16Modbus | ChecksumAlgorithm::Crc16Ccitt => 2,
        ChecksumAlgorithm::Crc32 => 4,
        _ => 1,
    }
}

pub fn calculate(bytes: &[u8], algorithm: ChecksumAlgorithm) -> Vec<u8> {
    match algorithm {
        ChecksumAlgorithm::None => Vec::new(),
        ChecksumAlgorithm::Sum8 | ChecksumAlgorithm::Lrc => {
            let sum = bytes.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte));
            vec![if algorithm == ChecksumAlgorithm::Lrc {
                sum.wrapping_neg()
            } else {
                sum
            }]
        }
        ChecksumAlgorithm::Xor8 => vec![bytes.iter().fold(0, |xor, byte| xor ^ byte)],
        ChecksumAlgorithm::OddParity | ChecksumAlgorithm::EvenParity => {
            let parity = bytes
                .iter()
                .fold(0, |parity, byte| parity ^ (byte.count_ones() & 1))
                as u8;
            vec![parity ^ u8::from(algorithm == ChecksumAlgorithm::OddParity)]
        }
        ChecksumAlgorithm::Crc16Modbus => (reflected_crc(bytes, 0xffff, 0xa001) as u16)
            .to_le_bytes()
            .to_vec(),
        ChecksumAlgorithm::Crc32 => (!reflected_crc(bytes, u32::MAX, 0xedb88320))
            .to_le_bytes()
            .to_vec(),
        ChecksumAlgorithm::Crc16Ccitt => {
            let mut crc = 0xffffu16;
            for byte in bytes {
                crc ^= u16::from(*byte) << 8;
                for _ in 0..8 {
                    crc = if crc & 0x8000 != 0 {
                        (crc << 1) ^ 0x1021
                    } else {
                        crc << 1
                    };
                }
            }
            crc.to_be_bytes().to_vec()
        }
    }
}

fn reflected_crc(bytes: &[u8], mut crc: u32, polynomial: u32) -> u32 {
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ polynomial
            } else {
                crc >> 1
            };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sum_wraps_to_one_byte() {
        let mut bytes = vec![0xff, 0x02];
        append_checksum(&mut bytes, ChecksumAlgorithm::Sum8);
        assert_eq!(bytes, [0xff, 0x02, 0x01]);
    }

    #[test]
    fn xor_uses_all_payload_bytes() {
        let mut bytes = vec![0x12, 0x34, 0x56];
        append_checksum(&mut bytes, ChecksumAlgorithm::Xor8);
        assert_eq!(bytes, [0x12, 0x34, 0x56, 0x70]);
    }

    #[test]
    fn parity_appends_the_bit_needed_for_the_entire_payload() {
        for (algorithm, data, expected) in [
            (ChecksumAlgorithm::EvenParity, vec![0xff, 0x01], 1),
            (ChecksumAlgorithm::OddParity, vec![0xff, 0x01], 0),
            (ChecksumAlgorithm::EvenParity, vec![0xff, 0x03], 0),
            (ChecksumAlgorithm::OddParity, vec![0xff, 0x03], 1),
        ] {
            let mut bytes = data.clone();
            append_checksum(&mut bytes, algorithm);
            assert_eq!(bytes, [data, vec![expected]].concat(), "{algorithm:?}");
        }
    }

    #[test]
    fn modbus_matches_the_standard_check_vector_in_wire_order() {
        let mut bytes = b"123456789".to_vec();
        append_checksum(&mut bytes, ChecksumAlgorithm::Crc16Modbus);
        assert_eq!(bytes, b"123456789\x37\x4b");
    }

    #[test]
    fn no_checksum_preserves_the_payload() {
        let mut bytes = vec![0x00, 0xff];
        append_checksum(&mut bytes, ChecksumAlgorithm::None);
        assert_eq!(bytes, [0x00, 0xff]);
    }
}
