using System.ComponentModel;
using System.Diagnostics;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Runtime.Versioning;
using System.Text;
using FlashDesk.Core;

namespace FlashDesk;

/// Repairs a missing ST-Link debug driver with Windows' signed, inbox WinUSB driver.
/// No vendor download or architecture-specific coinstaller is needed, including on ARM64.
[SupportedOSPlatform("windows")]
internal static class StLinkDriver
{
    internal const string InstallArgument = "--install-stlink-driver";

    internal static bool IsSupported(ushort vendor, ushort product) => vendor == 0x0483 && product is
        0x3748 or 0x374b or 0x374a or 0x3742 or 0x3752 or 0x374e or 0x374f or 0x3753 or 0x3754 or 0x3757;

    internal static ProbeSummary? SelectProbe(string? selector, IReadOnlyList<ProbeSummary> probes)
    {
        if (string.IsNullOrWhiteSpace(selector)) return probes.Count == 1 ? probes[0] : null;
        // Saved profiles may omit the serial or use an explicit interface number.
        // Match the same VID:PID[-interface][:serial] syntax that probe-rs accepts.
        var parts = selector.Trim().Split(':', 3);
        if (parts.Length < 2 || !ushort.TryParse(parts[0], NumberStyles.AllowHexSpecifier, CultureInfo.InvariantCulture, out var vendor)) return null;
        var productParts = parts[1].Split('-', 2);
        if (!ushort.TryParse(productParts[0], NumberStyles.AllowHexSpecifier, CultureInfo.InvariantCulture, out var product)
            || (productParts.Length == 2 && productParts[1].Length > 0
                && !byte.TryParse(productParts[1], NumberStyles.None, CultureInfo.InvariantCulture, out _))) return null;
        // Other probe implementations use interface numbers to select different
        // devices. Do not rewrite their explicit selectors using ST-Link rules.
        if (!IsSupported(vendor, product)) return probes.FirstOrDefault(probe => probe.identifier == selector.Trim());
        var matches = probes.Where(probe => probe.vendor_id == vendor && probe.product_id == product
            && (parts.Length < 3 || string.Equals(probe.serial_number ?? "", parts[2], StringComparison.Ordinal))).ToArray();
        return matches.Length == 1 ? matches[0] : null;
    }

    internal static StLinkUsb.Device? SelectDevice(ProbeSummary probe, IReadOnlyList<StLinkUsb.Device> devices)
    {
        if (!IsSupported(probe.vendor_id, probe.product_id)) return null;
        var matches = devices.Where(device => device.Vendor == probe.vendor_id && device.Product == probe.product_id
            && (string.IsNullOrEmpty(probe.serial_number)
                || SerialMatches(device.Serial, probe.serial_number))).ToArray();
        // Never repair a different probe when the choice is ambiguous.
        return matches.Length == 1 ? matches[0] : null;
    }

    private static bool SerialMatches(string windowsSerial, string probeSerial)
    {
        if (windowsSerial.Equals(probeSerial, StringComparison.OrdinalIgnoreCase)) return true;
        // probe-rs hex-encodes ST-Link serial descriptors shorter than 24 UTF-8 bytes.
        // Windows instance IDs can also change the descriptor's letter case.
        if (Encoding.UTF8.GetByteCount(windowsSerial) >= 24) return false;
        try
        {
            var bytes = Convert.FromHexString(probeSerial);
            return bytes.Length < 24 && windowsSerial.Equals(new UTF8Encoding(false, true).GetString(bytes), StringComparison.OrdinalIgnoreCase);
        }
        catch (FormatException) { return false; }
        catch (DecoderFallbackException) { return false; }
    }

    internal static Task EnsureInstalled(ProbeSummary? probe, Action<string> report) =>
        EnsureInstalled(probe, report, new WindowsDriverSystem());

    internal static async Task EnsureInstalled(ProbeSummary? probe, Action<string> report, IStLinkDriverSystem system)
    {
        if (probe is null || !IsSupported(probe.vendor_id, probe.product_id)) return;
        var device = SelectDevice(probe, await system.ListDevices());
        if (device is null || device.Ready || (!device.NeedsRepair && !device.UsesWinUsb)) return;
        if (device.NeedsRepair)
        {
            if (!system.NativeArchitecture)
                throw new PlatformNotSupportedException($"自动安装驱动需要使用 {RuntimeInformation.OSArchitecture} 版 FlashDesk，请安装对应版本后重试。");
            if (device.Service.Length == 0) await system.CheckInboxDriver(device.InstanceId);
            report("检测到 ST-Link 驱动缺失，正在安装；请允许管理员授权…");
            await system.Install(device.InstanceId);
        }
        report("正在等待 ST-Link 调试接口就绪…");
        // Device restart and interface registration can complete after the helper exits.
        for (var attempt = 0; attempt < 20; attempt++)
        {
            var current = (await system.ListDevices()).FirstOrDefault(candidate =>
                candidate.InstanceId.Equals(device.InstanceId, StringComparison.OrdinalIgnoreCase));
            if (current is { Ready: true }) return;
            await system.WaitForDevice();
        }
        throw new IOException("ST-Link 驱动尚未就绪。请重新插拔 ST-Link 后再连接。");
    }

    private static async Task InstallElevated(string instanceId)
    {
        // Relaunch only the short-lived driver helper; the workbench stays unelevated.
        var start = new ProcessStartInfo(Path.Combine(AppContext.BaseDirectory, "FlashDesk.exe"))
        {
            UseShellExecute = true,
            Verb = "runas",
            WindowStyle = ProcessWindowStyle.Hidden,
        };
        start.ArgumentList.Add(InstallArgument);
        start.ArgumentList.Add(instanceId);
        try
        {
            using var process = Process.Start(start) ?? throw new IOException("无法启动 ST-Link 驱动安装程序。");
            // Do not interrupt an in-flight system driver installation.
            await process.WaitForExitAsync();
            CheckInstallerExitCode(process.ExitCode);
        }
        catch (Win32Exception error) when (error.NativeErrorCode == 1223)
        {
            throw new OperationCanceledException("已取消 ST-Link 驱动安装。再次点击连接可重新安装。");
        }
    }

    internal static void CheckInstallerExitCode(int exitCode)
    {
        if (exitCode == 3010) throw new IOException("ST-Link 驱动已安装，需要重新启动 Windows 后再连接。");
        if (exitCode != 0)
            throw new IOException($"ST-Link 驱动安装失败：{new Win32Exception(exitCode).Message}（0x{exitCode:X8}）。请检查设备连接后重试。");
    }

    /// Handled before WinUI, updater hooks, single-instance redirection and Rust startup.
    internal static int RunInstaller(string[] arguments)
    {
        if (arguments.Length != 1 || !StLinkUsb.IsDebugInterface(arguments[0])) return 87;
        if (RuntimeInformation.ProcessArchitecture != RuntimeInformation.OSArchitecture) return 216;
        try { return StLinkUsb.Install(arguments[0]) ? 3010 : 0; }
        catch (Win32Exception error) { return error.NativeErrorCode; }
        catch (UnauthorizedAccessException) { return 5; }
        catch (Exception) { return 1; }
    }

    private sealed class WindowsDriverSystem : IStLinkDriverSystem
    {
        public bool NativeArchitecture => RuntimeInformation.ProcessArchitecture == RuntimeInformation.OSArchitecture;
        public Task<IReadOnlyList<StLinkUsb.Device>> ListDevices() => Task.Run(StLinkUsb.ListDevices);
        public Task CheckInboxDriver(string instanceId) => Task.Run(() => StLinkUsb.CheckInboxDriver(instanceId));
        public Task Install(string instanceId) => InstallElevated(instanceId);
        public Task WaitForDevice() => Task.Delay(250);
    }
}

[SupportedOSPlatform("windows")]
internal interface IStLinkDriverSystem
{
    bool NativeArchitecture { get; }
    Task<IReadOnlyList<StLinkUsb.Device>> ListDevices();
    Task CheckInboxDriver(string instanceId);
    Task Install(string instanceId);
    Task WaitForDevice();
}
