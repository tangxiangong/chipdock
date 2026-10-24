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
    pub path: String,
    pub config: SerialConfig,
}
#[ffi]
pub struct SerialSendRequest {
    pub text: String,
    pub format: SendFormat,
    pub line_ending: LineEnding,
    pub checksum: ChecksumAlgorithm,
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
    invoke(|| match service::serial_poll() {
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
        invoke(|| connection(service::serial_connect(input.path, input.config)))
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
    invoke(|| connection(service::serial_disconnect()))
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
            error: service::serial_send_with_checksum(
                input.text,
                input.format,
                input.line_ending,
                input.checksum,
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
            path: String::new(),
            config,
        });
        let response = chipdock_serial_connect(request).unwrap().unwire();
        assert!(response.error.unwrap().contains("请选择串口"));
        let request = Wire::from(SerialSendRequest {
            text: "中文".into(),
            format: SendFormat::Ascii,
            line_ending: LineEnding::Crlf,
            checksum: ChecksumAlgorithm::None,
        });
        let response = chipdock_serial_send(request).unwrap().unwire();
        assert!(response.error.unwrap().contains("ASCII"));
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
                text: "01 03".into(),
                format: SendFormat::Hex,
                line_ending: LineEnding::None,
                checksum,
            });
            assert_eq!(request.unwire().checksum, checksum);
        }
    }
}
