import Foundation
import Observation

@MainActor
@Observable
final class SerialStore {
    @ObservationIgnored let terminalTableState = TerminalTableState()
    var preferences = SerialPreferences.load() { didSet { preferences.save() } }
    private(set) var ports: [SerialPort] = []
    private(set) var status = SerialConnectionStatus()
    private(set) var frames: [SerialFrame] = []
    private(set) var busy = false
    private(set) var sending = false
    private(set) var refreshing = false
    var error: String?
    var input = ""
    var searchText = ""
    var direction = SerialDirectionFilter.all
    var baudRateInput = "" {
        didSet {
            if let baudRate = SerialBaudRate.parse(baudRateInput) { preferences.config.baudRate = baudRate }
        }
    }
    var autoSendIntervalInput = "" {
        didSet {
            if let interval = autoSendInterval { preferences.autoSendIntervalMilliseconds = interval }
        }
    }
    @ObservationIgnored private let core = SerialBackend()
    private let autoSender = SerialAutoSender()
    @ObservationIgnored private var pollingTask: Task<Void, Never>?
    private var connectionRevision = 0
    private var retainedBytes = 0

    init() {
        baudRateInput = String(preferences.config.baudRate)
        autoSendIntervalInput = String(preferences.autoSendIntervalMilliseconds)
    }

    var updateBlocked: Bool { status.connected || busy || sending || autoSending }

    var locked: Bool { status.connected || busy }
    var canSend: Bool { canTransmit && !autoSending }
    private var canTransmit: Bool { status.connected && !sending && !input.isEmpty && !busy }
    var autoSending: Bool { autoSender.isRunning }
    var validBaudRate: Bool { SerialBaudRate.parse(baudRateInput) != nil }
    var canConnect: Bool { !busy && !preferences.selectedPath.isEmpty && validBaudRate }
    var autoSendInterval: Int? {
        guard let value = Int(autoSendIntervalInput), SerialAutoSender.intervalRange.contains(value) else { return nil }
        return value
    }
    var canStartAutoSend: Bool { canSend && autoSendInterval != nil }
    var selectedPort: SerialPort? { ports.first { $0.path == preferences.selectedPath } }

    func start() async {
        guard pollingTask == nil else { return }
        pollingTask = Task { await pollLoop() }
    }
    private func pollLoop() async {
        defer { pollingTask = nil }
        do {
            await refreshPorts()
            var tick = 0
            while !Task.isCancelled {
                let revision = connectionRevision
                let changingConnection = busy
                let batch = try await core.poll()
                if !changingConnection && !busy && revision == connectionRevision && status != batch.status {
                    if let failure = batch.status.lastError, failure != status.lastError { error = failure }
                    status = batch.status
                    if !status.connected { autoSender.stop() }
                }
                if !batch.frames.isEmpty { append(batch.frames) }
                if batch.droppedFrames > 0 { error = "接收速度超过显示缓冲，已丢弃 \(batch.droppedFrames) 条旧记录；字节统计保持完整。" }
                tick += 1
                if tick % 10 == 0 { await refreshPorts(silent: true) }
                try await Task.sleep(for: .milliseconds(100))
            }
        } catch is CancellationError {
            autoSender.stop()
            _ = try? await core.disconnect()
        } catch { autoSender.stop(); self.error = error.localizedDescription }
    }

    func refreshPorts(silent: Bool = false) async {
        guard !refreshing else { return }
        refreshing = true
        defer { refreshing = false }
        do {
            let found = try await core.ports()
            if found != ports { ports = found }
            if !locked && !found.contains(where: { $0.path == preferences.selectedPath }) {
                let selected = found.first?.path ?? ""
                if preferences.selectedPath != selected { preferences.selectedPath = selected }
            }
        } catch { if !silent { self.error = error.localizedDescription } }
    }

    func toggleConnection() async {
        guard !busy else { return }
        if !status.connected && !validBaudRate {
            error = "波特率必须是 1–4294967295 的整数。"
            return
        }
        autoSender.stop()
        busy = true
        connectionRevision += 1
        let revision = connectionRevision
        defer { if revision == connectionRevision { busy = false } }
        do {
            error = nil
            let result: SerialConnectionStatus = status.connected
                ? try await core.disconnect()
                : try await core.connect(path: preferences.selectedPath, config: preferences.config)
            if revision == connectionRevision { status = result }
        } catch { self.error = error.localizedDescription }
    }

    func send() async {
        guard canSend else { return }
        _ = await transmit()
    }

    func toggleAutoSend() {
        if autoSending { autoSender.stop(); return }
        guard canSend else { return }
        guard let interval = autoSendInterval else {
            error = "自动发送间隔必须是 10–3600000 毫秒。"
            return
        }
        autoSender.start(intervalMilliseconds: interval) { [weak self] in
            guard let self else { return false }
            return await self.transmit()
        }
    }

    private func transmit() async -> Bool {
        guard canTransmit else { return false }
        let revision = connectionRevision
        sending = true
        defer { sending = false }
        do {
            try await core.send(input: input, format: preferences.sendFormat, lineEnding: preferences.lineEnding, checksum: preferences.checksum)
            return status.connected && !busy && revision == connectionRevision
        } catch {
            if revision == connectionRevision { self.error = error.localizedDescription }
            return false
        }
    }

    func clear() { frames.removeAll(keepingCapacity: true); retainedBytes = 0 }

    private func append(_ incoming: [SerialFrame]) {
        frames.append(contentsOf: incoming)
        retainedBytes += incoming.reduce(0) { $0 + $1.memoryCost }
        var removeCount = 0
        while frames.count - removeCount > 2000 || retainedBytes > 8 * 1024 * 1024 {
            retainedBytes -= frames[removeCount].memoryCost
            removeCount += 1
        }
        if removeCount > 0 { frames.removeFirst(removeCount) }
    }

    func shutdown() async {
        autoSender.stop()
        pollingTask?.cancel()
        busy = true
        connectionRevision += 1
        if let result = try? await core.disconnect() { status = result }
    }
}
