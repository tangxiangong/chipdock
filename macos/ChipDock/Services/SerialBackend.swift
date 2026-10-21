import Foundation

enum SerialClientError: LocalizedError {
    case operation(String)
    var errorDescription: String? { if case .operation(let text) = self { return text }; return nil }
}

/// Serial polling uses its own executor so firmware jobs never stall reception.
final class SerialBackend: Sendable {
    private static let worker = NativeWorker(name: "com.xiaoyu.chipdock.serial")
    private func run<T: Sendable>(_ work: @escaping @Sendable () throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            Self.worker.submit {
                do { continuation.resume(returning: try work()) }
                catch { continuation.resume(throwing: error) }
            }
        }
    }
    func ports() async throws -> [SerialPort] {
        try await run { [self] in
            try translated { try serialPorts() }.map {
                SerialPort(path: $0.path, displayName: $0.displayName, manufacturer: $0.manufacturer,
                           serialNumber: $0.serialNumber, vid: $0.vid, pid: $0.pid)
            }
        }
    }
    func poll() async throws -> SerialPollBatch {
        try await run { [self] in
            let batch = try translated { try serialPoll() }
            return SerialPollBatch(status: status(batch.status), frames: batch.frames.map {
                SerialFrame(direction: $0.direction == .receive ? "receive" : "send", bytes: Array($0.bytes), hex: $0.hex,
                            text: ["ascii": $0.text.ascii, "gbk": $0.text.gbk, "utf-8": $0.text.utf8], timestamp: $0.timestamp)
            }, droppedFrames: batch.droppedFrames)
        }
    }
    func connect(path: String, config: SerialConnectionConfig) async throws -> SerialConnectionStatus {
        try await run { [self] in
            guard let baudRate = UInt32(exactly: config.baudRate), let dataBits = UInt8(exactly: config.dataBits),
                  let stopBits = UInt8(exactly: config.stopBits) else { throw SerialClientError.operation("串口参数超出范围") }
            return status(try translated { try serialConnect(path: path, config: SerialConfig(
                baudRate: baudRate, dataBits: dataBits, stopBits: stopBits,
                parity: config.parity == "even" ? .even : config.parity == "odd" ? .odd : .none,
                flowControl: config.flowControl == "hardware" ? .hardware : config.flowControl == "software" ? .software : .none)) })
        }
    }
    func disconnect() async throws -> SerialConnectionStatus {
        try await run { [self] in
            status(try translated { try serialDisconnect() })
        }
    }
    func send(input: String, format: SerialDataFormat, lineEnding: String) async throws {
        try await run { [self] in
            let encoding: SendFormat = switch format { case .hex: .hex; case .ascii: .ascii; case .gbk: .gbk; case .utf8: .utf8 }
            let ending: LineEnding = switch lineEnding { case "lf": .lf; case "cr": .cr; case "crlf": .crlf; default: .none }
            try translated { try serialSend(input: input, format: encoding, lineEnding: ending) }
        }
    }
    private func translated<T>(_ operation: () throws -> T) throws -> T {
        do { return try operation() }
        catch let error as SerialError { switch error { case .Operation(let message): throw SerialClientError.operation(message) } }
    }
    private func status(_ value: SerialStatus) -> SerialConnectionStatus {
        let config = value.config.map { SerialConnectionConfig(baudRate: Int($0.baudRate), dataBits: Int($0.dataBits),
            stopBits: Int($0.stopBits), parity: $0.parity == .even ? "even" : $0.parity == .odd ? "odd" : "none",
            flowControl: $0.flowControl == .hardware ? "hardware" : $0.flowControl == .software ? "software" : "none") }
        return SerialConnectionStatus(connected: value.connected, path: value.path, config: config,
                                      rxBytes: value.rxBytes, txBytes: value.txBytes, openedAt: value.openedAt, lastError: value.lastError)
    }
}
