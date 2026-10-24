import Foundation

/// Keep view state by frame identity, independent of an NSTableView's lifetime.
@MainActor
final class TerminalTableState {
    var columnWidths: [String: Double] = [:]
    var selectedIDs: Set<UUID> = []
    var topFrameID: UUID?
    var topOffset = 0.0
    var horizontalOffset = 0.0

    func selection(in ids: [UUID]) -> IndexSet {
        IndexSet(ids.indices.filter { selectedIDs.contains(ids[$0]) })
    }

    func anchor(in ids: [UUID]) -> Int? {
        guard !ids.isEmpty else { return nil }
        return topFrameID.flatMap { ids.firstIndex(of: $0) } ?? 0
    }
}
