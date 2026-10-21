use super::types::ChecksumAlgorithm;

pub(super) fn append_checksum(bytes: &mut Vec<u8>, algorithm: ChecksumAlgorithm) {
    match algorithm {
        ChecksumAlgorithm::None => {}
        ChecksumAlgorithm::Sum8 => {
            let sum = bytes.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte));
            bytes.push(sum);
        }
        ChecksumAlgorithm::Xor8 => {
            let xor = bytes.iter().fold(0u8, |xor, byte| xor ^ byte);
            bytes.push(xor);
        }
        ChecksumAlgorithm::OddParity | ChecksumAlgorithm::EvenParity => {
            let parity = bytes
                .iter()
                .fold(0, |parity, byte| parity ^ (byte.count_ones() & 1))
                as u8;
            bytes.push(parity ^ u8::from(algorithm == ChecksumAlgorithm::OddParity));
        }
        ChecksumAlgorithm::Crc16Modbus => {
            let mut crc = 0xffffu16;
            for byte in bytes.iter() {
                crc ^= u16::from(*byte);
                for _ in 0..8 {
                    crc = if crc & 1 != 0 {
                        (crc >> 1) ^ 0xa001
                    } else {
                        crc >> 1
                    };
                }
            }
            bytes.extend_from_slice(&crc.to_le_bytes());
        }
    }
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
