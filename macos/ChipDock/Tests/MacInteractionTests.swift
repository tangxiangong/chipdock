import Foundation

@main
@MainActor
private struct MacInteractionTests {
    static func check(_ condition: @autoclosure () -> Bool, _ message: String) throws {
        if !condition() { throw Failure(description: message) }
    }

    struct Failure: Error, CustomStringConvertible { let description: String }

    static func main() {
        do {
            try exportingKeepsPanelsOpen()
            try discardedConfigurationKeepsSession()
            try terminalRestoresFramesByIdentity()
            try logSelectionSurvivesPruning()
            print("macOS interaction logic: 4 checks passed")
        } catch { print("FAIL: \(error)"); exit(1) }
    }

    static func exportingKeepsPanelsOpen() throws {
        let store = WorkbenchStore()
        store.showMemory = true
        store.showLogs = true
        store.requestExport(PendingExport(data: Data([0xAA]), contentType: .data,
                                          filename: "memory.bin", operation: "导出内存"))
        try check(store.showMemory && store.showLogs, "Export closed the persistent auxiliary views")
        try check(store.pendingExport?.data == Data([0xAA]), "Export did not retain its payload")
        print("PASS: exporting preserves auxiliary views")
    }

    static func discardedConfigurationKeepsSession() throws {
        let store = WorkbenchStore()
        store.chip = "STM32F103VE"
        store.memoryResult = MemoryReadResult(address: 0x0800_0000, length: 1, dataHex: "AA")
        var draft = store.targetConfiguration
        draft.chip = "STM32F407VG"
        draft.speed = "1000"
        draft.wireProtocol = .jtag
        try check(store.chip == "STM32F103VE" && store.speed == "4000" && store.memoryResult != nil,
                  "Editing a draft invalidated the original session")
        store.applyTargetConfiguration(draft)
        try check(store.chip == "STM32F407VG" && store.speed == "1000" && store.wireProtocol == .jtag,
                  "Confirming did not apply the chosen configuration")
        try check(store.memoryResult == nil, "Committing a changed target retained stale memory")
        for text in ["", "0", "-1", "4294967296", "1.5", "+1000"] {
            draft.speed = text
            try check(draft.validSpeed == nil, "Invalid connection speed accepted: \(text)")
        }
        draft.speed = " 24000 "
        try check(draft.validSpeed == 24000, "A valid connection speed was rejected")
        print("PASS: target drafts isolate edits and validate before commit")
    }

    static func terminalRestoresFramesByIdentity() throws {
        let state = TerminalTableState()
        let first = UUID(), second = UUID(), third = UUID(), fourth = UUID()
        state.selectedIDs = [second, third]
        state.topFrameID = third
        state.topOffset = 7
        state.columnWidths["data"] = 432
        try check(state.selection(in: [second, third, fourth]) == IndexSet([0, 1]),
                  "Pruning older frames moved the selection to different records")
        try check(state.anchor(in: [second, third, fourth]) == 1 && state.topOffset == 7,
                  "Pruning older frames lost the reading anchor")
        try check(state.selection(in: [first, fourth]).isEmpty && state.anchor(in: [first, fourth]) == 0,
                  "Expired records left a stale selection or an invalid viewport")
        try check(state.anchor(in: []) == nil && state.columnWidths["data"] == 432,
                  "Emptying the terminal discarded column preferences")
        print("PASS: terminal state survives frame pruning and view recreation")
    }

    static func logSelectionSurvivesPruning() throws {
        let first = UUID(), second = UUID(), third = UUID(), fourth = UUID()
        let previous = [(first, NSRange(location: 0, length: 5)), (second, NSRange(location: 5, length: 6)),
                        (third, NSRange(location: 11, length: 4))]
        let current = [(second, NSRange(location: 0, length: 6)), (third, NSRange(location: 6, length: 4)),
                       (fourth, NSRange(location: 10, length: 8))]
        let restored = LogTextSelection.restore([NSRange(location: 7, length: 3), NSRange(location: 6, length: 7)],
                                                from: previous, to: current)
        try check(restored == [NSRange(location: 2, length: 3), NSRange(location: 1, length: 7)],
                  "Pruning the prefix changed the text selected for copying")
        try check(LogTextSelection.restore([NSRange(location: 1, length: 2)], from: previous, to: current).isEmpty,
                  "An expired selection moved onto unrelated text")
        try check(LogTextSelection.restore([NSRange(location: 7, length: 3)], from: previous, to: []).isEmpty,
                  "Clearing the log retained an out-of-bounds selection")
        print("PASS: log selections remain attached to their entries after pruning")
    }
}
