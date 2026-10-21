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
    var path: String?
    var config: SerialConnectionConfig?
    var rxBytes: UInt64 = 0
    var txBytes: UInt64 = 0
    var openedAt: String?
    var lastError: String?
}

enum SerialDataFormat: String, Codable, CaseIterable, Identifiable, Sendable {
    case hex, ascii, gbk, utf8 = "utf-8"
    var id: String { rawValue }
    var title: String { rawValue.uppercased() }
}

enum SerialChecksum: String, Codable, CaseIterable, Identifiable, Sendable {
    case none, sum8, oddParity, evenParity, xor8, crc16Modbus
    var id: Self { self }
    var title: String {
        switch self {
        case .none: "无校验"
        case .sum8: "校验和"
        case .oddParity: "奇校验"
        case .evenParity: "偶校验"
        case .xor8: "异或校验"
        case .crc16Modbus: "CRC16/MODBUS"
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

struct SerialFrame: Identifiable, Sendable {
    var id = UUID()
    let direction: String
    let bytes: [UInt8]
    let hex: String
    let text: [String: String]
    let timestamp: String

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
}

struct SerialPreferences: Codable, Sendable {
    var config = SerialConnectionConfig()
    var selectedPath = ""
    var displayFormat: SerialDataFormat = .utf8
    var sendFormat: SerialDataFormat = .utf8
    var lineEnding = "none"
    var autoScroll = true
    var checksum: SerialChecksum = .none
    var autoSendIntervalMilliseconds = 1000

    private enum CodingKeys: String, CodingKey {
        case config, selectedPath, displayFormat, sendFormat, lineEnding, autoScroll, checksum, autoSendIntervalMilliseconds
    }

    init() {}

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        config = try values.decodeIfPresent(SerialConnectionConfig.self, forKey: .config) ?? SerialConnectionConfig()
        selectedPath = try values.decodeIfPresent(String.self, forKey: .selectedPath) ?? ""
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
