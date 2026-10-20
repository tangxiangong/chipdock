using ChipDock.Core;
using Microsoft.UI.Dispatching;
using static ChipDock.Formatting;

namespace ChipDock;

/// Window state shared by every view. Mutations happen on the UI thread and
/// coalesce into one <see cref="Changed"/> notification per dispatcher turn.
internal sealed partial class WorkbenchStore
{
    private readonly DispatcherQueue dispatcher = DispatcherQueue.GetForCurrentThread();
    private bool didStart, notificationPending;
    private CancellationTokenSource? analysisCancellation;
    private int analysisRevision;
    private readonly RecentFirmwareHistory recentHistory = new(
        () => RustCore.LoadRecentFiles(AppSettings.StorageDirectory),
        files => RustCore.SaveRecentFiles(AppSettings.StorageDirectory, files));

    internal WorkbenchStore() => recentHistory.Changed += Notify;

    internal event Action? Changed;
    internal event Action<ActivityEntry>? LogAppended;
    internal event Action? LogsCleared;
    /// A failed operation the window presents as a dialog.
    internal event Action<UserFacingError>? Failed;
    /// Target identification returned candidates; the window opens the chip picker.
    internal event Action? ChipPickerRequested;

    internal string CoreVersion { get; } = RustCore.Version();
    internal string Status { get; private set; } = "准备就绪";
    internal WorkbenchOperation? Operation { get; private set; }
    internal ProgressRecord? LatestEvent { get; private set; }
    internal List<ActivityEntry> Logs { get; } = [];
    internal int UnseenErrors { get; private set; }

    internal bool LogsVisible
    {
        get;
        set { field = value; if (value) UnseenErrors = 0; Notify(); }
    }

    internal List<ProbeSummary> Probes { get; private set; } = [];
    internal bool HasScanned { get; private set; }
    internal ConnectionInfo? Connection { get; private set; }
    internal TargetInformation? TargetInformation { get; private set; }
    internal List<TargetCandidate> TargetCandidates { get; private set; } = [];
    internal List<MemoryRegionLayout> MemoryRegions { get; private set; } = [];

    internal string SelectedProbe
    {
        get;
        set { if (field != value) { field = value; InvalidateConnection(); } }
    } = "";

    internal string Chip
    {
        get;
        set { if (field != value) { field = value; InvalidateConnection(); RefreshMemoryMap(); } }
    } = "";

    internal WireProtocol WireProtocol
    {
        get;
        set { if (field.IsJtag != value.IsJtag) { field = value; InvalidateConnection(); } }
    } = WireProtocol.Swd;

    internal string Speed
    {
        get;
        set { if (field != value) { field = value; InvalidateConnection(); } }
    } = "4000";

    internal bool ConnectUnderReset
    {
        get;
        set { if (field != value) { field = value; InvalidateConnection(); } }
    }

    internal string? FirmwarePath { get; private set; }
    internal FirmwareUsage? FirmwareUsage { get; private set; }
    internal FirmwarePreview? FirmwarePreview { get; private set; }
    internal bool AnalyzingFirmware { get; private set; }
    internal string? FirmwareError { get; private set; }

    internal string BinBaseAddress
    {
        get;
        set { if (field != value) { field = value; if (FirmwarePath is not null) ScheduleFirmwareAnalysis(); Notify(); } }
    } = "0x08000000";

    internal bool Verify { get; set { field = value; Notify(); } } = AppSettings.DefaultVerify;
    internal bool ResetAfter { get; set { field = value; Notify(); } } = AppSettings.DefaultReset;
    internal bool DryRun { get; set { field = value; Notify(); } }
    internal EraseStrategy EraseStrategy { get; set { field = value; Notify(); } }
    internal List<ProfileRecord> Profiles { get; private set; } = [];
    internal List<RecentFirmwareRecord> RecentFiles => recentHistory.Files;
    internal HashSet<string> MissingRecentPaths => recentHistory.MissingPaths;

    internal string MemoryAddress { get; set { field = value; Notify(); } } = "0x08000000";
    internal uint MemoryLength { get; set { field = value; Notify(); } } = 256;
    internal MemoryReadResult? MemoryResult { get; private set; }

    internal bool IsBusy => Operation is not null;
    internal bool IsBin => FirmwarePath is { } path && Path.GetExtension(path).Equals(".bin", StringComparison.OrdinalIgnoreCase);
    internal string EffectiveChip => Connection?.chip ?? Chip;
    internal string TargetName => TargetInformation?.device_type ?? Connection?.chip ?? (Chip.Length == 0 ? "自动识别" : Chip);
    internal uint? ValidSpeed => uint.TryParse(Speed, out var value) && value > 0 ? value : null;
    internal bool CanConnect => !IsBusy && ValidSpeed is not null && (!HasScanned || Probes.Count > 0);

    internal bool CanFlash =>
        !IsBusy && Connection is not null && FirmwareUsage is not null && FirmwareError is null && !AnalyzingFirmware
        && (!IsBin || ParseAddress(BinBaseAddress) is not null);

    internal bool CanDisconnect => !IsBusy && Connection is not null;

    internal bool CanErase => !IsBusy && Connection is not null;

    internal bool CanReadMemory =>
        ParseAddress(MemoryAddress) is { } address && !IsBusy && Connection is not null
        && MemoryLength is >= 1 and <= 4096 && address <= ulong.MaxValue - MemoryLength;

    internal FlashOptions FlashOptions => new()
    {
        verify = Verify,
        dry_run = DryRun,
        skip_erase = EraseStrategy == EraseStrategy.Skip,
        allow_erase_all = EraseStrategy == EraseStrategy.Full,
        reset_after = ResetAfter,
    };

    internal TargetSelection TargetSelection => new()
    {
        chip = EffectiveChip.Length == 0 ? null : EffectiveChip,
        protocol = WireProtocol,
        speed_khz = ValidSpeed,
        connect_under_reset = ConnectUnderReset,
    };

    internal FirmwareInput? FirmwareInput => FirmwarePath is { } path
        ? new FirmwareInput { path = path, format = null, base_address = IsBin ? ParseAddress(BinBaseAddress) : null }
        : null;

    private string? ProbeSelector => SelectedProbe.Length == 0 ? null : SelectedProbe;

    internal async Task Start()
    {
        if (didStart) return;
        didStart = true;
        AppendLog("应用", $"Rust Core {CoreVersion} 已就绪");
        try
        {
            Profiles = await RustCore.LoadProfiles(AppSettings.StorageDirectory);
            await recentHistory.Load();
            Notify();
        }
        catch (Exception error) { Report(error, "加载配置"); }
    }

    internal void RefreshRecentFileAvailability() => recentHistory.RefreshAvailability();

    internal void InvalidateConnection()
    {
        Connection = null;
        TargetInformation = null;
        TargetCandidates = [];
        MemoryResult = null;
        Notify();
    }

    /// Also closes the firmware and drops the last job's progress, so the
    /// previous flash no longer shows once the target is gone.
    internal void Disconnect()
    {
        if (!CanDisconnect) return;
        InvalidateConnection();
        ClearFirmware();
        LatestEvent = null;
        SetStatus("已断开");
        AppendLog("连接", "已清除目标连接");
    }

    internal async void RefreshMemoryMap()
    {
        var requestedChip = EffectiveChip;
        if (requestedChip.Length == 0) { MemoryRegions = []; Notify(); return; }
        try
        {
            var regions = await RustCore.TargetMemoryMap(requestedChip);
            if (EffectiveChip == requestedChip) { MemoryRegions = regions; Notify(); }
        }
        catch (Exception error)
        {
            if (EffectiveChip == requestedChip) { MemoryRegions = []; Notify(); }
            AppendLog("内存映射", error.Message, ActivityLevel.Error, (error as CoreException)?.Response.detail ?? "");
        }
    }

    internal ulong FlashCapacity => MemoryRegions
        .Where(region => region.kind.IsNvm && !region.is_alias)
        .Aggregate(0UL, (total, region) => total + region.size);

    internal string LogExportText => string.Join(Environment.NewLine, Logs.Select(entry =>
        $"{entry.Date.UtcDateTime:yyyy-MM-ddTHH:mm:ssZ}\t{entry.Level.Title()}\t{entry.Operation}\t{entry.Message}"
        + (entry.Detail.Length == 0 ? "" : Environment.NewLine + entry.Detail)));

    internal void AppendLog(string operation, string message, ActivityLevel level = ActivityLevel.Info, string detail = "")
    {
        var entry = new ActivityEntry(DateTimeOffset.Now, operation, level, message, detail);
        Logs.Add(entry);
        if (level == ActivityLevel.Error && !LogsVisible) UnseenErrors++;
        if (Logs.Count > 2000)
        {
            Logs.RemoveRange(0, Logs.Count - 2000);
            LogsCleared?.Invoke();
        }
        else LogAppended?.Invoke(entry);
        Notify();
    }

    internal void ClearLogs()
    {
        Logs.Clear();
        UnseenErrors = 0;
        LogsCleared?.Invoke();
        Notify();
    }

    internal void Report(Exception error, string operation)
    {
        if (error is CoreException { Response: var response })
        {
            TargetCandidates = response.target_candidates;
            if (response.target_information is { } information) TargetInformation = information;
            SetStatus(response.detail ?? response.message);
            AppendLog(operation, response.message, ActivityLevel.Error, response.detail ?? "");
            if (response.target_candidates.Count > 0) ChipPickerRequested?.Invoke();
            else Failed?.Invoke(new UserFacingError(response.message, response.detail ?? "", response.recovery));
        }
        else
        {
            SetStatus($"{operation}失败");
            AppendLog(operation, error.Message, ActivityLevel.Error);
            Failed?.Invoke(new UserFacingError($"{operation}失败", error.Message, ""));
        }
    }

    private void SetStatus(string status)
    {
        Status = status;
        Notify();
    }

    private void ConsumeEvents()
    {
        foreach (var progress in RustCore.TakeEvents())
        {
            LatestEvent = progress;
            Status = progress.message;
            var level = progress.stage.IsFailed ? ActivityLevel.Error : progress.stage.IsCompleted ? ActivityLevel.Success : ActivityLevel.Info;
            AppendLog(progress.kind.IsFlash ? "烧录" : "擦除", progress.message, level);
        }
    }

    /// Runs one hardware operation at a time, polling job progress while it blocks.
    internal async Task<T?> Perform<T>(WorkbenchOperation operation, Func<Task<T>> body) where T : class
    {
        if (IsBusy) return null;
        Operation = operation;
        Status = $"{operation.Title()}…";
        if (operation is WorkbenchOperation.Flash or WorkbenchOperation.Erase)
        {
            _ = RustCore.TakeEvents();
            LatestEvent = null;
        }
        Notify();
        using var polling = new CancellationTokenSource();
        var poller = PollEvents(polling.Token);
        try { return await body(); }
        catch (Exception error) { Report(error, operation.Title()); return null; }
        finally
        {
            polling.Cancel();
            await poller;
            ConsumeEvents();
            Operation = null;
            Notify();
        }
    }

    private async Task PollEvents(CancellationToken cancellation)
    {
        while (!cancellation.IsCancellationRequested)
        {
            ConsumeEvents();
            try { await Task.Delay(100, cancellation); }
            catch (OperationCanceledException) { break; }
        }
    }

    internal void Notify()
    {
        if (notificationPending) return;
        notificationPending = true;
        dispatcher.TryEnqueue(() =>
        {
            notificationPending = false;
            Changed?.Invoke();
        });
    }
}
