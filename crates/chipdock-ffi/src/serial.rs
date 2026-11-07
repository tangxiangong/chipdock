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

/// Validate, preview and decode serial protocol templates without hardware access.
#[uniffi::export]
pub fn serial_program(request: String) -> Result<String, SerialError> {
    chipdock_core::serial::program::execute(&request).map_err(Into::into)
}

#[uniffi::export]
pub fn serial_create_session() -> Result<u64, SerialError> {
    serial::serial_create_session()
}

#[uniffi::export]
pub fn serial_close_session(id: u64) -> Result<(), SerialError> {
    serial::serial_close_session(id)
}

#[uniffi::export]
pub fn serial_shutdown() -> Result<(), SerialError> {
    serial::serial_shutdown()
}

#[uniffi::export]
pub fn serial_session_poll(id: u64) -> Result<PollBatch, SerialError> {
    serial::serial_session_poll(id)
}

#[uniffi::export]
pub fn serial_session_connect(
    id: u64,
    path: String,
    config: SerialConfig,
    reconnect: bool,
) -> Result<SerialStatus, SerialError> {
    serial::serial_session_connect(id, path, config, reconnect)
}

#[uniffi::export]
pub fn serial_session_disconnect(id: u64) -> Result<SerialStatus, SerialError> {
    serial::serial_session_disconnect(id)
}

#[uniffi::export]
pub fn serial_session_set_line(
    id: u64,
    line: ControlLine,
    enabled: bool,
) -> Result<SerialStatus, SerialError> {
    serial::serial_session_set_line(id, line, enabled)
}

#[uniffi::export]
pub fn serial_session_send(
    id: u64,
    input: String,
    format: SendFormat,
    line_ending: LineEnding,
    checksum: ChecksumAlgorithm,
) -> Result<(), SerialError> {
    serial::serial_session_send(id, input, format, line_ending, checksum)
}

#[uniffi::export]
pub fn serial_session_send_options(
    id: u64,
    input: String,
    format: SendFormat,
    ending: LineEnding,
    checksum: ChecksumAlgorithm,
    options: SendOptions,
) -> Result<(), SerialError> {
    serial::serial_session_send_options(id, input, format, ending, checksum, options)
}

#[uniffi::export]
pub fn serial_workbench(id: u64, request: String) -> Result<String, SerialError> {
    serial::serial_workbench(id, &request)
}

#[uniffi::export]
pub fn serial_session_records(id: u64, cursor: u64, limit: u32) -> Result<PollBatch, SerialError> {
    serial::serial_session_records(id, cursor, limit)
}

#[uniffi::export]
pub fn serial_is_busy() -> Result<bool, SerialError> {
    serial::serial_is_busy()
}

#[uniffi::export]
pub fn serial_ipc_configure(mode: u8) -> Result<(), SerialError> {
    serial::ipc::configure(mode)
}

#[derive(uniffi::Record)]
pub struct SerialPlotRequest {
    pub id: u64,
    pub session_id: u64,
}

#[uniffi::export]
pub fn serial_ipc_take_plot() -> Option<SerialPlotRequest> {
    serial::ipc::take_plot().map(|(id, session_id)| SerialPlotRequest { id, session_id })
}

#[uniffi::export]
pub fn serial_ipc_finish_plot(id: u64, failure: Option<String>) {
    serial::ipc::finish_plot(id, failure);
}
