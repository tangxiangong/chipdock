import Foundation

func addressLabel(_ value: UInt64?) -> String {
    guard let value else { return "—" }
    return String(format: "0x%08llX", value)
}

func byteCountLabel(_ value: UInt64) -> String {
    if value >= 1_048_576 { return String(format: "%.1f MiB", Double(value) / 1_048_576) }
    if value >= 1024 { return String(format: "%.1f KiB", Double(value) / 1024) }
    return "\(value) B"
}

func parseAddress(_ text: String) -> UInt64? {
    let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
    if trimmed.lowercased().hasPrefix("0x") {
        return UInt64(trimmed.dropFirst(2), radix: 16)
    }
    return UInt64(trimmed, radix: 10)
}

extension FirmwareFormat {
    var title: String {
        switch self {
        case .elf: "ELF"
        case .hex: "HEX"
        case .bin: "BIN"
        }
    }
}

extension WireProtocol {
    var title: String { self == .swd ? "SWD" : "JTAG" }
}

extension JobStage {
    var isTerminal: Bool { self == .completed || self == .failed }
}

extension ProbeSummary: Identifiable {
    public var id: String { identifier }
    var displayName: String { product ?? identifier }
}

extension SavedProfile: Identifiable {}

extension MemoryReadResult {
    var data: Data {
        var bytes = [UInt8]()
        var index = dataHex.startIndex
        while index < dataHex.endIndex {
            let end = dataHex.index(index, offsetBy: 2, limitedBy: dataHex.endIndex) ?? dataHex.endIndex
            guard let value = UInt8(dataHex[index..<end], radix: 16) else { return Data() }
            bytes.append(value)
            index = end
        }
        return Data(bytes)
    }

    var lines: [MemoryLine] {
        let bytes = Array(data)
        return stride(from: 0, to: bytes.count, by: 16).map { offset in
            MemoryLine(address: address + UInt64(offset), bytes: Array(bytes[offset..<min(offset + 16, bytes.count)]))
        }
    }
}
