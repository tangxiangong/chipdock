import Foundation

private struct SerialTestFailure: Error, CustomStringConvertible {
    let description: String
}

@main
@MainActor
private struct SerialSendTests {
    static func check(_ condition: @autoclosure () -> Bool, _ message: String) throws {
        guard condition() else { throw SerialTestFailure(description: message) }
    }

    static func waitUntil(_ message: String, _ condition: () -> Bool) async throws {
        let deadline = ContinuousClock.now.advanced(by: .seconds(1))
        while !condition() {
            try check(ContinuousClock.now < deadline, message)
            try await Task.sleep(for: .milliseconds(2))
        }
    }

    static func main() async {
        let tests: [(String, @MainActor () async throws -> Void)] = [
            ("protocol JSON shares defaults and round trips", protocolRules),
            ("frame fragments preserve payload and checksum order", protocolPresets),
            ("legacy preferences retain settings", legacyPreferences),
            ("custom baud rates reject invalid input", customBaudRates),
            ("invalid intervals never start", invalidIntervals),
            ("repeated sends remain serialized and stop on failure", serializedSends),
            ("stop cancels the next send", stopDuringSend),
            ("an old send cannot stop a new session", restartDuringSend),
        ]
        var failures = 0
        for (name, test) in tests {
            do { try await test(); print("PASS: \(name)") }
            catch { failures += 1; print("FAIL: \(name): \(error)") }
        }
        if failures > 0 { exit(1) }
        print("Serial send logic: \(tests.count) checks passed")
    }

    static func protocolPresets() async throws {
        let source = "AA BB {value:u16be} {checksum:sum8}"
        let added = SerialFrameFragment.u8.inserting(into: source)
        try check(added == "AA BB {value:u16be} {value2:u8} {checksum:sum8}", "Parameter must preserve bytes and precede checksum with a unique name")
        let replaced = SerialFrameFragment.crc16.inserting(into: added)
        try check(replaced == "AA BB {value:u16be} {value2:u8} {checksum:crc16modbus}", "Selecting checksum must replace only the checksum")
        try check(SerialFrameFragment.header.inserting(into: "{value:u8}") == "AA BB {value:u8}", "Header must precede payload")
        try check(SerialFrameFragment.u16le.inserting(into: "") == "{value:u16le}", "Empty frame must accept a fragment")
        try check(SerialFrameFragment(rawValue: "unrelated") == nil, "Unknown fragment must be rejected")
    }

    static func protocolRules() async throws {
        let json = #"[{"name":"motor-speed","template":"AA BB {speed:u16be:0:3000} {checksum:sum8}","response":"report"},{"name":"report","receive":true,"template":"CC DD {speed:u16be} {checksum:sum8}","display":"速度：{speed} rpm"}]"#
        let rules = try JSONDecoder().decode([SerialProtocolRule].self, from: Data(json.utf8))
        try check(rules[0].timeout_ms == 1000 && rules[0].checksum_end == nil && !rules[0].receive, "Optional protocol defaults differ from Rust")
        let restored = try JSONDecoder().decode([SerialProtocolRule].self, from: JSONEncoder().encode(rules))
        try check(restored == rules, "Protocol import/export changed templates or linkage")
        let response = #"{"hex":"","buffer":"CC","records":[{"name":"report","hex":"CC DD 00 2D D6","message":"速度：45 rpm","valid":true}],"response":"","timeout_ms":0}"#
        let decoded = try JSONDecoder().decode(SerialProtocolResult.self, from: Data(response.utf8))
        try check(decoded.buffer == "CC" && decoded.records[0].valid, "Decode response lost buffer or validity")
    }

    static func legacyPreferences() async throws {
        let data = Data(#"{"config":{"baudRate":250000,"dataBits":8,"stopBits":1,"parity":"even","flowControl":"none"},"selectedPath":"/dev/cu.custom","displayFormat":"hex","sendFormat":"gbk","lineEnding":"crlf","autoScroll":false}"#.utf8)
        var preferences = try JSONDecoder().decode(SerialPreferences.self, from: data)
        try check(preferences.config.baudRate == 250000 && preferences.config.parity == "even", "Connection settings reset")
        try check(preferences.selectedPath == "/dev/cu.custom" && preferences.sendFormat == .gbk && !preferences.autoScroll, "Saved serial preferences reset")
        try check(preferences.checksum == .none && preferences.autoSendIntervalMilliseconds == 1000, "New options need compatible defaults")
        preferences.checksum = .crc16Modbus
        preferences.autoSendIntervalMilliseconds = 250
        let restored = try JSONDecoder().decode(SerialPreferences.self, from: JSONEncoder().encode(preferences))
        try check(restored.checksum == .crc16Modbus && restored.autoSendIntervalMilliseconds == 250, "Send options did not round-trip")
    }

    static func customBaudRates() async throws {
        for (input, expected) in [("250000", 250000), (" 115200 ", 115200), ("4294967295", 4294967295)] {
            try check(SerialBaudRate.parse(input) == expected, "Valid custom baud rate rejected: \(input)")
        }
        for input in ["", " ", "0", "-9600", "4294967296", "115.2", "115200abc", "+9600"] {
            try check(SerialBaudRate.parse(input) == nil, "Invalid custom baud rate accepted: \(input)")
        }
    }

    static func invalidIntervals() async throws {
        let sender = SerialAutoSender()
        for interval in [-1, 0, 9, 3_600_001, Int.max] {
            let started = sender.start(intervalMilliseconds: interval) { true }
            defer { sender.stop() }
            try check(!started && !sender.isRunning, "Invalid interval started automatic sending: \(interval)")
        }
    }

    static func serializedSends() async throws {
        let sender = SerialAutoSender()
        defer { sender.stop() }
        var active = 0
        var maximumActive = 0
        var count = 0
        sender.start(intervalMilliseconds: 10) {
            active += 1
            maximumActive = max(maximumActive, active)
            try? await Task.sleep(for: .milliseconds(15))
            count += 1
            active -= 1
            return count < 3
        }
        try await waitUntil("The first send never started") { active == 1 }
        try await waitUntil("The send loop did not stop after failure") { !sender.isRunning }
        try check(count == 3 && maximumActive == 1, "Automatic sends overlapped or continued after failure")
    }

    static func stopDuringSend() async throws {
        let sender = SerialAutoSender()
        defer { sender.stop() }
        var release: CheckedContinuation<Void, Never>?
        var count = 0
        sender.start(intervalMilliseconds: 10) {
            count += 1
            await withCheckedContinuation { release = $0 }
            return true
        }
        try await waitUntil("The in-flight send never started") { release != nil }
        sender.stop()
        release?.resume()
        try await Task.sleep(for: .milliseconds(40))
        try check(!sender.isRunning && count == 1, "Stopping queued another send")
    }

    static func restartDuringSend() async throws {
        let sender = SerialAutoSender()
        defer { sender.stop() }
        var releaseOld: CheckedContinuation<Void, Never>?
        var releaseNew: CheckedContinuation<Void, Never>?
        var oldFinished = false
        sender.start(intervalMilliseconds: 10) {
            await withCheckedContinuation { releaseOld = $0 }
            oldFinished = true
            return false
        }
        try await waitUntil("The old send never started") { releaseOld != nil }
        sender.stop()
        sender.start(intervalMilliseconds: 10) {
            await withCheckedContinuation { releaseNew = $0 }
            return false
        }
        try await waitUntil("The new send never started") { releaseNew != nil }
        releaseOld?.resume()
        try await waitUntil("The old send did not finish") { oldFinished }
        try check(sender.isRunning, "An old task cleared the new task's running state")
        releaseNew?.resume()
        try await waitUntil("The replacement send did not stop") { !sender.isRunning }
    }
}
