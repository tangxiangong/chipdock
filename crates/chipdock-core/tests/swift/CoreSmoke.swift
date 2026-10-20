import Foundation

private struct CheckFailure: Error, CustomStringConvertible {
    let description: String
}

private func check(_ condition: @autoclosure () -> Bool, _ message: String) throws {
    guard condition() else { throw CheckFailure(description: message) }
}

private func expectCoreError(_ code: ErrorCode, _ operation: () throws -> Void) throws {
    do {
        try operation()
    } catch let error as CoreError {
        guard case let .Operation(response) = error else { throw error }
        try check(response.code == code, "Unexpected typed error: \(response)")
        try check(!response.message.isEmpty, "The user-facing error message was lost")
        try check(!response.recovery.isEmpty, "The recovery advice was lost")
        return
    }
    throw CheckFailure(description: "Expected a typed CoreError")
}

@main
private struct BackendInteropTests {
    static func main() throws {
        let core = ChipDockCore()
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        var passed = 0

        let chips = try core.searchChips(query: "STM32F103VE", limit: 10)
        try check(chips.contains { $0.hasPrefix("STM32F103VE") }, "Built-in chip search failed across UniFFI")
        let regions = try core.targetMemoryMap(chip: "STM32F103VE")
        try check(regions.contains { $0.kind == .nvm && $0.start == 0x0800_0000 }, "Memory-map addresses changed at the FFI boundary")
        passed += 1

        try expectCoreError(.invalidUserInput) { _ = try core.searchChips(query: "", limit: 10) }
        passed += 1

        let image = root.appendingPathComponent("固件.bin")
        try Data([0, 1, 2, 3, 0xFF]).write(to: image)
        let usage = try core.firmwareUsage(input: FirmwareInput(path: image.path, format: .bin, baseAddress: 0x0800_0000))
        try check(usage.usedBytes == 5 && usage.startAddress == 0x0800_0000 && usage.endAddress == 0x0800_0005,
                  "BIN size or address conversion failed")
        passed += 1

        let preview = try core.firmwarePreview(input: FirmwareInput(path: image.path, format: .bin, baseAddress: 0x0800_0000), length: 4096)
        try check(preview.startAddress == 0x0800_0000 && preview.bytes == [0, 1, 2, 3, 0xFF],
                  "Firmware preview bytes, optionals or addresses changed across UniFFI")
        passed += 1

        let sparseImage = root.appendingPathComponent("sparse.hex")
        try Data(":020000040800F2\n:020000000001FD\n:01000300FFFD\n:00000001FF\n".utf8).write(to: sparseImage)
        let sparsePreview = try core.firmwarePreview(input: FirmwareInput(path: sparseImage.path, format: .hex, baseAddress: nil), length: 4096)
        try check(sparsePreview.bytes == [0, 1, nil, 0xFF], "An unwritten firmware address became a real byte across UniFFI")
        passed += 1

        try expectCoreError(.invalidFirmwareAddress) {
            _ = try core.firmwareUsage(input: FirmwareInput(path: image.path, format: .bin, baseAddress: UInt64.max - 3))
        }
        passed += 1

        let empty = root.appendingPathComponent("empty.bin")
        try Data().write(to: empty)
        try expectCoreError(.invalidUserInput) {
            _ = try core.firmwareUsage(input: FirmwareInput(path: empty.path, format: .bin, baseAddress: 0))
        }
        passed += 1

        let profileDirectory = root.appendingPathComponent("profiles", isDirectory: true)
        try FileManager.default.createDirectory(at: profileDirectory, withIntermediateDirectories: true)
        let legacy = """
        [{"id":"550e8400-e29b-41d4-a716-446655440000","name":"旧版配置","probe":null,
        "target":{"chip":"STM32F103VE","protocol":"swd","speedKhz":4000,"connectUnderReset":false},
        "flashOptions":{"verify":true,"dryRun":false,"skipErase":false,"allowEraseAll":false,"resetAfter":true},
        "binBaseAddress":134217728,"updatedAt":"2026-07-06T08:00:00Z"}]
        """
        try Data(legacy.utf8).write(to: profileDirectory.appendingPathComponent("profiles.json"))
        let profiles = try core.loadProfiles(directory: profileDirectory.path)
        try check(profiles.count == 1 && profiles[0].name == "旧版配置" && profiles[0].target.chip == "STM32F103VE",
                  "Legacy UUID/RFC3339 profile compatibility regressed")
        try core.saveProfiles(directory: profileDirectory.path, profiles: profiles)
        let reloadedProfiles = try core.loadProfiles(directory: profileDirectory.path)
        try check(reloadedProfiles == profiles, "Profiles did not round-trip through generated Swift records")
        passed += 1

        let files = [RecentFirmware(path: image.path, format: .bin, baseAddress: 0x0800_0000,
                                    usedAt: Date(timeIntervalSince1970: -0.5))]
        try core.saveRecentFiles(directory: root.path, files: files)
        let reloaded = try core.loadRecentFiles(directory: root.path)
        try check(reloaded == files, "Pre-epoch dates or Unicode paths did not survive storage")
        passed += 1

        let future = RecentFirmware(path: image.path, format: .bin, baseAddress: 0,
                                    usedAt: Date(timeIntervalSince1970: 100_000_000_000_000))
        try expectCoreError(.invalidUserInput) { try core.saveRecentFiles(directory: root.path, files: [future]) }
        let afterInvalidSave = try core.loadRecentFiles(directory: root.path)
        try check(afterInvalidSave == files, "Rejecting an invalid foreign timestamp changed stored history")
        passed += 1

        let target = TargetSelection(chip: "STM32F103VE", protocol: .swd, speedKhz: 4000, connectUnderReset: false)
        // These calls are rejected before open_session; no USB work is reachable.
        try expectCoreError(.invalidUserInput) {
            _ = try core.readMemory(request: MemoryRequest(probe: nil, target: target, address: UInt64.max, length: 2))
        }
        try expectCoreError(.invalidUserInput) {
            _ = try core.readMemory(request: MemoryRequest(probe: nil, target: target, address: 0, length: 0))
        }
        passed += 1

        let request = FlashRequest(
            firmware: FirmwareInput(path: empty.path, format: .bin, baseAddress: 0),
            probe: nil, target: target,
            options: FlashOptions(verify: true, dryRun: true, skipErase: false, allowEraseAll: true, resetAfter: true)
        )
        try expectCoreError(.invalidUserInput) { _ = try core.flashFirmware(request: request) }
        let events = core.takeEvents()
        try check(events.last?.stage == .failed && events.last?.kind == .flash,
                  "The terminal error event was lost across UniFFI")
        try check(!events.contains { [.connecting, .programming, .resetting].contains($0.stage) },
                  "An invalid image reached a hardware stage")
        try check(core.takeEvents().isEmpty, "The event queue was not drained exactly once")
        passed += 1

        try check(!core.version().isEmpty, "Core version was empty")
        print("Swift/UniFFI backend integration: \(passed) checks passed (no UI, no hardware).")
    }
}
