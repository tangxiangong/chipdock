import Foundation
import SwiftUI

enum EraseStrategy: String, CaseIterable, Identifiable {
    case automatic, full, skip
    var id: Self { self }
    var title: String {
        switch self {
        case .automatic: "自动擦除所需扇区"
        case .full: "整片擦除"
        case .skip: "跳过擦除"
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
