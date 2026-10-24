import Foundation

/// An editable value snapshot. Editing or abandoning it cannot alter a session.
struct TargetConfiguration {
    var selectedProbe: String
    var chip: String
    var wireProtocol: WireProtocol
    var speed: String
    var connectUnderReset: Bool

    var validSpeed: UInt32? {
        let value = speed.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty, value.utf8.allSatisfy({ (48...57).contains($0) }),
              let speed = UInt32(value), speed > 0 else { return nil }
        return speed
    }
}
