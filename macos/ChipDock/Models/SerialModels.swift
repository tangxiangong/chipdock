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

    static func load() -> Self {
        guard let data = UserDefaults.standard.data(forKey: "serial.preferences"),
              let value = try? JSONDecoder().decode(Self.self, from: data) else { return Self() }
        return value
    }

    func save() {
        if let data = try? JSONEncoder().encode(self) { UserDefaults.standard.set(data, forKey: "serial.preferences") }
    }
}
