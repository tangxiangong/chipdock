import Foundation
import SwiftUI

enum EraseStrategy: String, CaseIterable, Identifiable {
    case automatic, full, skip
    var id: Self { self }
    var title: String {
        switch self {
        case .automatic: "按需擦除"
        case .full: "整片擦除"
        case .skip: "跳过擦除"
        }
    }
    var detail: String {
        switch self {
        case .automatic: "仅擦除固件覆盖的扇区"
        case .full: "写入前擦除整个 Flash，其他数据会丢失"
        case .skip: "不擦除，目标区域须已擦除"
        }
    }
}

enum Appearance: String, CaseIterable, Identifiable {
    case system, light, dark
    var id: Self { self }
    var title: String {
        switch self {
        case .system: "跟随系统"
        case .light: "浅色"
        case .dark: "深色"
        }
    }
    var colorScheme: ColorScheme? {
        switch self {
        case .system: nil
        case .light: .light
        case .dark: .dark
        }
    }
}

struct ActivityEntry: Identifiable, Sendable {
    enum Level: String, Sendable { case info = "信息", success = "完成", error = "错误" }
    let id = UUID()
    var date = Date()
    var operation: String
    var level: Level
    var message: String
    var detail: String = ""
}

/// Text offsets move when older log entries are pruned. Keep selection endpoints
/// attached to entry identities so Copy cannot silently select another message.
enum LogTextSelection {
    static func restore(_ selections: [NSRange], from previous: [(UUID, NSRange)],
                        to current: [(UUID, NSRange)]) -> [NSRange] {
        let currentRanges = Dictionary(uniqueKeysWithValues: current)
        func anchor(at position: Int) -> (UUID, Int)? {
            if let entry = previous.first(where: { NSLocationInRange(position, $0.1) }) {
                return (entry.0, position - entry.1.location)
            }
            if let last = previous.last, position == last.1.upperBound { return (last.0, last.1.length) }
            return nil
        }
        return selections.compactMap { selection in
            guard selection.location != NSNotFound, selection.length <= Int.max - selection.location,
                  let start = anchor(at: selection.location),
                  let end = anchor(at: selection.upperBound - (selection.length > 0 ? 1 : 0)),
                  let startRange = currentRanges[start.0], let endRange = currentRanges[end.0] else { return nil }
            let startPosition = startRange.location + min(start.1, startRange.length)
            let endPosition = endRange.location + min(end.1 + (selection.length > 0 ? 1 : 0), endRange.length)
            guard endPosition >= startPosition else { return nil }
            return NSRange(location: startPosition, length: endPosition - startPosition)
        }
    }
}

struct UserFacingError: Identifiable {
    let id = UUID()
    var message: String
    var detail: String
    var recovery: String
}

struct MemoryLine: Identifiable {
    var address: UInt64
    var bytes: [UInt8]
    var id: UInt64 { address }
    var hex: String { bytes.map { String(format: "%02X", $0) }.joined(separator: " ") }
    var ascii: String { String(bytes.map { (32...126).contains($0) ? Character(UnicodeScalar($0)) : "·" }) }
}

/// The two tools occupy the same navigation level.
enum WorkbenchTool: Hashable {
    case flash, serial
}
