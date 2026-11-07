import Foundation

struct SerialConnectionConfig: Codable, Equatable, Sendable {
    var baudRate = 115200
    var dataBits = 8
    var stopBits = 1
    var parity = "none"
    var flowControl = "none"

    var summary: String { "\(baudRate) · \(dataBits)\(parity == "none" ? "N" : parity == "even" ? "E" : "O")\(stopBits)" }
}

struct SerialPort: Identifiable, Equatable, Sendable {
    let path: String
    let displayName: String
    let manufacturer: String?
    let serialNumber: String?
    let vid: String?
    let pid: String?
    var id: String { path }
}

struct SerialConnectionStatus: Equatable, Sendable {
    var connected = false
    var jobKind: String?
    var jobDone: UInt64 = 0
    var jobTotal: UInt64 = 0
    var jobError: String?
    var uptimeMs: UInt64 = 0
    var rxPerSecond: UInt64 = 0
    var txPerSecond: UInt64 = 0
    var logging = false
    var capturing = false
    var captureError: String?
    var feedback: String?
    var reconnecting = false
    var generation: UInt64 = 0
    var rts: Bool?
    var dtr: Bool?
    var path: String?
    var config: SerialConnectionConfig?
    var rxBytes: UInt64 = 0
    var txBytes: UInt64 = 0
    var openedAt: String?
    var lastError: String?
}

enum SerialDataFormat: String, Codable, CaseIterable, Identifiable, Sendable {
    case hex, ascii, gbk, utf8 = "utf-8", latin1 = "latin-1"
    var id: String { rawValue }
    var title: String { rawValue.uppercased() }
}

enum SerialChecksum: String, Codable, CaseIterable, Identifiable, Sendable {
    case none, sum8, oddParity, evenParity, xor8, crc16Modbus, crc16Ccitt, crc32, lrc
    var id: Self { self }
    var title: String {
        switch self {
        case .none: "无校验"
        case .sum8: "校验和"
        case .oddParity: "奇校验"
        case .evenParity: "偶校验"
        case .xor8: "异或校验"
        case .crc16Modbus: "CRC16/MODBUS"
        case .crc16Ccitt: "CRC16/CCITT"
        case .crc32: "CRC32"
        case .lrc: "LRC"
        }
    }
}

enum SerialBaudRate {
    static let presets = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600]

    static func parse(_ text: String) -> Int? {
        let value = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty, value.utf8.allSatisfy({ (48...57).contains($0) }),
              let rate = UInt32(value), rate > 0 else { return nil }
        return Int(rate)
    }
}

enum SerialDirectionFilter: String, CaseIterable, Identifiable, Sendable {
    case all = "全部"
    case receive = "接收"
    case send = "发送"
    var id: Self { self }
}

struct SerialFrame: Identifiable, Encodable, Sendable {
    var id = UUID()
    let direction: String
    let bytes: [UInt8]
    let hex: String
    let text: [String: String]
    let timestamp: String
    var source = "serial"
    var sequence: UInt64 = 0
    var parsed: [SerialProtocolRecord] = []

    private enum CodingKeys: String, CodingKey {
        case direction, bytes, hex, text, timestamp
    }

    var isReceive: Bool { direction == "receive" }
    @MainActor private static let timestampParser: ISO8601DateFormatter = {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter
    }()
    @MainActor private static let timeFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "HH:mm:ss.SSS"
        return formatter
    }()
    @MainActor var receivedAt: Date? { Self.timestampParser.date(from: timestamp) }
    @MainActor var time: String {
        guard let date = Self.timestampParser.date(from: timestamp) else { return timestamp }
        return Self.timeFormatter.string(from: date)
    }
    var memoryCost: Int { bytes.count * 16 + 1024 }
    func display(_ format: SerialDataFormat) -> String {
        if format == .hex { return hex }
        return (text[format.rawValue] ?? "")
            .replacingOccurrences(of: "\0", with: "␀")
            .replacingOccurrences(of: "\r", with: "␍")
            .replacingOccurrences(of: "\n", with: "␊\n")
            .replacingOccurrences(of: "\t", with: "⇥")
    }
}

struct SerialPollBatch: Sendable {
    let status: SerialConnectionStatus
    let frames: [SerialFrame]
    let droppedFrames: UInt64
    let nextCursor: UInt64
}

struct SerialPreferences: Codable, Sendable {
    var config = SerialConnectionConfig()
    var selectedPath = ""
    var autoConnect = false
    var autoReconnect = false
    var packets = "[]"
    var replies = "[]"
    var mixed = false
    var timestamps = true
    var resetOnClear = false
    var colors: [SerialColorRule] = []
    var txColor = ""
    var hexLineEnding = false
    var skipPrefix: UInt32 = 0
    var skipSuffix: UInt32 = 0
    var framingMode = "read"
    var framingValue = ""
    var draft = ""
    var history: [String] = []
    let version = 1
    var displayFormat: SerialDataFormat = .utf8
    var sendFormat: SerialDataFormat = .utf8
    var lineEnding = "none"
    var autoScroll = true
    var checksum: SerialChecksum = .none
    var autoSendIntervalMilliseconds = 1000

    private enum CodingKeys: String, CodingKey {
        case version, packets, replies, mixed, timestamps, resetOnClear, colors, txColor, draft, history, hexLineEnding, skipPrefix, skipSuffix, framingMode, framingValue, config, selectedPath, autoConnect, autoReconnect, displayFormat, sendFormat, lineEnding, autoScroll, checksum, autoSendIntervalMilliseconds
    }

    init() {}

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        packets = try values.decodeIfPresent(String.self, forKey: .packets) ?? "[]"
        replies = try values.decodeIfPresent(String.self, forKey: .replies) ?? "[]"
        mixed = try values.decodeIfPresent(Bool.self, forKey: .mixed) ?? false
        timestamps = try values.decodeIfPresent(Bool.self, forKey: .timestamps) ?? true
        resetOnClear = try values.decodeIfPresent(Bool.self, forKey: .resetOnClear) ?? false
        colors = try values.decodeIfPresent([SerialColorRule].self, forKey: .colors) ?? []
        txColor = try values.decodeIfPresent(String.self, forKey: .txColor) ?? ""
        hexLineEnding = try values.decodeIfPresent(Bool.self, forKey: .hexLineEnding) ?? false
        skipPrefix = try values.decodeIfPresent(UInt32.self, forKey: .skipPrefix) ?? 0
        skipSuffix = try values.decodeIfPresent(UInt32.self, forKey: .skipSuffix) ?? 0
        framingMode = try values.decodeIfPresent(String.self, forKey: .framingMode) ?? "read"
        framingValue = try values.decodeIfPresent(String.self, forKey: .framingValue) ?? ""
        draft = try values.decodeIfPresent(String.self, forKey: .draft) ?? ""
        history = try values.decodeIfPresent([String].self, forKey: .history) ?? []
        config = try values.decodeIfPresent(SerialConnectionConfig.self, forKey: .config) ?? SerialConnectionConfig()
        selectedPath = try values.decodeIfPresent(String.self, forKey: .selectedPath) ?? ""
        autoConnect = try values.decodeIfPresent(Bool.self, forKey: .autoConnect) ?? false
        autoReconnect = try values.decodeIfPresent(Bool.self, forKey: .autoReconnect) ?? false
        displayFormat = try values.decodeIfPresent(SerialDataFormat.self, forKey: .displayFormat) ?? .utf8
        sendFormat = try values.decodeIfPresent(SerialDataFormat.self, forKey: .sendFormat) ?? .utf8
        lineEnding = try values.decodeIfPresent(String.self, forKey: .lineEnding) ?? "none"
        autoScroll = try values.decodeIfPresent(Bool.self, forKey: .autoScroll) ?? true
        checksum = try values.decodeIfPresent(SerialChecksum.self, forKey: .checksum) ?? .none
        autoSendIntervalMilliseconds = try values.decodeIfPresent(Int.self, forKey: .autoSendIntervalMilliseconds) ?? 1000
    }

    static func load() -> Self {
        guard let data = UserDefaults.standard.data(forKey: "serial.preferences"),
              let value = try? JSONDecoder().decode(Self.self, from: data) else { return Self() }
        return value
    }

    func save() {
        if let data = try? JSONEncoder().encode(self) { UserDefaults.standard.set(data, forKey: "serial.preferences") }
    }
}

struct SerialProtocolRule: Codable, Identifiable, Sendable, Equatable {
    var name = "new-command"
    var template = "AA {value:u16be} {checksum:sum8}"
    var display = ""
    var receive = false
    var checksum_start = 0
    var checksum_end: Int?
    var response = ""
    var timeout_ms = 1000
    var delimiter = ""
    var length_field = ""
    var length_includes_frame = false
    var trailer = ""
    var id: String { name }
}
struct SerialProtocolResult: Decodable, Sendable {
    var hex: String
    var buffer: String
    var records: [SerialProtocolRecord]
    var response: String
    var timeout_ms: Int
    var layout: [SerialProtocolSpan]?
}

struct SerialProtocolSpan: Decodable, Sendable {
    let name: String
    let start: Int
    let bytes: Int
    let hex: String
}
struct SerialProtocolRecord: Decodable, Sendable, Identifiable {
    var id = UUID()
    var name: String
    var hex: String
    var message: String
    var valid: Bool
    var checksumValid: Bool?
    var payloadHex: String?
    private enum CodingKeys: String, CodingKey { case name, hex, message, valid, checksumValid, payloadHex }
}

extension SerialProtocolRule {
    private enum CodingKeys: String, CodingKey { case name, template, display, receive, checksum_start, checksum_end, response, timeout_ms, delimiter, length_field, length_includes_frame, trailer }
    init(from decoder: Decoder) throws {
        self.init()
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = try c.decode(String.self, forKey: .name)
        template = try c.decode(String.self, forKey: .template)
        display = try c.decodeIfPresent(String.self, forKey: .display) ?? ""
        receive = try c.decodeIfPresent(Bool.self, forKey: .receive) ?? false
        checksum_start = try c.decodeIfPresent(Int.self, forKey: .checksum_start) ?? 0
        checksum_end = try c.decodeIfPresent(Int.self, forKey: .checksum_end)
        response = try c.decodeIfPresent(String.self, forKey: .response) ?? ""
        timeout_ms = try c.decodeIfPresent(Int.self, forKey: .timeout_ms) ?? 1000
        delimiter = try c.decodeIfPresent(String.self, forKey: .delimiter) ?? ""
        length_field = try c.decodeIfPresent(String.self, forKey: .length_field) ?? ""
        length_includes_frame = try c.decodeIfPresent(Bool.self, forKey: .length_includes_frame) ?? false
        trailer = try c.decodeIfPresent(String.self, forKey: .trailer) ?? ""
    }
}

/// Editable fragments for the currently selected frame, never whole rules.
enum SerialFrameFragment: String, CaseIterable, Identifiable {
    case header, u8, u16be, u16le, i16be, u32be, sum8, xor8, crc16
    var id: String { rawValue }
    var title: String {
        switch self {
        case .header: "帧头 AA BB"
        case .sum8: "SUM8"
        case .xor8: "XOR8"
        case .crc16: "CRC16-Modbus"
        default: rawValue
        }
    }
    var token: String {
        switch self {
        case .header: "AA BB"
        case .sum8: "{checksum:sum8}"
        case .xor8: "{checksum:xor8}"
        case .crc16: "{checksum:crc16modbus}"
        default: "{value:\(rawValue)}"
        }
    }
    func inserting(into template: String) -> String {
        var tokens = template.split(whereSeparator: \.isWhitespace).map(String.init)
        if token.hasPrefix("{checksum:") {
            tokens.removeAll { $0.hasPrefix("{checksum:") }
            tokens.append(token)
        } else {
            var fragment = token
            if token.hasPrefix("{value:") {
                var name = "value"
                var suffix = 2
                while tokens.contains(where: { $0.hasPrefix("{\(name):") }) {
                    name = "value\(suffix)"; suffix += 1
                }
                fragment = token.replacingOccurrences(of: "{value:", with: "{\(name):")
            }
            if self == .header { tokens.insert(contentsOf: fragment.components(separatedBy: " "), at: 0) }
            else { tokens.insert(fragment, at: tokens.firstIndex(where: { $0.hasPrefix("{checksum:") }) ?? tokens.count) }
        }
        return tokens.joined(separator: " ")
    }
}

struct SerialColorRule: Codable, Equatable, Sendable {
    var pattern: String
    var regex: Bool
    var color: String

    static let presets: [(String, [Self])] = [
        ("ESP-IDF", [.init(pattern: "^[EWe] \\(", regex: true, color: "orange"), .init(pattern: "^[IDV] \\(", regex: true, color: "blue")]),
        ("日志等级", [.init(pattern: "ERROR|FATAL", regex: true, color: "red"), .init(pattern: "WARN", regex: false, color: "orange"),
                     .init(pattern: "INFO", regex: false, color: "blue")]),
        ("AT 响应", [.init(pattern: "ERROR", regex: false, color: "red"), .init(pattern: "OK", regex: false, color: "green")]),
    ]
}
