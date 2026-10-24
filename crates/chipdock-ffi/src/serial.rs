use crate::records::*;
use chipdock_service::serial;

#[uniffi::export]
pub fn serial_ports() -> Result<Vec<PortInfo>, SerialError> {
    serial::serial_ports()
}

#[uniffi::export]
pub fn serial_poll() -> Result<PollBatch, SerialError> {
    serial::serial_poll()
}

#[uniffi::export]
pub fn serial_connect(path: String, config: SerialConfig) -> Result<SerialStatus, SerialError> {
    serial::serial_connect(path, config)
}

#[uniffi::export]
pub fn serial_disconnect() -> Result<SerialStatus, SerialError> {
    serial::serial_disconnect()
}

#[uniffi::export]
pub fn serial_send(
    input: String,
    format: SendFormat,
    line_ending: LineEnding,
) -> Result<(), SerialError> {
    serial::serial_send(input, format, line_ending)
}

#[uniffi::export]
pub fn serial_send_with_checksum(
    input: String,
    format: SendFormat,
    line_ending: LineEnding,
    checksum: ChecksumAlgorithm,
) -> Result<(), SerialError> {
    serial::serial_send_with_checksum(input, format, line_ending, checksum)
}
