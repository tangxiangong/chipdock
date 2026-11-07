//! Interoptopus owns the ABI, wire encoding and managed memory wrappers.
//! Prefix exports so they cannot collide with imported Windows/Winsock symbols.
use super::BridgeError;
use chipdock_core::serial::{PollBatch, types::*};
use chipdock_service::serial as service;
use interoptopus::{
    ffi,
    lang::types::{TypeInfo, WireIO},
    pattern::result::panic_to_result,
    wire::Wire,
};

#[ffi]
pub struct SerialConnectRequest {
    pub session_id: Option<u64>,
    pub reconnect: bool,
    pub path: String,
    pub config: SerialConfig,
}
#[ffi]
pub struct SerialSendRequest {
    pub session_id: Option<u64>,
    pub text: String,
    pub format: SendFormat,
    pub line_ending: LineEnding,
    pub checksum: ChecksumAlgorithm,
    pub options: Option<SendOptions>,
}
#[ffi]
pub struct SerialStatusResponse {
    pub error: Option<String>,
}
#[ffi]
pub struct SerialPortsResponse {
    pub payload: Vec<PortInfo>,
    pub error: Option<String>,
}
#[ffi]
pub struct SerialPollResponse {
    pub payload: Option<PollBatch>,
    pub error: Option<String>,
}
#[ffi]
pub struct SerialConnectionResponse {
    pub payload: Option<SerialStatus>,
    pub error: Option<String>,
}

fn invoke<T: TypeInfo + WireIO>(work: impl FnOnce() -> T) -> ffi::Result<Wire<T>, BridgeError> {
    panic_to_result(|| {
        Wire::try_from(work())
            .map_err(|_| BridgeError::Serialization)
            .into()
    })
}
fn request<T: TypeInfo + WireIO>(mut wire: Wire<T>) -> Result<T, BridgeError> {
    wire.try_unwire().map_err(|_| BridgeError::Serialization)
}

#[ffi]
pub fn chipdock_serial_ports() -> ffi::Result<Wire<SerialPortsResponse>, BridgeError> {
    invoke(|| match service::serial_ports() {
        Ok(payload) => SerialPortsResponse {
            payload,
            error: None,
        },
        Err(error) => SerialPortsResponse {
            payload: Vec::new(),
            error: Some(error.to_string()),
        },
    })
}
#[ffi]
pub fn chipdock_serial_poll() -> ffi::Result<Wire<SerialPollResponse>, BridgeError> {
    chipdock_serial_session_poll(service::DEFAULT_SESSION)
}

#[ffi]
pub fn chipdock_serial_session_poll(id: u64) -> ffi::Result<Wire<SerialPollResponse>, BridgeError> {
    poll_response(service::serial_session_poll(id))
}

#[ffi]
pub fn chipdock_serial_session_records(
    id: u64,
    cursor: u64,
    limit: u32,
) -> ffi::Result<Wire<SerialPollResponse>, BridgeError> {
    poll_response(service::serial_session_records(id, cursor, limit))
}

fn poll_response(
    result: Result<PollBatch, service::SerialError>,
) -> ffi::Result<Wire<SerialPollResponse>, BridgeError> {
    invoke(|| match result {
        Ok(payload) => SerialPollResponse {
            payload: Some(payload),
            error: None,
        },
        Err(error) => SerialPollResponse {
            payload: None,
            error: Some(error.to_string()),
        },
    })
}
#[ffi]
pub fn chipdock_serial_connect(
    input: Wire<SerialConnectRequest>,
) -> ffi::Result<Wire<SerialConnectionResponse>, BridgeError> {
    panic_to_result(|| {
        let input = match request(input) {
            Ok(value) => value,
            Err(error) => return ffi::Result::Err(error),
        };
        invoke(|| {
            connection(service::serial_session_connect(
                input.session_id.unwrap_or(service::DEFAULT_SESSION),
                input.path,
                input.config,
                input.reconnect,
            ))
        })
    })
}
fn connection(result: Result<SerialStatus, service::SerialError>) -> SerialConnectionResponse {
    match result {
        Ok(payload) => SerialConnectionResponse {
            payload: Some(payload),
            error: None,
        },
        Err(error) => SerialConnectionResponse {
            payload: None,
            error: Some(error.to_string()),
        },
    }
}
#[ffi]
pub fn chipdock_serial_disconnect() -> ffi::Result<Wire<SerialConnectionResponse>, BridgeError> {
    chipdock_serial_session_disconnect(service::DEFAULT_SESSION)
}

#[ffi]
pub fn chipdock_serial_session_disconnect(
    id: u64,
) -> ffi::Result<Wire<SerialConnectionResponse>, BridgeError> {
    invoke(|| connection(service::serial_session_disconnect(id)))
}
#[ffi]
pub fn chipdock_serial_send(
    input: Wire<SerialSendRequest>,
) -> ffi::Result<Wire<SerialStatusResponse>, BridgeError> {
    panic_to_result(|| {
        let input = match request(input) {
            Ok(value) => value,
            Err(error) => return ffi::Result::Err(error),
        };
        invoke(|| SerialStatusResponse {
            error: service::serial_session_send_options(
                input.session_id.unwrap_or(service::DEFAULT_SESSION),
                input.text,
                input.format,
                input.line_ending,
                input.checksum,
                input.options.unwrap_or_default(),
            )
            .err()
            .map(|error| error.to_string()),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wire_preserves_serial_config_and_transport_errors() {
        let config = SerialConfig::default();
        let request = Wire::from(SerialConnectRequest {
            session_id: None,
            reconnect: false,
            path: String::new(),
            config,
        });
        let response = chipdock_serial_connect(request).unwrap().unwire();
        assert!(response.error.unwrap().contains("请选择串口"));
        let request = Wire::from(SerialSendRequest {
            session_id: None,
            options: None,
            text: "中文".into(),
            format: SendFormat::Ascii,
            line_ending: LineEnding::Crlf,
            checksum: ChecksumAlgorithm::None,
        });
        let response = chipdock_serial_send(request).unwrap().unwire();
        assert!(response.error.unwrap().contains("ASCII"));
    }

    #[test]
    fn protocol_wire_previews_without_a_connection() {
        let json = r#"{"rules":[{"name":"speed","template":"AA BB {v:u16be} {checksum:sum8}"}],"operation":"encode","input":"speed 45"}"#;
        let response =
            chipdock_serial_program(Wire::from(SerialProgramRequest { json: json.into() }))
                .unwrap()
                .unwire();
        assert!(response.error.is_none());
        assert!(response.json.contains("AA BB 00 2D 92"));
        let response =
            chipdock_serial_program(Wire::from(SerialProgramRequest { json: "{}".into() }))
                .unwrap()
                .unwire();
        assert!(response.error.is_some());
    }

    #[test]
    fn wire_preserves_every_checksum_selection() {
        for checksum in [
            ChecksumAlgorithm::None,
            ChecksumAlgorithm::Sum8,
            ChecksumAlgorithm::OddParity,
            ChecksumAlgorithm::EvenParity,
            ChecksumAlgorithm::Xor8,
            ChecksumAlgorithm::Crc16Modbus,
        ] {
            let mut request = Wire::from(SerialSendRequest {
                session_id: None,
                options: None,
                text: "01 03".into(),
                format: SendFormat::Hex,
                line_ending: LineEnding::None,
                checksum,
            });
            assert_eq!(request.unwire().checksum, checksum);
        }
    }
}

#[ffi]
pub struct SerialProgramRequest {
    pub json: String,
}
#[ffi]
pub struct SerialProgramResponse {
    pub json: String,
    pub error: Option<String>,
}
#[ffi]
pub fn chipdock_serial_program(
    input: Wire<SerialProgramRequest>,
) -> ffi::Result<Wire<SerialProgramResponse>, BridgeError> {
    panic_to_result(|| {
        let input = match request(input) {
            Ok(value) => value,
            Err(error) => return ffi::Result::Err(error),
        };
        invoke(
            || match chipdock_core::serial::program::execute(&input.json) {
                Ok(json) => SerialProgramResponse { json, error: None },
                Err(error) => SerialProgramResponse {
                    json: String::new(),
                    error: Some(error.to_string()),
                },
            },
        )
    })
}

#[ffi]
pub struct SerialSessionCreated {
    pub id: u64,
    pub error: Option<String>,
}

#[ffi]
pub fn chipdock_serial_create_session() -> ffi::Result<Wire<SerialSessionCreated>, BridgeError> {
    invoke(|| match service::serial_create_session() {
        Ok(id) => SerialSessionCreated { id, error: None },
        Err(error) => SerialSessionCreated {
            id: 0,
            error: Some(error.to_string()),
        },
    })
}

#[ffi]
pub fn chipdock_serial_close_session(
    id: u64,
) -> ffi::Result<Wire<SerialStatusResponse>, BridgeError> {
    invoke(|| SerialStatusResponse {
        error: service::serial_close_session(id)
            .err()
            .map(|error| error.to_string()),
    })
}

#[ffi]
pub fn chipdock_serial_shutdown() -> ffi::Result<Wire<SerialStatusResponse>, BridgeError> {
    invoke(|| SerialStatusResponse {
        error: service::serial_shutdown()
            .err()
            .map(|error| error.to_string()),
    })
}

#[ffi]
pub fn chipdock_serial_session_set_line(
    id: u64,
    rts: bool,
    enabled: bool,
) -> ffi::Result<Wire<SerialConnectionResponse>, BridgeError> {
    invoke(|| {
        connection(service::serial_session_set_line(
            id,
            if rts {
                ControlLine::Rts
            } else {
                ControlLine::Dtr
            },
            enabled,
        ))
    })
}

#[ffi]
pub fn chipdock_serial_workbench(
    id: u64,
    input: Wire<SerialProgramRequest>,
) -> ffi::Result<Wire<SerialProgramResponse>, BridgeError> {
    panic_to_result(|| {
        let input = match request(input) {
            Ok(value) => value,
            Err(error) => return ffi::Result::Err(error),
        };
        invoke(|| match service::serial_workbench(id, &input.json) {
            Ok(json) => SerialProgramResponse { json, error: None },
            Err(error) => SerialProgramResponse {
                json: String::new(),
                error: Some(error.to_string()),
            },
        })
    })
}

#[ffi]
pub fn chipdock_serial_is_busy() -> bool {
    service::serial_is_busy().unwrap_or(true)
}

#[ffi]
pub fn chipdock_serial_ipc_configure(
    mode: u8,
) -> ffi::Result<Wire<SerialStatusResponse>, BridgeError> {
    invoke(|| SerialStatusResponse {
        error: service::ipc::configure(mode)
            .err()
            .map(|error| error.to_string()),
    })
}

#[ffi]
pub struct SerialPlotRequest {
    pub id: u64,
    pub session_id: u64,
}

#[ffi]
pub fn chipdock_serial_ipc_take_plot() -> ffi::Result<Wire<SerialPlotRequest>, BridgeError> {
    invoke(|| {
        let (id, session_id) = service::ipc::take_plot().unwrap_or_default();
        SerialPlotRequest { id, session_id }
    })
}

#[ffi]
pub struct SerialPlotCompletion {
    pub id: u64,
    pub failure: Option<String>,
}

#[ffi]
pub fn chipdock_serial_ipc_finish_plot(
    input: Wire<SerialPlotCompletion>,
) -> ffi::Result<Wire<SerialStatusResponse>, BridgeError> {
    panic_to_result(|| {
        let input = match request(input) {
            Ok(value) => value,
            Err(error) => return ffi::Result::Err(error),
        };
        service::ipc::finish_plot(input.id, input.failure);
        invoke(|| SerialStatusResponse { error: None })
    })
}
