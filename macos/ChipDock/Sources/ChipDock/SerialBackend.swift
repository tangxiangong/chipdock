import Foundation

enum SerialClientError: LocalizedError {
    case operation(String)
    var errorDescription: String? { if case .operation(let text) = self { return text }; return nil }
}

/// Serial polling uses its own executor so firmware jobs never stall reception.
final class SerialBackend: Sendable {
    let id: UInt64
    init(id: UInt64 = 1) { self.id = id }

    func run<T: Sendable>(_ work: @escaping @Sendable () throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
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
    func poll(after cursor: UInt64) async throws -> SerialPollBatch {
        try await run { [self] in
            let batch = try translated { try serialSessionRecords(id: id, cursor: cursor, limit: 256) }
            return SerialPollBatch(status: status(batch.status), frames: batch.frames.map {
                SerialFrame(direction: $0.direction == .receive ? "receive" : "send", bytes: Array($0.bytes), hex: $0.hex,
                            text: ["ascii": $0.text.ascii, "gbk": $0.text.gbk, "utf-8": $0.text.utf8, "latin-1": $0.text.latin1],
                            timestamp: $0.timestamp, source: $0.source, sequence: $0.sequence, parsed: $0.parsed.map {
                                SerialProtocolRecord(name: $0.name, hex: $0.hex, message: $0.message, valid: $0.valid,
                                                     checksumValid: $0.checksumValid, payloadHex: $0.payloadHex)
                            })
            }, droppedFrames: batch.droppedFrames, nextCursor: batch.nextCursor)
        }
    }
    func connect(path: String, config: SerialConnectionConfig, reconnect: Bool = false) async throws -> SerialConnectionStatus {
        try await run { [self] in
            guard let baudRate = UInt32(exactly: config.baudRate), let dataBits = UInt8(exactly: config.dataBits),
                  let stopBits = UInt8(exactly: config.stopBits) else { throw SerialClientError.operation("串口参数超出范围") }
            return status(try translated { try serialSessionConnect(id: id, path: path, config: SerialConfig(
                baudRate: baudRate, dataBits: dataBits, stopBits: stopBits,
                parity: config.parity == "even" ? .even : config.parity == "odd" ? .odd : .none,
                flowControl: config.flowControl == "hardware" ? .hardware : config.flowControl == "software" ? .software : .none), reconnect: reconnect) })
        }
    }
    func disconnect() async throws -> SerialConnectionStatus {
        try await run { [self] in
            status(try translated { try serialSessionDisconnect(id: id) })
        }
    }
    func send(input: String, format: SerialDataFormat, lineEnding: String, checksum: SerialChecksum, options: SendOptions) async throws {
        try await run { [self] in
            let encoding: SendFormat = switch format { case .hex: .hex; case .ascii: .ascii; case .gbk: .gbk; case .utf8: .utf8; case .latin1: .latin1 }
            let ending: LineEnding = switch lineEnding { case "lf": .lf; case "cr": .cr; case "crlf": .crlf; default: .none }
            let algorithm: ChecksumAlgorithm = switch checksum {
            case .none: .none
            case .sum8: .sum8
            case .oddParity: .oddParity
            case .evenParity: .evenParity
            case .xor8: .xor8
            case .crc16Modbus: .crc16Modbus
            case .crc16Ccitt: .crc16Ccitt
            case .crc32: .crc32
            case .lrc: .lrc
            }
            try translated { try serialSessionSendOptions(id: id, input: input, format: encoding, ending: ending, checksum: algorithm, options: options) }
        }
    }
    func program(rules: [SerialProtocolRule], operation: String, input: String = "", buffer: String = "", incoming: String = "", rule: String = "") async throws -> SerialProtocolResult {
        struct Request: Encodable { var rules: [SerialProtocolRule]; var operation: String; var input: String; var buffer: String; var incoming: String; var rule: String }
        let request = Request(rules: rules, operation: operation, input: input, buffer: buffer, incoming: incoming, rule: rule)
        let json = String(decoding: try JSONEncoder().encode(request), as: UTF8.self)
        return try await run { [self] in
            let response = try translated { try serialProgram(request: json) }
            return try JSONDecoder().decode(SerialProtocolResult.self, from: Data(response.utf8))
        }
    }
    func workbench(_ request: String) async throws -> String {
        try await run { [self] in try translated { try serialWorkbench(id: id, request: request) } }
    }
    func setLocalControl(_ mode: UInt8) async throws {
        try await run { [self] in try translated { try serialIpcConfigure(mode: mode) } }
    }

    func close() async throws {
        try await run { [self] in try translated { try serialCloseSession(id: id) } }
    }
    func create() async throws -> UInt64 {
        try await run { [self] in try translated { try serialCreateSession() } }
    }
    func setLine(rts: Bool, enabled: Bool) async throws -> SerialConnectionStatus {
        try await run { [self] in
            status(try translated { try serialSessionSetLine(id: id, line: rts ? .rts : .dtr, enabled: enabled) })
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
        return SerialConnectionStatus(connected: value.connected, jobKind: value.jobKind,
            jobDone: value.jobDone, jobTotal: value.jobTotal, jobError: value.jobError, uptimeMs: value.uptimeMs,
            rxPerSecond: value.rxPerSecond, txPerSecond: value.txPerSecond, logging: value.logging, capturing: value.capturing,
            captureError: value.captureError, feedback: value.feedback, reconnecting: value.reconnecting, generation: value.generation, rts: value.rts, dtr: value.dtr, path: value.path, config: config,
                                      rxBytes: value.rxBytes, txBytes: value.txBytes, openedAt: value.openedAt, lastError: value.lastError)
    }
}
