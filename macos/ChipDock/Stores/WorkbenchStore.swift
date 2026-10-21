import Foundation
import Observation

enum WorkbenchOperation: String {
    case scan = "扫描探针", connect = "连接目标", flash = "固件烧录", erase = "擦除芯片", read = "读取内存"
}

@MainActor
@Observable
final class WorkbenchStore {
    @ObservationIgnored let rust = RustService()
    @ObservationIgnored var didStart = false
    @ObservationIgnored var analysisTask: Task<Void, Never>?
    @ObservationIgnored var analysisRevision = UUID()
    @ObservationIgnored var recentFilesUpdateTask: Task<Void, Error>?
    @ObservationIgnored var recentFilesLoadTask: Task<Void, Error>?
    @ObservationIgnored var recentFilesLoaded = false

    var showMemory = false
    var showLogs = false { didSet { if showLogs { unseenErrors = 0 } } }
    var unseenErrors = 0
    var showTargetSetup = false
    var showImporter = false
    var showChipPicker = false
    var showSaveProfile = false
    var confirmErase = false
    var pendingExport: PendingExport?
    var failure: UserFacingError?
    var status = "准备就绪"
    var operation: WorkbenchOperation?
    var logs: [ActivityEntry] = []
    var latestEvent: ProgressEvent?

    var probes: [ProbeSummary] = []
    var hasScanned = false
    var selectedProbe = "" { didSet { if oldValue != selectedProbe { invalidateConnection() } } }
    var chip = "" { didSet { if oldValue != chip { invalidateConnection(); refreshMemoryMap() } } }
    var wireProtocol: WireProtocol = .swd { didSet { if oldValue != wireProtocol { invalidateConnection() } } }
    var speed = "4000" { didSet { if oldValue != speed { invalidateConnection() } } }
    var connectUnderReset = false { didSet { if oldValue != connectUnderReset { invalidateConnection() } } }
    var connection: ConnectionInfo?
    var targetInformation: TargetInformation?
    var targetCandidates: [TargetCandidate] = []
    var memoryRegions: [MemoryRegionLayout] = []

    var firmwareURL: URL?
    var firmwareUsage: FirmwareUsage?
    var firmwarePreview: FirmwarePreview?
    var analyzingFirmware = false
    var firmwareError: String?
    var binBaseAddress = "0x08000000"
    var verify = UserDefaults.standard.object(forKey: "defaultVerify") as? Bool ?? true
    var resetAfter = UserDefaults.standard.object(forKey: "defaultReset") as? Bool ?? true
    var dryRun = false
    var eraseStrategy: EraseStrategy = .automatic
    var profiles: [SavedProfile] = []
    var recentFiles: [RecentFirmware] = [] {
        didSet { refreshRecentFileAvailability() }
    }
    var missingRecentPaths: Set<String> = []

    var memoryAddress = "0x08000000"
    var memoryLength: UInt32 = 256
    var memoryResult: MemoryReadResult?
    var memoryLines: [MemoryLine] = []

    var isBusy: Bool { operation != nil }
    var isBin: Bool { firmwareURL?.pathExtension.lowercased() == "bin" }
    var effectiveChip: String { connection?.chip ?? chip }
    var targetName: String { targetInformation?.deviceType ?? connection?.chip ?? (chip.isEmpty ? "自动识别" : chip) }
    var validSpeed: UInt32? { UInt32(speed).flatMap { $0 > 0 ? $0 : nil } }
    var canConnect: Bool { !isBusy && validSpeed != nil && (!hasScanned || !probes.isEmpty) }
    var canFlash: Bool {
        !isBusy && connection != nil && firmwareUsage != nil && firmwareError == nil && !analyzingFirmware
            && (!isBin || parseAddress(binBaseAddress) != nil)
    }
    var canDisconnect: Bool { !isBusy && connection != nil }
    var canErase: Bool { !isBusy && connection != nil }
    var canReadMemory: Bool {
        guard let address = parseAddress(memoryAddress) else { return false }
        return !isBusy && connection != nil && (1...4096).contains(memoryLength)
            && !address.addingReportingOverflow(UInt64(memoryLength)).overflow
    }
    var flashCapacity: UInt64 {
        memoryRegions.filter { $0.kind == .nvm && !$0.isAlias }.reduce(0) { $0 + $1.size }
    }
    var flashOptions: FlashOptions {
        FlashOptions(verify: verify, dryRun: dryRun, skipErase: eraseStrategy == .skip,
                     allowEraseAll: eraseStrategy == .full, resetAfter: resetAfter)
    }
    var targetSelection: TargetSelection {
        TargetSelection(chip: effectiveChip.isEmpty ? nil : effectiveChip, protocol: wireProtocol,
                        speedKhz: validSpeed, connectUnderReset: connectUnderReset)
    }
    var targetConfiguration: TargetConfiguration {
        TargetConfiguration(selectedProbe: selectedProbe, chip: chip, wireProtocol: wireProtocol,
                            speed: speed, connectUnderReset: connectUnderReset)
    }

    func applyTargetConfiguration(_ configuration: TargetConfiguration) {
        selectedProbe = configuration.selectedProbe
        chip = configuration.chip
        wireProtocol = configuration.wireProtocol
        speed = configuration.speed
        connectUnderReset = configuration.connectUnderReset
    }
    var firmwareInput: FirmwareInput? {
        guard let firmwareURL else { return nil }
        return FirmwareInput(path: firmwareURL.path, format: nil, baseAddress: isBin ? parseAddress(binBaseAddress) : nil)
    }
    var storageDirectory: String {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("com.xiaoyu.flashdesk", isDirectory: true).path
    }

    func start() async {
        guard !didStart else { return }
        didStart = true
        appendLog("应用", "Rust Core \(rust.version) 已就绪")
        let directory = storageDirectory
        do {
            profiles = try await rust.run { try $0.loadProfiles(directory: directory) }
            try await loadRecentFilesIfNeeded()
        } catch { report(error, operation: "加载配置") }
    }

    func loadRecentFilesIfNeeded() async throws {
        guard !recentFilesLoaded else { return }
        if let task = recentFilesLoadTask { try await task.value; return }
        let directory = storageDirectory
        let task = Task { [rust] in
            do {
                recentFiles = try await rust.run { try $0.loadRecentFiles(directory: directory) }
                recentFilesLoaded = true
            } catch {
                recentFilesLoadTask = nil
                throw error
            }
        }
        recentFilesLoadTask = task
        try await task.value
    }

    func refreshRecentFileAvailability() {
        missingRecentPaths = Set(recentFiles.compactMap { file in
            FileManager.default.fileExists(atPath: file.path) ? nil : file.path
        })
    }

    func invalidateConnection() {
        connection = nil
        targetInformation = nil
        targetCandidates = []
        memoryResult = nil
        memoryLines = []
    }

    /// Also closes the firmware and drops the last job's progress, so the
    /// previous flash no longer shows once the target is gone.
    func disconnect() {
        guard canDisconnect else { return }
        invalidateConnection()
        clearFirmware()
        latestEvent = nil
        status = "已断开"
        appendLog("连接", "已清除目标连接")
    }

    func refreshMemoryMap() {
        let requestedChip = effectiveChip
        guard !requestedChip.isEmpty else { memoryRegions = []; return }
        Task { [rust] in
            do {
                let regions = try await rust.run { try $0.targetMemoryMap(chip: requestedChip) }
                if effectiveChip == requestedChip { memoryRegions = regions }
            } catch {
                if effectiveChip == requestedChip { memoryRegions = [] }
                appendLog("内存映射", String(describing: error), level: .error)
            }
        }
    }

    var logExportText: String {
        logs.map { entry in
            "\(entry.date.ISO8601Format())\t\(entry.level.rawValue)\t\(entry.operation)\t\(entry.message)\(entry.detail.isEmpty ? "" : "\n\(entry.detail)")"
        }.joined(separator: "\n")
    }

    /// Closes the popovers first so the save panel is not dismissed with them.
    func requestExport(_ export: PendingExport) {
        pendingExport = export
    }

    func appendLog(_ operation: String, _ message: String, level: ActivityEntry.Level = .info, detail: String = "") {
        logs.append(ActivityEntry(operation: operation, level: level, message: message, detail: detail))
        if level == .error && !showLogs { unseenErrors += 1 }
        if logs.count > 2000 { logs.removeFirst(logs.count - 2000) }
    }

    func report(_ error: Error, operation: String) {
        if let coreError = error as? CoreError, case let .Operation(response) = coreError {
            targetCandidates = response.targetCandidates
            if let information = response.targetInformation { targetInformation = information }
            failure = UserFacingError(message: response.message, detail: response.detail ?? "", recovery: response.recovery)
            if !response.targetCandidates.isEmpty {
                failure = nil
                showChipPicker = true
            }
            status = response.detail ?? response.message
            appendLog(operation, response.message, level: .error, detail: response.detail ?? "")
        } else {
            failure = UserFacingError(message: "\(operation)失败", detail: error.localizedDescription, recovery: "")
            status = "\(operation)失败"
            appendLog(operation, error.localizedDescription, level: .error)
        }
    }

    func consumeEvents() {
        for event in rust.takeEvents() {
            latestEvent = event
            status = event.message
            let level: ActivityEntry.Level = event.stage == .failed ? .error : (event.stage == .completed ? .success : .info)
            appendLog(event.kind == .flash ? "烧录" : "擦除", event.message, level: level)
        }
    }

    func perform<T: Sendable>(_ operation: WorkbenchOperation, body: () async throws -> T) async -> T? {
        guard !isBusy else { return nil }
        self.operation = operation
        status = "\(operation.rawValue)…"
        failure = nil
        if operation == .flash || operation == .erase {
            _ = rust.takeEvents()
            latestEvent = nil
        }
        let poller = Task { @MainActor [weak self] in
            while !Task.isCancelled {
                self?.consumeEvents()
                do { try await Task.sleep(for: .milliseconds(100)) }
                catch { break }
            }
        }
        defer {
            poller.cancel()
            consumeEvents()
            self.operation = nil
        }
        do { return try await body() }
        catch { report(error, operation: operation.rawValue); return nil }
    }
}
