using ChipDock.Core;
using static ChipDock.Formatting;

namespace ChipDock;

internal sealed partial class WorkbenchStore
{
    internal async Task ScanProbes(bool updateSelection = true)
    {
        var result = await Perform(WorkbenchOperation.Scan, ProbeDriver.ListProbes);
        if (result is null) return;
        HasScanned = true;
        Probes = result;
        if (updateSelection)
        {
            if (result.Count == 1) SelectedProbe = result[0].identifier;
            else if (!result.Any(probe => probe.identifier == SelectedProbe)) { SelectedProbe = ""; InvalidateConnection(); }
        }
        SetStatus(result.Count == 0 ? "未发现调试探针" : $"发现 {result.Count} 个调试探针");
        AppendLog("探针", Status);
    }

    internal async Task Connect()
    {
        if (!CanConnect) return;
        var request = new ConnectRequest { probe = ProbeSelector, target = TargetSelection };
        InvalidateConnection();
        var result = await Perform(WorkbenchOperation.Connect, async () =>
        {
            var probes = await ProbeDriver.ListProbes();
            Probes = probes;
            HasScanned = true;
            Notify();
            var selected = ProbeDriver.SelectProbe(request.probe, probes);
            // Pin the chosen probe while UAC and device restart are in progress.
            if (selected is not null) request.probe = selected.identifier;
            request.probe = await ProbeDriver.PrepareProbe(selected, message =>
            {
                SetStatus(message);
                AppendLog("驱动", message);
            }) ?? request.probe;
            return await RustCore.ConnectTarget(request);
        });
        if (result is null) return;
        // These setters invalidate the connection, so apply them before storing it.
        SelectedProbe = result.probe;
        WireProtocol = result.protocol;
        Speed = (result.speed_khz ?? ValidSpeed ?? 4000).ToString();
        ConnectUnderReset = result.connect_under_reset;
        Connection = result;
        TargetInformation = result.target_information;
        TargetCandidates = [];
        SetStatus($"{TargetName} 已就绪");
        AppendLog("连接", $"{TargetName} · {result.protocol.Title()} · {Speed} kHz", ActivityLevel.Success);
        RefreshMemoryMap();
    }

    internal async Task Flash()
    {
        if (!CanFlash || FirmwareInput is not { } input) return;
        var request = new FlashRequest { firmware = input, probe = ProbeSelector, target = TargetSelection, options = FlashOptions };
        await Perform(WorkbenchOperation.Flash, () => RustCore.FlashFirmware(request));
    }

    internal async Task Erase()
    {
        if (!CanErase) return;
        var request = new EraseRequest { probe = ProbeSelector, target = TargetSelection };
        await Perform(WorkbenchOperation.Erase, () => RustCore.EraseTarget(request));
        MemoryResult = null;
        Notify();
    }

    internal async Task ReadMemory()
    {
        if (!CanReadMemory || ParseAddress(MemoryAddress) is not { } address) return;
        var request = new MemoryRequest { probe = ProbeSelector, target = TargetSelection, address = address, length = MemoryLength };
        var result = await Perform(WorkbenchOperation.Read, () => RustCore.ReadMemory(request));
        if (result is null) return;
        MemoryResult = result;
        SetStatus($"已读取 {result.length} 字节");
        AppendLog("内存", $"{AddressLabel(result.address)} · {result.length} 字节", ActivityLevel.Success);
    }

    internal void SelectFirmware(string path)
    {
        if (IsBusy) return;
        FirmwarePath = path;
        ScheduleFirmwareAnalysis();
    }

    internal void ClearFirmware()
    {
        if (IsBusy) return;
        analysisCancellation?.Cancel();
        analysisRevision++;
        FirmwarePath = null;
        FirmwareUsage = null;
        FirmwarePreview = null;
        FirmwareError = null;
        AnalyzingFirmware = false;
        Notify();
    }

    internal void ScheduleFirmwareAnalysis()
    {
        analysisCancellation?.Cancel();
        var revision = ++analysisRevision;
        FirmwareUsage = null;
        FirmwarePreview = null;
        FirmwareError = null;
        AnalyzingFirmware = false;
        Notify();
        if (FirmwareInput is not { } input) return;
        if (IsBin && ParseAddress(BinBaseAddress) is null)
        {
            FirmwareError = "请输入有效的 BIN 基地址";
            return;
        }
        AnalyzingFirmware = true;
        analysisCancellation = new CancellationTokenSource();
        _ = AnalyzeFirmware(input, revision, analysisCancellation.Token);
    }

    private async Task AnalyzeFirmware(FirmwareInput input, int revision, CancellationToken cancellation)
    {
        FirmwarePreview preview;
        try
        {
            await Task.Delay(200, cancellation);
            preview = await RustCore.FirmwarePreview(input);
            if (cancellation.IsCancellationRequested || revision != analysisRevision) return;
        }
        catch (OperationCanceledException)
        {
            if (revision == analysisRevision) { AnalyzingFirmware = false; Notify(); }
            return;
        }
        catch (Exception error)
        {
            if (revision != analysisRevision) return;
            AnalyzingFirmware = false;
            FirmwareError = error is CoreException { Response: var response } ? response.detail ?? response.message : error.Message;
            AppendLog("固件分析", FirmwareError, ActivityLevel.Error);
            return;
        }
        var usage = preview.usage;
        FirmwareUsage = usage;
        FirmwarePreview = preview;
        AnalyzingFirmware = false;
        AppendLog("固件", $"{Path.GetFileName(input.path)} · {usage.format.Title()} · {ByteCountLabel(usage.used_bytes)}");

        try
        {
            await recentHistory.Record(new RecentFirmwareRecord
            {
                path = input.path,
                format = usage.format,
                base_address = input.base_address,
                used_at_ms = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(),
            });
        }
        catch (Exception error)
        {
            AppendLog("最近使用", error.Message, ActivityLevel.Error, (error as CoreException)?.Response.detail ?? "");
        }
    }

    internal void OpenRecent(RecentFirmwareRecord file)
    {
        if (IsBusy) return;
        RefreshRecentFileAvailability();
        if (MissingRecentPaths.Contains(file.path))
        {
            SetStatus("固件文件不存在");
            AppendLog("打开固件", Status, ActivityLevel.Error, file.path);
            Failed?.Invoke(new UserFacingError("找不到固件文件", file.path,
                "文件可能已被移动或删除。请选择其他固件，或从最近使用中移除此记录。"));
            return;
        }
        if (file.base_address is { } address) BinBaseAddress = AddressLabel(address);
        SelectFirmware(file.path);
    }

    internal async Task RemoveRecentFiles(IEnumerable<string> paths)
    {
        if (IsBusy) return;
        var removed = paths.ToHashSet(StringComparer.OrdinalIgnoreCase);
        if (removed.Count == 0) return;
        try { await recentHistory.Remove(removed); }
        catch (Exception error) { Report(error, "移除最近使用记录"); }
    }

    internal async Task ClearMissingRecentFiles()
    {
        RefreshRecentFileAvailability();
        await RemoveRecentFiles(MissingRecentPaths);
    }

    internal async Task ClearRecentFiles()
    {
        if (IsBusy) return;
        try { await recentHistory.Clear(); }
        catch (Exception error) { Report(error, "清除最近使用"); }
    }

    internal void ApplyProfile(ProfileRecord profile)
    {
        if (IsBusy) return;
        InvalidateConnection();
        SelectedProbe = profile.probe ?? "";
        Chip = profile.target.chip ?? "";
        WireProtocol = profile.target.protocol;
        Speed = (profile.target.speed_khz ?? 4000).ToString();
        ConnectUnderReset = profile.target.connect_under_reset;
        Verify = profile.flash_options.verify;
        ResetAfter = profile.flash_options.reset_after;
        DryRun = profile.flash_options.dry_run;
        EraseStrategy = profile.flash_options.skip_erase ? EraseStrategy.Skip
            : profile.flash_options.allow_erase_all ? EraseStrategy.Full : EraseStrategy.Automatic;
        if (profile.bin_base_address is { } address) BinBaseAddress = AddressLabel(address);
        if (FirmwarePath is not null) ScheduleFirmwareAnalysis();
        RefreshMemoryMap();
        SetStatus($"已载入配置“{profile.name}”");
        AppendLog("配置", Status);
    }

    internal async Task<bool> SaveProfile(string name)
    {
        var trimmed = name.Trim();
        if (trimmed.Length == 0) return false;
        var profile = new ProfileRecord
        {
            id = Guid.NewGuid().ToString(),
            name = trimmed,
            probe = ProbeSelector,
            target = TargetSelection,
            flash_options = FlashOptions,
            bin_base_address = ParseAddress(BinBaseAddress),
            updated_at_ms = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(),
        };
        if (!await PersistProfiles([.. Profiles, profile])) return false;
        SetStatus($"已保存配置“{trimmed}”");
        return true;
    }

    internal async Task DeleteProfile(ProfileRecord profile)
    {
        if (IsBusy) return;
        await PersistProfiles(Profiles.Where(saved => saved.id != profile.id).ToList());
    }

    private async Task<bool> PersistProfiles(List<ProfileRecord> updated)
    {
        try
        {
            await RustCore.SaveProfiles(AppSettings.StorageDirectory, updated);
            Profiles = updated;
            Notify();
            return true;
        }
        catch (Exception error) { Report(error, "保存配置"); return false; }
    }
}
