import Foundation
import Observation

@MainActor
@Observable
final class SerialStore: Identifiable {
    let id: UInt64
    @ObservationIgnored let terminalTableState = TerminalTableState()
    var preferences: SerialPreferences { didSet { if id == 1 { preferences.save() } } }
    private(set) var ports: [SerialPort] = []
    private(set) var status = SerialConnectionStatus()
    private(set) var frames: [SerialFrame] = []
    private(set) var pausedFrames: [SerialFrame]?
    private(set) var pausedParsed: [SerialProtocolRecord]?
    private(set) var clearedThrough: UInt64 = 0
    private var cursor: UInt64 = 0
    var terminalJump: UInt64?
    var searchCase = false
    var searchRegex = false
    private(set) var busy = false
    private(set) var sending = false
    private(set) var refreshing = false
    var error: String?
    var input = "" {
        didSet {
            if !commandMode { preferences.draft = String(input.prefix(262_144)) }
            else { commandDraft = input }
        }
    }
    var commandMode = false {
        didSet { if commandMode != oldValue { input = commandMode ? commandDraft : preferences.draft } }
    }
    private var commandDraft = ""
    var protocolRules: [SerialProtocolRule] = {
        guard let data = UserDefaults.standard.data(forKey: "serial.protocol.rules"),
              let rules = try? JSONDecoder().decode([SerialProtocolRule].self, from: data) else { return [] }
        return rules
    }()
    var protocolRecords: [SerialProtocolRecord] = []
    var protocolDisplay = "both"
    var commandPreview = ""
    var commandError = ""
    var commandHistory: [String] = []
    var feedback: String { status.feedback ?? "" }
    private var protocolRevision = 0
    private var historyIndex = 0

    func previewCommand() async {
        guard commandMode else { commandPreview = ""; commandError = ""; return }
        let value = input
        let revision = protocolRevision
        commandPreview = ""
        do {
            let result = try await core.program(rules: protocolRules, operation: "encode", input: value)
            guard value == input && revision == protocolRevision else { return }
            commandPreview = result.hex; commandError = ""
        } catch {
            guard value == input && revision == protocolRevision else { return }
            commandError = error.localizedDescription
        }
    }
    func saveRules(_ rules: [SerialProtocolRule]) async throws {
        let data = try JSONEncoder().encode(rules)
        try await applyRules(rules)
        if id == 1 { UserDefaults.standard.set(data, forKey: "serial.protocol.rules") }
        protocolRules = rules; protocolRevision += 1
        await previewCommand()
    }
    func testRules(_ rules: [SerialProtocolRule], input: String, rule: SerialProtocolRule) async throws -> SerialProtocolResult {
        try await core.program(rules: rules, operation: "inspect", input: rule.receive ? "" : input, incoming: rule.receive ? input : "", rule: rule.name)
    }
    func recallCommand(_ delta: Int) {
        guard !commandHistory.isEmpty else { return }
        historyIndex = min(commandHistory.count, max(0, historyIndex + delta))
        input = historyIndex == commandHistory.count ? "" : commandHistory[historyIndex]
    }
    func workbench(_ action: [String: Any]) async throws -> String {
        let data = try JSONSerialization.data(withJSONObject: action)
        return try await core.workbench(String(decoding: data, as: UTF8.self))
    }
    private func applyRules(_ rules: [SerialProtocolRule]) async throws {
        let value = try JSONSerialization.jsonObject(with: JSONEncoder().encode(rules))
        _ = try await workbench(["action": "protocol", "rules": value])
    }
    func applyFraming(mode: String, value: String) async throws {
        var config: [String: Any] = ["mode": mode]
        switch mode {
        case "idle", "fixed":
            guard let number = Int(value), number > 0 else { throw SerialClientError.operation("请输入正整数") }
            config[mode == "idle" ? "ms" : "bytes"] = number
        case "header", "tail": config["hex"] = value
        default: break
        }
        _ = try await workbench(["action": "framing", "config": config])
        preferences.framingMode = mode
        preferences.framingValue = value
    }

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
    @ObservationIgnored private let core: SerialBackend
    private var startingRepeat = false
    @ObservationIgnored private var pollingTask: Task<Void, Never>?
    private var connectionRevision = 0
    private var retainedBytes = 0

    init(id: UInt64 = 1, copying source: SerialStore? = nil) {
        self.id = id
        core = SerialBackend(id: id)
        var config = source?.preferences ?? .load()
        if source != nil { config.autoConnect = false; config.draft = ""; config.history = [] }
        preferences = config
        if let source { protocolRules = source.protocolRules }

        baudRateInput = String(preferences.config.baudRate)
        autoSendIntervalInput = String(preferences.autoSendIntervalMilliseconds)
        input = preferences.draft
    }

    var updateBlocked: Bool { status.connected || status.reconnecting || busy || sending || autoSending }

    var locked: Bool { status.connected || status.reconnecting || busy }
    var canSend: Bool { canTransmit && !autoSending }
    private var canTransmit: Bool { status.connected && status.jobKind == nil && !sending && !input.isEmpty && !busy && (!commandMode || (!commandPreview.isEmpty && commandError.isEmpty)) }
    var autoSending: Bool { startingRepeat || status.jobKind == "repeat" }
    var validBaudRate: Bool { SerialBaudRate.parse(baudRateInput) != nil }
    var canConnect: Bool { !busy && !preferences.selectedPath.isEmpty && validBaudRate }
    var autoSendInterval: Int? {
        guard let value = Int(autoSendIntervalInput), (10...3_600_000).contains(value) else { return nil }
        return value
    }
    var canStartAutoSend: Bool { !commandMode && canSend && autoSendInterval != nil }
    var selectedPort: SerialPort? { ports.first { $0.path == preferences.selectedPath } }

    func start() async {
        guard pollingTask == nil else { return }
        pollingTask = Task { [weak self] in await self?.pollLoop() }
    }
    private func pollLoop() async {
        defer { pollingTask = nil }
        do {
            do {
                try await applyRules(protocolRules)
                try await applyFraming(mode: preferences.framingMode, value: preferences.framingValue)
                let rules = try JSONSerialization.jsonObject(with: Data(preferences.replies.utf8))
                _ = try await workbench(["action": "replies", "rules": rules])
            } catch { self.error = error.localizedDescription }
            let startupPath = preferences.selectedPath
            await refreshPorts()
            if id == 1 && preferences.autoConnect && selectedPort?.path == startupPath { await toggleConnection() }
            var tick = 0
            while !Task.isCancelled {
                let revision = connectionRevision
                let changingConnection = busy
                let batch = try await core.poll(after: cursor)
                cursor = batch.nextCursor
                if !changingConnection && !busy && revision == connectionRevision && status != batch.status {
                    if let failure = batch.status.lastError, failure != status.lastError { error = failure }
                    if let failure = batch.status.captureError, failure != status.captureError { error = failure }
                    if let failure = batch.status.jobError, failure != status.jobError, failure != "发送任务已取消" { error = failure }
                    status = batch.status
                    if status.connected {
                        if let path = status.path, path != preferences.selectedPath { preferences.selectedPath = path }
                        if let config = status.config, config != preferences.config {
                            preferences.config = config
                            baudRateInput = String(config.baudRate)
                        }
                    }
                }
                if !batch.frames.isEmpty {
                    append(batch.frames)
                    protocolRecords.append(contentsOf: batch.frames.flatMap(\.parsed))
                    if protocolRecords.count > 2000 { protocolRecords.removeFirst(protocolRecords.count - 2000) }
                }
                if batch.droppedFrames > 0 { error = "接收速度超过显示缓冲，已丢弃 \(batch.droppedFrames) 条旧记录；字节统计保持完整。" }
                tick += 1
                if tick % 10 == 0 { await refreshPorts(silent: true) }
                try await Task.sleep(for: .milliseconds(100))
            }
        } catch is CancellationError {
        } catch { self.error = error.localizedDescription }
    }

    func refreshPorts(silent: Bool = false) async {
        guard !refreshing else { return }
        refreshing = true
        defer { refreshing = false }
        do {
            let found = try await core.ports()
            if found != ports { ports = found }
            if !locked && (preferences.selectedPath.isEmpty || !preferences.autoConnect) && !found.contains(where: { $0.path == preferences.selectedPath }) {
                let selected = found.first?.path ?? ""
                if preferences.selectedPath != selected { preferences.selectedPath = selected }
            }
        } catch { if !silent { self.error = error.localizedDescription } }
    }

    func toggleConnection() async {
        guard !busy else { return }
        if !status.connected && !status.reconnecting && !validBaudRate {
            error = "波特率必须是 1–4294967295 的整数。"
            return
        }
        busy = true
        connectionRevision += 1
        let revision = connectionRevision
        defer { if revision == connectionRevision { busy = false } }
        do {
            error = nil
            let result: SerialConnectionStatus = status.connected || status.reconnecting
                ? try await core.disconnect()
                : try await core.connect(path: preferences.selectedPath, config: preferences.config, reconnect: preferences.autoReconnect)
            if revision == connectionRevision { status = result }
        } catch { self.error = error.localizedDescription }
    }

    func send() async {
        guard canSend else { return }
        _ = await transmit()
    }

    func toggleAutoSend() {
        guard !startingRepeat else { return }
        if status.jobKind == "repeat" { Task { await cancelJob() }; return }
        guard canStartAutoSend, let interval = autoSendInterval else { return }
        startingRepeat = true
        Task {
            defer { startingRepeat = false }
            do {
                let options: [String: Any] = ["hexLineEnding": preferences.hexLineEnding,
                    "skipPrefix": preferences.skipPrefix, "skipSuffix": preferences.skipSuffix]
                let message: [String: Any] = ["text": input, "format": preferences.sendFormat.rawValue,
                    "ending": preferences.lineEnding, "checksum": preferences.checksum.rawValue, "options": options]
                _ = try await workbench(["action": "startJob", "job": ["kind": "repeat", "message": message,
                                                                      "intervalMs": interval] as [String: Any]])
                status = try await core.poll(after: cursor).status
            } catch { self.error = error.localizedDescription }
        }
    }

    func cancelJob() async {
        do { _ = try await workbench(["action": "cancelJob"]) }
        catch { self.error = error.localizedDescription }
    }

    private func transmit() async -> Bool {
        guard canTransmit else { return false }
        let revision = connectionRevision
        sending = true
        defer { sending = false }
        do {
            if commandMode {
                let command = input
                let result = try await workbench(["action": "command", "input": command])
                guard revision == connectionRevision else { return false }
                let hex = (try? JSONSerialization.jsonObject(with: Data(result.utf8)) as? [String: String])?["hex"] ?? commandPreview
                commandHistory.append(command)
                if commandHistory.count > 100 { commandHistory.removeFirst() }
                historyIndex = commandHistory.count
                protocolRecords.append(SerialProtocolRecord(name: "TX", hex: hex, message: command, valid: true))
                if protocolRecords.count > 2000 { protocolRecords.removeFirst(protocolRecords.count - 2000) }
            } else {
                let sent = input
                try await core.send(input: sent, format: preferences.sendFormat, lineEnding: preferences.lineEnding,
                    checksum: preferences.checksum, options: SendOptions(hexLineEnding: preferences.hexLineEnding,
                        skipPrefix: preferences.skipPrefix, skipSuffix: preferences.skipSuffix))
                var history = preferences.history.filter { $0 != sent }
                history.append(String(sent.prefix(262_144)))
                while history.count > 50 || history.reduce(0, { $0 + $1.utf8.count }) > 1024 * 1024 { history.removeFirst() }
                preferences.history = history
            }
            return status.connected && !busy && revision == connectionRevision
        } catch {
            if revision == connectionRevision { self.error = error.localizedDescription }
            return false
        }
    }

    func clear() {
        clearedThrough = cursor
        protocolRecords.removeAll()
        frames.removeAll(keepingCapacity: true)
        if pausedFrames != nil { pausedFrames = []; pausedParsed = [] }
        retainedBytes = 0
        if preferences.resetOnClear { Task { await resetCounters() } }
    }

    func resetCounters() async {
        do { _ = try await workbench(["action": "resetCounters"]) }
        catch { self.error = error.localizedDescription }
    }

    func togglePause() {
        if pausedFrames == nil { pausedFrames = frames; pausedParsed = protocolRecords }
        else { pausedFrames = nil; pausedParsed = nil }
    }

    func jumpToRecord(_ sequence: UInt64) {
        pausedFrames = nil; pausedParsed = nil
        searchText = ""; direction = .all
        preferences.autoScroll = false
        if protocolDisplay == "parsed" { protocolDisplay = "both" }
        terminalJump = sequence
    }

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

    func setLine(rts: Bool, enabled: Bool) async {
        guard status.connected && !busy else { return }
        busy = true
        defer { busy = false }
        do { status = try await core.setLine(rts: rts, enabled: enabled) }
        catch { self.error = error.localizedDescription }
    }

    func shutdown() async {
        pollingTask?.cancel()
        busy = true
        connectionRevision += 1
        do { try await core.close() }
        catch { self.error = error.localizedDescription }
    }
}

@MainActor
@Observable
final class SerialWorkbench {
    private(set) var sessions = [SerialStore()]
    var selected: UInt64 = 1
    private(set) var changing = false
    private(set) var settingControl = false
    private(set) var localControl = UInt8(clamping: UserDefaults.standard.integer(forKey: "serial.localControl"))
    @ObservationIgnored private var plotTask: Task<Void, Never>?
    @ObservationIgnored private var showPlot: ((UInt64) -> Void)?
    var active: SerialStore { sessions.first { $0.id == selected } ?? sessions[0] }
    var updateBlocked: Bool { changing || sessions.contains { $0.updateBlocked } || ((try? serialIsBusy()) ?? true) }

    func start(openPlot: @escaping @MainActor (UInt64) -> Void) async {
        await sessions[0].start()
        showPlot = openPlot
        guard plotTask == nil else { return }
        await setLocalControl(localControl)
        plotTask = Task { [weak self] in
            while !Task.isCancelled {
                guard let self else { return }
                if let request = serialIpcTakePlot() {
                    if self.sessions.contains(where: { $0.id == request.sessionId }) {
                        self.showPlot?(request.sessionId)
                        serialIpcFinishPlot(id: request.id, failure: nil)
                    } else {
                        serialIpcFinishPlot(id: request.id, failure: "串口会话不存在")
                    }
                }
                do { try await Task.sleep(for: .milliseconds(150)) } catch { return }
            }
        }
    }

    func setLocalControl(_ mode: UInt8) async {
        guard !settingControl else { return }
        settingControl = true
        defer { settingControl = false }
        do {
            try await SerialBackend().setLocalControl(mode)
            localControl = mode
            UserDefaults.standard.set(Int(mode), forKey: "serial.localControl")
        } catch { active.error = error.localizedDescription }
    }

    func add() async {
        guard !changing, sessions.count < 16 else { return }
        let source = active
        changing = true
        defer { changing = false }
        do {
            let id = try await SerialBackend().create()
            let session = SerialStore(id: id, copying: source)
            sessions.append(session)
            selected = id
            await session.start()
        } catch { source.error = error.localizedDescription }
    }

    func close(_ session: SerialStore) async {
        guard !changing, session.id != 1 else { return }
        changing = true
        defer { changing = false }
        await session.shutdown()
        sessions.removeAll { $0.id == session.id }
        if selected == session.id { selected = sessions.last?.id ?? 1 }
    }
}
