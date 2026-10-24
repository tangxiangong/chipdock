using ChipDock.Core;

namespace ChipDock;

/// A structured Rust Core error; target candidates and recovery advice survive the bridge.
internal sealed class CoreException(ErrorResponse response) : Exception(response.message)
{
    internal ErrorResponse Response { get; } = response;
}

// Interoptopus generates the C# ABI and binary wire codecs. Blocking USB and file
// work runs on the thread pool; Rust serializes hardware access itself and reports
// overlapping jobs as JobAlreadyRunning. Each returned wire buffer is disposed on
// its worker thread after Unwire has produced managed, owned DTOs.
internal static class RustCore
{
    private static void Check(ErrorResponse? error)
    {
        if (error is not null) throw new CoreException(error);
    }

    private static T Require<T>(T? value, string operation) where T : class =>
        value ?? throw new InvalidOperationException($"Rust Core 未返回{operation}结果");

    internal static string Version()
    {
        using var wire = Interop.version().AsOk();
        return wire.Unwire().text;
    }

    internal static Task<List<ProbeSummary>> ListProbes() => NativeWorker.Probe.Run(() =>
    {
        using var wire = Interop.list_probes().AsOk();
        var response = wire.Unwire(); Check(response.error);
        return response.payload;
    });

    internal static Task<ConnectionInfo> ConnectTarget(ConnectRequest value) => NativeWorker.Probe.Run(() =>
    {
        using var request = value.Wire();
        using var wire = Interop.connect_target(request).AsOk();
        var response = wire.Unwire(); Check(response.error);
        return Require(response.payload, "连接");
    });

    internal static Task<List<string>> SearchChips(string query, uint limit) => NativeWorker.Probe.Run(() =>
    {
        using var request = new ChipSearchRequest { query = query, limit = limit }.Wire();
        using var wire = Interop.search_chips(request).AsOk();
        var response = wire.Unwire(); Check(response.error);
        return response.payload;
    });

    internal static Task<List<MemoryRegionLayout>> TargetMemoryMap(string chip) => NativeWorker.Probe.Run(() =>
    {
        using var request = new TextValue { text = chip }.Wire();
        using var wire = Interop.target_memory_map(request).AsOk();
        var response = wire.Unwire(); Check(response.error);
        return response.payload;
    });

    internal static Task<FirmwareUsage> FirmwareUsage(FirmwareInput value) => NativeWorker.Probe.Run(() =>
    {
        using var request = value.Wire();
        using var wire = Interop.firmware_usage(request).AsOk();
        var response = wire.Unwire(); Check(response.error);
        return Require(response.payload, "固件分析");
    });

    internal static Task<FirmwarePreview> FirmwarePreview(FirmwareInput value, uint length = 4096) => NativeWorker.Probe.Run(() =>
    {
        using var request = new FirmwarePreviewRequest { firmware = value, length = length }.Wire();
        using var wire = Interop.firmware_preview(request).AsOk();
        var response = wire.Unwire(); Check(response.error);
        return Require(response.payload, "固件预览");
    });

    /// Blocks until the job finishes; progress is drained separately with <see cref="TakeEvents"/>.
    internal static Task<string> FlashFirmware(FlashRequest value) => NativeWorker.Probe.Run(() =>
    {
        using var request = value.Wire();
        using var wire = Interop.flash_firmware(request).AsOk();
        var response = wire.Unwire(); Check(response.error);
        return Require(response.payload, "烧录");
    });

    internal static Task<string> EraseTarget(EraseRequest value) => NativeWorker.Probe.Run(() =>
    {
        using var request = value.Wire();
        using var wire = Interop.erase_target(request).AsOk();
        var response = wire.Unwire(); Check(response.error);
        return Require(response.payload, "擦除");
    });

    internal static Task<MemoryReadResult> ReadMemory(MemoryRequest value) => NativeWorker.Probe.Run(() =>
    {
        using var request = value.Wire();
        using var wire = Interop.read_memory(request).AsOk();
        var response = wire.Unwire(); Check(response.error);
        return Require(response.payload, "内存读取");
    });

    /// Only takes the event queue lock, so it is safe to call from the UI thread.
    internal static List<ProgressRecord> TakeEvents()
    {
        using var wire = Interop.take_events().AsOk();
        return wire.Unwire().payload;
    }

    internal static Task<List<ProfileRecord>> LoadProfiles(string directory) => NativeWorker.Probe.Run(() =>
    {
        using var request = new DirectoryRequest { directory = directory }.Wire();
        using var wire = Interop.load_profiles(request).AsOk();
        var response = wire.Unwire(); Check(response.error);
        return response.payload;
    });

    internal static Task SaveProfiles(string directory, List<ProfileRecord> profiles) => NativeWorker.Probe.Run(() =>
    {
        using var request = new SaveProfilesRequest { directory = directory, profiles = profiles }.Wire();
        using var wire = Interop.save_profiles(request).AsOk();
        Check(wire.Unwire().error);
    });

    internal static Task<List<RecentFirmwareRecord>> LoadRecentFiles(string directory) => NativeWorker.Probe.Run(() =>
    {
        using var request = new DirectoryRequest { directory = directory }.Wire();
        using var wire = Interop.load_recent_files(request).AsOk();
        var response = wire.Unwire(); Check(response.error);
        return response.payload;
    });

    internal static Task SaveRecentFiles(string directory, List<RecentFirmwareRecord> files) => NativeWorker.Probe.Run(() =>
    {
        using var request = new SaveRecentFilesRequest { directory = directory, files = files }.Wire();
        using var wire = Interop.save_recent_files(request).AsOk();
        Check(wire.Unwire().error);
    });
}
