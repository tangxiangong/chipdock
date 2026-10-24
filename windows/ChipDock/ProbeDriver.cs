using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Runtime.Versioning;
using System.Text;
using ChipDock.Core;

namespace ChipDock;

/// Repairs supported debug interfaces with Windows' signed, inbox WinUSB driver.
/// No vendor download or architecture-specific coinstaller is needed, including on ARM64.
internal static class ProbeDriver
{
    internal const string InstallArgument = "--install-probe-driver";

    [SupportedOSPlatform("windows")]
    internal static async Task<List<ProbeSummary>> ListProbes()
    {
        var probes = await RustCore.ListProbes();
        return MergeProbes(probes, await Task.Run(ProbeUsb.ListDevices));
    }

    internal static List<ProbeSummary> MergeProbes(IReadOnlyList<ProbeSummary> probes, IReadOnlyList<UsbProbeDevice> devices)
    {
        var result = probes.ToList();
        foreach (var group in devices.GroupBy(device => (device.Vendor, device.Product)))
        {
            // A serial read from a Windows instance ID may differ in case from the
            // USB descriptor. Only synthesize a serial-free selector for ONE device.
            if (group.Any(device => !device.UniqueOnBus)
                || group.Select(device => device.Serial).Distinct(StringComparer.OrdinalIgnoreCase).Count() != 1
                || probes.Any(probe => probe.vendor_id == group.Key.Vendor && probe.product_id == group.Key.Product)) continue;
            foreach (var device in group)
            {
                if (device.Family == ProbeFamily.Glasgow) continue; // Requires an applet-specific IN/OUT pair.
                if (device.Family == ProbeFamily.Ch347 && group.Any(other => InterfaceNumber(other.InstanceId) == 4)
                    && InterfaceNumber(device.InstanceId) != 4) continue;
                var number = device.Family == ProbeFamily.CmsisDap ? InterfaceNumber(device.InstanceId) : null;
                result.Add(new ProbeSummary
                {
                    identifier = $"{device.Vendor:x4}:{device.Product:x4}" + (number is { } value ? $"-{value}" : ""),
                    vendor_id = device.Vendor, product_id = device.Product, serial_number = null,
                    product = device.Family.Title() + "（待检查驱动）",
                });
            }
        }
        return result;
    }

    internal static ProbeSummary? SelectProbe(string? selector, IReadOnlyList<ProbeSummary> probes)
    {
        if (string.IsNullOrWhiteSpace(selector)) return probes.Count == 1 ? probes[0] : null;
        if (ProbeSelector.Parse(selector) is not { } selection) return null;
        var family = ProbeDriverRules.KnownFamily(selection.Vendor, selection.Product);
        var matches = probes.Where(probe => probe.vendor_id == selection.Vendor && probe.product_id == selection.Product
            && (selection.Serial is null || string.Equals(probe.serial_number ?? "", selection.Serial, StringComparison.Ordinal))
            && (selection.Interface is null || family is ProbeFamily.StLink or ProbeFamily.Ftdi
                || (family is ProbeFamily.JLink or ProbeFamily.WchLink) && selection.Interface == 0
                || ProbeSelector.Parse(probe.identifier)?.Interface == selection.Interface)).ToArray();
        if (matches.Length != 1) return null;
        var chosen = matches[0];
        // FTDI interface is a runtime channel selection, absent from list_probes.
        // Pin the serial while retaining that explicitly requested channel.
        if (family == ProbeFamily.Ftdi && selection.Interface is { } number)
            return new ProbeSummary
            {
                identifier = $"{selection.Vendor:x4}:{selection.Product:x4}-{number}"
                    + (chosen.serial_number is { } serial ? $":{serial}" : ""),
                vendor_id = chosen.vendor_id, product_id = chosen.product_id,
                serial_number = chosen.serial_number, product = chosen.product,
            };
        return chosen;
    }

    internal static IReadOnlyList<UsbProbeDevice> SelectDevices(ProbeSummary probe, IReadOnlyList<UsbProbeDevice> devices)
    {
        var selection = ProbeSelector.Parse(probe.identifier);
        var serial = probe.serial_number;
        var glasgow = ProbeDriverRules.KnownFamily(probe.vendor_id, probe.product_id) == ProbeFamily.Glasgow;
        var pair = glasgow ? serial?.Split(':') : null;
        if (glasgow) serial = pair?.FirstOrDefault();
        var matches = devices.Where(device => device.Vendor == probe.vendor_id && device.Product == probe.product_id
            && (string.IsNullOrEmpty(serial) || SerialMatches(device.Serial, serial,
                device.Family == ProbeFamily.StLink))).ToArray();
        if (string.IsNullOrEmpty(serial) && matches.Any(device => !device.UniqueOnBus)) return [];
        if (matches.Select(device => device.Serial).Distinct(StringComparer.OrdinalIgnoreCase).Count() != 1) return [];
        if (glasgow)
        {
            if (pair is not { Length: 3 } || !byte.TryParse(pair[1], out var input) || !byte.TryParse(pair[2], out var output)
                || input == output) return [];
            matches = matches.Where(device => InterfaceNumber(device.InstanceId) is { } number && (number == input || number == output)).ToArray();
            return matches.Length == 2 ? matches : [];
        }
        if (matches.FirstOrDefault()?.Family == ProbeFamily.Ch347)
        {
            var preferred = matches.Any(device => InterfaceNumber(device.InstanceId) == 4) ? 4 : 2;
            matches = matches.Where(device => InterfaceNumber(device.InstanceId) == preferred).ToArray();
        }
        else if (selection?.Interface is { } number && matches.FirstOrDefault()?.Family != ProbeFamily.StLink)
            // Standalone USB devices have no MI_XX child, but their default
            // interface is still selected as channel 0 by probe-rs.
            matches = matches.Where(device => (InterfaceNumber(device.InstanceId) ?? 0) == number).ToArray();
        return matches.Length == 1 ? matches : [];
    }

    internal static UsbProbeDevice? SelectDevice(ProbeSummary probe, IReadOnlyList<UsbProbeDevice> devices) =>
        SelectDevices(probe, devices) is { Count: 1 } selected ? selected[0] : null;

    private static byte? InterfaceNumber(string instanceId) => UsbProbeIdentity.Parse(instanceId)?.Interface;

    [SupportedOSPlatform("windows")]
    internal static async Task<string?> PrepareProbe(ProbeSummary? probe, Action<string> report)
    {
        if (probe is null) return null;
        var before = SelectDevices(probe, await Task.Run(ProbeUsb.ListDevices));
        await EnsureInstalled(probe, report, new WindowsDriverSystem(), before.Select(device => device.InstanceId).ToArray());
        if (before.Count == 0) return probe.identifier;
        var refreshed = SelectProbe(probe.identifier, await RustCore.ListProbes())
            ?? throw new IOException("驱动检查完成，但无法唯一识别所选烧录器。请重新扫描并选择设备。");
        var after = SelectDevices(refreshed, await Task.Run(ProbeUsb.ListDevices));
        if (!before.Select(device => device.InstanceId).Order(StringComparer.OrdinalIgnoreCase)
            .SequenceEqual(after.Select(device => device.InstanceId).Order(StringComparer.OrdinalIgnoreCase), StringComparer.OrdinalIgnoreCase))
            throw new IOException("驱动检查期间所选烧录器已断开或改变。请重新扫描后连接。");
        return refreshed.identifier;
    }

    private static bool SerialMatches(string windowsSerial, string probeSerial, bool encodedStLink = true)
    {
        if (windowsSerial.Equals(probeSerial, StringComparison.OrdinalIgnoreCase)) return true;
        // probe-rs hex-encodes ST-Link serial descriptors shorter than 24 UTF-8 bytes.
        // Windows instance IDs can also change the descriptor's letter case.
        if (!encodedStLink || Encoding.UTF8.GetByteCount(windowsSerial) >= 24) return false;
        try
        {
            var bytes = Convert.FromHexString(probeSerial);
            return bytes.Length < 24 && windowsSerial.Equals(new UTF8Encoding(false, true).GetString(bytes), StringComparison.OrdinalIgnoreCase);
        }
        catch (FormatException) { return false; }
        catch (DecoderFallbackException) { return false; }
    }

    internal static async Task EnsureInstalled(ProbeSummary? probe, Action<string> report, IProbeDriverSystem system,
        IReadOnlyList<string>? expectedInstanceIds = null)
    {
        if (probe is null) return;
        var devices = SelectDevices(probe, await system.ListDevices());
        if (expectedInstanceIds is not null && !expectedInstanceIds.Order(StringComparer.OrdinalIgnoreCase)
            .SequenceEqual(devices.Select(device => device.InstanceId).Order(StringComparer.OrdinalIgnoreCase), StringComparer.OrdinalIgnoreCase))
            throw new IOException("所选烧录器已断开或改变，已停止驱动修复。请重新扫描并选择设备。");
        foreach (var device in devices)
        {
            if (device.Ready) continue;
            var title = device.Family.Title();
            if (!device.UsesWinUsb && !device.NeedsRepair)
                throw new IOException($"{title} 调试接口已绑定 {device.Service}，无法安全自动替换。请在设备管理器中检查所选调试接口的驱动；J-Link 可使用 J-Link Configurator 选择 WinUSB。");
            if (device.NeedsRepair)
            {
                if (!system.NativeArchitecture)
                    throw new PlatformNotSupportedException($"自动安装驱动需要使用 {RuntimeInformation.OSArchitecture} 版 ChipDock，请安装对应版本后重试。");
                if (!device.UsesWinUsb) await system.CheckInboxDriver(device.InstanceId);
                report(device.UsesWinUsb ? $"正在修复 {title} 的 WinUSB 接口；请允许管理员授权…"
                    : $"正在为 {title} 调试接口安装 WinUSB（当前驱动：{(device.Service.Length == 0 ? "未安装" : device.Service)}）；请允许管理员授权…");
                await system.Install(device.InstanceId);
            }
            report($"正在等待 {title} 调试接口就绪…");
            var ready = false;
            for (var attempt = 0; attempt < 20; attempt++)
            {
                var current = (await system.ListDevices()).FirstOrDefault(candidate =>
                    candidate.InstanceId.Equals(device.InstanceId, StringComparison.OrdinalIgnoreCase));
                if (current is { Ready: true }) { ready = true; break; }
                await system.WaitForDevice();
            }
            if (!ready) throw new IOException($"{title} 驱动尚未就绪。请关闭占用设备的调试工具并重新插拔烧录器后再连接。");
        }
    }

    [SupportedOSPlatform("windows")]
    private static async Task InstallElevated(string instanceId)
    {
        // Relaunch only the short-lived driver helper; the workbench stays unelevated.
        var start = new ProcessStartInfo(Path.Combine(AppContext.BaseDirectory, "ChipDock.exe"))
        {
            UseShellExecute = true,
            Verb = "runas",
            WindowStyle = ProcessWindowStyle.Hidden,
        };
        start.ArgumentList.Add(InstallArgument);
        start.ArgumentList.Add(instanceId);
        try
        {
            using var process = Process.Start(start) ?? throw new IOException("无法启动烧录器驱动安装程序。");
            // Do not interrupt an in-flight system driver installation.
            await process.WaitForExitAsync();
            CheckInstallerExitCode(process.ExitCode);
        }
        catch (Win32Exception error) when (error.NativeErrorCode == 1223)
        {
            throw new OperationCanceledException("已取消烧录器驱动安装。再次点击连接可重新安装。");
        }
    }

    internal static void CheckInstallerExitCode(int exitCode)
    {
        if (exitCode == 3010) throw new IOException("烧录器驱动已安装，需要重新启动 Windows 后再连接。");
        if (exitCode != 0)
            throw new IOException($"烧录器驱动安装失败：{new Win32Exception(exitCode).Message}（0x{exitCode:X8}）。请检查设备连接后重试。");
    }

    /// Handled before WinUI, updater hooks, single-instance redirection and Rust startup.
    [SupportedOSPlatform("windows")]
    internal static int RunInstaller(string[] arguments)
    {
        if (arguments.Length != 1 || !ProbeUsb.CanInspect(arguments[0])) return 87;
        if (RuntimeInformation.ProcessArchitecture != RuntimeInformation.OSArchitecture) return 216;
        try { return ProbeUsb.Install(arguments[0]) ? 3010 : 0; }
        catch (Win32Exception error) { return error.NativeErrorCode; }
        catch (UnauthorizedAccessException) { return 5; }
        catch (Exception) { return 1; }
    }

    [SupportedOSPlatform("windows")]
    private sealed class WindowsDriverSystem : IProbeDriverSystem
    {
        public bool NativeArchitecture => RuntimeInformation.ProcessArchitecture == RuntimeInformation.OSArchitecture;
        public Task<IReadOnlyList<UsbProbeDevice>> ListDevices() => Task.Run(ProbeUsb.ListDevices);
        public Task CheckInboxDriver(string instanceId) => Task.Run(() =>
        {
            try { ProbeUsb.CheckInboxDriver(instanceId); }
            catch (Win32Exception error)
            {
                throw new IOException($"无法自动选择系统 WinUSB 驱动：{error.Message}（0x{error.NativeErrorCode:X8}）。J-Link 可在 J-Link Configurator 中切换为 WinUSB；其他 USB 调试接口可通过 Zadig 绑定 WinUSB。请保留串口、HID 和存储接口的原驱动。", error);
            }
        });
        public Task Install(string instanceId) => InstallElevated(instanceId);
        public Task WaitForDevice() => Task.Delay(250);
    }
}

internal interface IProbeDriverSystem
{
    bool NativeArchitecture { get; }
    Task<IReadOnlyList<UsbProbeDevice>> ListDevices();
    Task CheckInboxDriver(string instanceId);
    Task Install(string instanceId);
    Task WaitForDevice();
}
