import XCTest
@testable import ChipDock

final class CoreIntegrationTests: XCTestCase {
    func testGeneratedBindingsSearchAndMemoryMap() throws {
        let core = ChipDockCore()
        XCTAssertEqual(core.version(), "0.1.2")
        XCTAssertEqual(Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String, core.version())
        let chips = try core.searchChips(query: "STM32F103VE", limit: 10)
        XCTAssertTrue(chips.contains { $0.hasPrefix("STM32F103VE") })
        let regions = try core.targetMemoryMap(chip: "STM32F103VE")
        XCTAssertTrue(regions.contains { $0.kind == .nvm && $0.start == 0x0800_0000 })
        XCTAssertTrue(core.takeEvents().isEmpty)
    }

    func testUniFFIReturnsStructuredErrors() throws {
        let core = ChipDockCore()
        XCTAssertThrowsError(try core.searchChips(query: "", limit: 10)) { error in
            guard let error = error as? CoreError, case let .Operation(response) = error else {
                return XCTFail("Expected a typed UniFFI error, received \(error)")
            }
            XCTAssertEqual(response.code, .invalidUserInput)
            XCTAssertFalse(response.message.isEmpty)
            XCTAssertFalse(response.recovery.isEmpty)
        }
    }

    func testBINAnalysisCrossesSwiftRustBoundaryWithoutHardware() throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let file = directory.appendingPathComponent("firmware.bin")
        try Data([0, 1, 2, 0xFF]).write(to: file)
        let usage = try ChipDockCore().firmwareUsage(input: FirmwareInput(path: file.path, format: .bin, baseAddress: 0x0800_0000))
        XCTAssertEqual(usage.usedBytes, 4)
        XCTAssertEqual(usage.startAddress, 0x0800_0000)
        XCTAssertEqual(usage.endAddress, 0x0800_0004)
    }

    func testProfilesReadLegacyJSONAndRoundTripThroughUniFFI() throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let legacy = """
        [{"id":"550e8400-e29b-41d4-a716-446655440000","name":"Legacy","probe":null,
          "target":{"chip":"STM32F103VE","protocol":"swd","speedKhz":4000,"connectUnderReset":false},
          "flashOptions":{"verify":true,"dryRun":false,"skipErase":false,"allowEraseAll":false,"resetAfter":true},
          "binBaseAddress":134217728,"updatedAt":"2026-07-06T08:00:00Z"}]
        """
        try Data(legacy.utf8).write(to: directory.appendingPathComponent("profiles.json"))
        let core = ChipDockCore()
        let profiles = try core.loadProfiles(directory: directory.path)
        XCTAssertEqual(profiles.count, 1)
        XCTAssertEqual(profiles[0].target.chip, "STM32F103VE")
        XCTAssertEqual(profiles[0].target.protocol, .swd)
        try core.saveProfiles(directory: directory.path, profiles: profiles)
        XCTAssertEqual(try core.loadProfiles(directory: directory.path), profiles)
    }

    func testRecentFirmwarePreservesPathAndBaseAddress() throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let core = ChipDockCore()
        let files = [RecentFirmware(path: "/tmp/固件.bin", format: .bin, baseAddress: 0x0800_0000,
                                    usedAt: Date(timeIntervalSince1970: 1_782_000_000))]
        try core.saveRecentFiles(directory: directory.path, files: files)
        XCTAssertEqual(try core.loadRecentFiles(directory: directory.path), files)
    }

    func testAddressParsingPreserves64BitValuesAndRejectsOverflow() {
        XCTAssertEqual(parseAddress(" 0xFFFFFFFFFFFFFFFE "), UInt64.max - 1)
        XCTAssertEqual(parseAddress("134217728"), 0x0800_0000)
        XCTAssertNil(parseAddress("0x10000000000000000"))
        XCTAssertNil(parseAddress("-1"))
        XCTAssertNil(parseAddress("0x"))
        XCTAssertEqual(addressLabel(0x1_0000_0000), "0x100000000")
    }

    func testMemoryExportReconstructsExactBytesAndPartialLastLine() {
        let result = MemoryReadResult(address: 0x0800_0000, length: 17, dataHex: "000102030405060708090a0b0c0d0e0f41")
        XCTAssertEqual(result.data, Data(Array(0...15).map(UInt8.init) + [0x41]))
        XCTAssertEqual(result.lines.count, 2)
        XCTAssertEqual(result.lines[1].address, 0x0800_0010)
        XCTAssertEqual(result.lines[1].ascii, "A")
    }

    private func temporaryDirectory() throws -> URL {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }
}
