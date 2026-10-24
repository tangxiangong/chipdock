import Foundation

extension WorkbenchStore {
    func scanProbes(updateSelection: Bool = true) async {
        let result = await perform(.scan) { [rust] in try await rust.run { try $0.listProbes() } }
        guard let result else { return }
        hasScanned = true
        probes = result
        if updateSelection && !showTargetSetup {
            if result.count == 1 { selectedProbe = result[0].identifier }
            else if !result.contains(where: { $0.identifier == selectedProbe }) { selectedProbe = ""; invalidateConnection() }
        }
        status = result.isEmpty ? "未发现调试探针" : "发现 \(result.count) 个调试探针"
        appendLog("探针", status)
    }

    func connect() async {
        guard canConnect else { return }
        let request = ConnectRequest(probe: selectedProbe.isEmpty ? nil : selectedProbe, target: targetSelection)
        let result = await perform(.connect) { [rust] in try await rust.run { try $0.connectTarget(request: request) } }
        guard let result else { return }
        selectedProbe = result.probe
        wireProtocol = result.protocol
        speed = String(result.speedKhz ?? validSpeed ?? 4000)
        connectUnderReset = result.connectUnderReset
        connection = result
        targetInformation = result.targetInformation
        targetCandidates = []
        status = "\(targetName) 已就绪"
        appendLog("连接", "\(targetName) · \(result.protocol.title) · \(speed) kHz", level: .success)
        refreshMemoryMap()
    }

    func flash() async {
        guard canFlash, let input = firmwareInput else { return }
        let request = FlashRequest(firmware: input, probe: selectedProbe.isEmpty ? nil : selectedProbe,
                                   target: targetSelection, options: flashOptions)
        _ = await perform(.flash) { [rust] in try await rust.run { try $0.flashFirmware(request: request) } }
    }

    func erase() async {
        guard canErase else { return }
        let request = EraseRequest(probe: selectedProbe.isEmpty ? nil : selectedProbe, target: targetSelection)
        _ = await perform(.erase) { [rust] in try await rust.run { try $0.eraseTarget(request: request) } }
        memoryResult = nil
        memoryLines = []
    }

    func readMemory() async {
        guard canReadMemory, let address = parseAddress(memoryAddress) else { return }
        let request = MemoryRequest(probe: selectedProbe.isEmpty ? nil : selectedProbe, target: targetSelection,
                                    address: address, length: memoryLength)
        let result = await perform(.read) { [rust] in try await rust.run { try $0.readMemory(request: request) } }
        guard let result else { return }
        memoryResult = result
        memoryLines = result.lines
        status = "已读取 \(result.length) 字节"
        appendLog("内存", "\(addressLabel(result.address)) · \(result.length) 字节", level: .success)
    }

    func selectFirmware(_ url: URL) {
        guard !isBusy else { return }
        firmwareURL = url
        scheduleFirmwareAnalysis()
    }

    func clearFirmware() {
        guard !isBusy else { return }
        analysisTask?.cancel()
        analysisRevision = UUID()
        firmwareURL = nil
        firmwareUsage = nil
        firmwarePreview = nil
        firmwareError = nil
        analyzingFirmware = false
    }

    func scheduleFirmwareAnalysis() {
        analysisTask?.cancel()
        analysisRevision = UUID()
        let revision = analysisRevision
        firmwareUsage = nil
        firmwarePreview = nil
        firmwareError = nil
        guard let input = firmwareInput else { return }
        if isBin && parseAddress(binBaseAddress) == nil {
            firmwareError = "请输入有效的 BIN 基地址"
            analyzingFirmware = false
            return
        }
        analyzingFirmware = true
        analysisTask = Task { [rust] in
            do {
                try await Task.sleep(for: .milliseconds(200))
                let preview = try await rust.run { try $0.firmwarePreview(input: input, length: 4096) }
                guard !Task.isCancelled, revision == analysisRevision else { return }
                let usage = preview.usage
                firmwareUsage = usage
                firmwarePreview = preview
                analyzingFirmware = false
                appendLog("固件", "\(URL(fileURLWithPath: input.path).lastPathComponent) · \(usage.format.title) · \(byteCountLabel(usage.usedBytes))")
                try await updateRecentFiles { files in
                    var updated = files.filter { $0.path != input.path }
                    updated.insert(RecentFirmware(path: input.path, format: usage.format,
                                                  baseAddress: input.baseAddress, usedAt: .now), at: 0)
                    return Array(updated.prefix(10))
                }
            } catch is CancellationError {
                if revision == analysisRevision { analyzingFirmware = false }
            } catch {
                guard revision == analysisRevision else { return }
                analyzingFirmware = false
                if let coreError = error as? CoreError, case let .Operation(response) = coreError {
                    firmwareError = response.detail ?? response.message
                } else { firmwareError = error.localizedDescription }
                appendLog("固件分析", firmwareError ?? "固件分析失败", level: .error)
            }
        }
    }

    func openRecent(_ file: RecentFirmware) {
        guard !isBusy else { return }
        refreshRecentFileAvailability()
        guard !missingRecentPaths.contains(file.path) else {
            failure = UserFacingError(message: "找不到固件文件", detail: file.path,
                                      recovery: "文件可能已被移动或删除。请选择其他固件，或从最近使用中移除此记录。")
            status = "固件文件不存在"
            appendLog("打开固件", status, level: .error, detail: file.path)
            return
        }
        if let address = file.baseAddress { binBaseAddress = addressLabel(address) }
        selectFirmware(URL(fileURLWithPath: file.path))
    }

    /// Serialize history edits and apply each edit to the latest saved list.
    /// Opening a file and removing another record must not overwrite each other.
    private func updateRecentFiles(_ update: @escaping ([RecentFirmware]) -> [RecentFirmware]) async throws {
        let previous = recentFilesUpdateTask
        let task = Task { [rust] in
            _ = try? await previous?.value
            try await loadRecentFilesIfNeeded()
            let files = update(recentFiles)
            let directory = storageDirectory
            try await rust.run { try $0.saveRecentFiles(directory: directory, files: files) }
            recentFiles = files
        }
        recentFilesUpdateTask = task
        try await task.value
    }

    func removeRecentFiles(at paths: Set<String>) async {
        guard !isBusy, !paths.isEmpty else { return }
        do {
            try await updateRecentFiles { $0.filter { !paths.contains($0.path) } }
        } catch { report(error, operation: "移除最近使用记录") }
    }

    func clearMissingRecentFiles() async {
        refreshRecentFileAvailability()
        await removeRecentFiles(at: missingRecentPaths)
    }

    func clearRecentFiles() async {
        guard !isBusy else { return }
        do {
            try await updateRecentFiles { _ in [] }
        } catch { report(error, operation: "清除最近使用") }
    }

    func applyProfile(_ profile: SavedProfile) {
        guard !isBusy else { return }
        invalidateConnection()
        selectedProbe = profile.probe ?? ""
        chip = profile.target.chip ?? ""
        wireProtocol = profile.target.protocol
        speed = String(profile.target.speedKhz ?? 4000)
        connectUnderReset = profile.target.connectUnderReset
        verify = profile.flashOptions.verify
        resetAfter = profile.flashOptions.resetAfter
        dryRun = profile.flashOptions.dryRun
        eraseStrategy = profile.flashOptions.skipErase ? .skip : (profile.flashOptions.allowEraseAll ? .full : .automatic)
        if let address = profile.binBaseAddress { binBaseAddress = addressLabel(address) }
        if firmwareURL != nil { scheduleFirmwareAnalysis() }
        refreshMemoryMap()
        status = "已载入配置“\(profile.name)”"
        appendLog("配置", status)
    }

    func saveProfile(name: String) async {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        let profile = SavedProfile(id: UUID().uuidString, name: trimmed, probe: selectedProbe.isEmpty ? nil : selectedProbe,
                                   target: targetSelection, flashOptions: flashOptions, binBaseAddress: parseAddress(binBaseAddress), updatedAt: .now)
        let updated = profiles + [profile]
        if await persistProfiles(updated) { showSaveProfile = false; status = "已保存配置“\(trimmed)”" }
    }

    func deleteProfile(_ profile: SavedProfile) async {
        guard !isBusy else { return }
        _ = await persistProfiles(profiles.filter { $0.id != profile.id })
    }

    private func persistProfiles(_ updated: [SavedProfile]) async -> Bool {
        let directory = storageDirectory
        do {
            try await rust.run { try $0.saveProfiles(directory: directory, profiles: updated) }
            profiles = updated
            return true
        } catch { report(error, operation: "保存配置"); return false }
    }
}
