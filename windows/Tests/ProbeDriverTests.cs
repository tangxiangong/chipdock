using System.Runtime.Versioning;
using FlashDesk.Core;

namespace FlashDesk.Tests;

internal static class ProbeDriverTests
{
    [SupportedOSPlatform("windows")]
    internal static async Task CheckProbeSelectors()
    {
        var devices = ProbeUsb.ListDevices();
        foreach (var probe in (await RustCore.ListProbes()).Where(probe => ProbeDriverRules.IsStLink(probe.vendor_id, probe.product_id)))
            Check.That(ProbeDriver.SelectDevice(probe, devices) is not null, "native debug interface matches the real probe-rs selector and serial");
    }

    internal static async Task RunSimulation()
    {
        const string debugId = @"USB\VID_0483&PID_374B&MI_00\7&123&0&0000";
        Check.That(IsDebugInterface(debugId), "V2-1 debug interface can be repaired");
        Check.That(IsDebugInterface(@"USB\VID_0483&PID_3748\SERIAL"), "standalone V2 can be repaired");
        Check.That(IsDebugInterface(@"usb\vid_0483&pid_3757&mi_00\serial"), "V3PWR debug ID is case insensitive");
        foreach (var id in new[] {
            debugId.Replace("MI_00", "MI_01"), debugId.Replace("MI_00", "MI_02"),
            debugId.Replace("&MI_00", ""), debugId.Replace("VID_0483", "VID_1234"),
            debugId.Replace("PID_374B", "PID_0001"), @"ROOT\VID_0483&PID_3748\SERIAL", "", debugId + @"\extra",
        }) Check.That(!IsDebugInterface(id), "unrelated device, composite parent, storage and serial are excluded");

        var probe = new ProbeSummary { identifier = "0483:374b:SERIAL", vendor_id = 0x0483, product_id = 0x374b, serial_number = "SERIAL", product = "STLink V2-1" };
        var device = new UsbProbeDevice(debugId, 0x0483, 0x374b, "SERIAL", "", false, false, false);
        var other = device with { InstanceId = debugId + "OTHER", Serial = "OTHER" };
        Check.That(device.NeedsRepair && !device.Ready, "missing driver triggers repair");
        Check.That(!(device with { Service = "USBSTOR" }).NeedsRepair, "existing other driver is preserved");
        Check.That(!(device with { Service = "WinUSB", HasInterfaceGuid = true }).NeedsRepair, "working WinUSB is left alone");
        Check.That((device with { Service = "WINUSB" }).NeedsRepair, "missing WinUSB interface GUID is repairable");
        var ready = device with { Service = "WinUSB", HasInterfaceGuid = true, Started = true, InterfaceActive = true };
        Check.That(ready.Ready && !ready.NeedsRepair, "active and started WinUSB interface is ready");
        Check.That(!(ready with { Started = false }).Ready, "registry configuration alone does not mean driver is started");
        Check.That(!(ready with { InterfaceActive = false }).Ready, "driver must publish an active device interface");
        Check.That(ProbeDriver.SelectDevice(probe, [other, device]) == device, "repair selects the exact probe serial");
        Check.That(ProbeDriver.SelectDevice(probe, [other]) is null, "another probe never substitutes for selected serial");
        probe.serial_number = null;
        Check.That(ProbeDriver.SelectDevice(probe, [other, device]) is null, "ambiguous probes are never repaired automatically");
        Check.That(ProbeDriver.SelectProbe(null, [probe]) == probe, "one probe is selected automatically");
        Check.That(ProbeDriver.SelectProbe(null, [probe, probe]) is null, "multiple probes require selection");
        Check.That(ProbeDriver.SelectProbe("other", [probe]) is null, "stale selection is not replaced");
        Check.That(ProbeDriver.SelectProbe(" 0483:374B ", [probe]) == probe, "profile may select a unique VID:PID without serial");
        Check.That(ProbeDriver.SelectProbe("0483:374b-0", [probe]) == probe, "explicit debug interface is accepted");
        Check.That(ProbeDriver.SelectProbe("0483:374b-", [probe]) == probe, "empty interface syntax agrees with probe-rs");
        Check.That(ProbeDriver.SelectProbe("0483:374b", [probe, probe]) is null, "VID:PID profile cannot auto-repair ambiguous probes");
        Check.That(ProbeDriver.SelectProbe("0483:374b-256", [probe]) is null, "invalid interface is not repaired");
        Check.That(ProbeDriver.SelectProbe("0483:374b:", [probe]) == probe, "empty selector serial matches a probe without serial as in probe-rs");
        probe.serial_number = "SERIAL";
        Check.That(ProbeDriver.SelectProbe("0483:374b:OTHER", [probe]) is null, "missing profile serial never selects a replacement");
        Check.That(ProbeDriver.SelectProbe("0483:374b:serial", [probe]) is null, "selector serial matching agrees with probe-rs case sensitivity");
        var dap = new ProbeSummary { identifier = "c251:f001-1:SERIAL", vendor_id = 0xc251, product_id = 0xf001, serial_number = "SERIAL", product = "CMSIS-DAP" };
        Check.That(ProbeDriver.SelectProbe("c251:f001-2:SERIAL", [dap]) is null, "ST-Link repair never rewrites another probe's interface selection");

        const string shortSerial = "123456789ABC";
        var legacySerial = Convert.ToHexString(System.Text.Encoding.UTF8.GetBytes(shortSerial));
        var legacyProbe = new ProbeSummary { identifier = $"0483:3748:{legacySerial}", vendor_id = 0x0483, product_id = 0x3748, serial_number = legacySerial, product = "STLink V2" };
        var standalone = device with { InstanceId = @"USB\VID_0483&PID_3748\123456789ABC", Product = 0x3748, Serial = shortSerial };
        Check.That(ProbeDriver.SelectDevice(legacyProbe, [standalone]) == standalone, "ordinary standalone V2 short serial matches probe-rs hex encoding");
        legacyProbe.serial_number = Convert.ToHexString(System.Text.Encoding.UTF8.GetBytes(shortSerial.ToLowerInvariant()));
        Check.That(ProbeDriver.SelectDevice(legacyProbe, [standalone]) == standalone, "Windows serial casing does not break encoded descriptor matching");
        var nonAscii = standalone with { Serial = new string('\u00e9', 12) };
        legacyProbe.serial_number = Convert.ToHexString(System.Text.Encoding.UTF8.GetBytes(nonAscii.Serial));
        Check.That(ProbeDriver.SelectDevice(legacyProbe, [nonAscii]) is null, "encoding threshold uses UTF-8 bytes rather than UTF-16 character count");
        legacyProbe.serial_number = legacySerial;
        var standaloneReady = standalone with { Service = "WinUSB", Started = true, InterfaceActive = true };
        Check.That(standaloneReady.Ready && !standaloneReady.NeedsRepair, "standalone V2 needs no composite interface GUID to work with nusb");

        GuidChecks();
        await InstallationChecks(probe, device, ready, legacyProbe, standaloneReady);
        await OtherProbeChecks();
    }

    private static bool IsDebugInterface(string instanceId) => ProbeDriverRules.DebugInterface(instanceId, [], "") is not null;

    [SupportedOSPlatform("windows")]
    internal static void RunNative()
    {
        const string debugId = @"USB\VID_0483&PID_374B&MI_00\7&123&0&0000";
        Check.That(ProbeDriver.RunInstaller([]) == 87, "helper rejects missing arguments before touching devices");
        Check.That(ProbeDriver.RunInstaller([debugId, "extra"]) == 87, "helper rejects extra arguments");
        Check.That(ProbeDriver.RunInstaller([debugId.Replace("MI_00", "MI_02")]) == 87, "elevated helper cannot target the serial interface");

        // Read-only integration check exercises the native ABI on both x64 and ARM64 CI.
        var devices = ProbeUsb.ListDevices();
        Check.That(devices.All(candidate => ProbeUsb.CanInspect(candidate.InstanceId)), "native discovery only returns supported debug interfaces");
        foreach (var candidate in devices)
        {
            Console.WriteLine($"{candidate.Family.Title()} {candidate.Vendor:X4}:{candidate.Product:X4}: service={candidate.Service}, interface GUID={candidate.HasInterfaceGuid}, started={candidate.Started}, active={candidate.InterfaceActive}");
            if (candidate.Service.Length == 0)
            {
                ProbeUsb.CheckInboxDriver(candidate.InstanceId);
                Check.That(true, "inbox WinUSB driver resolves for the connected driverless interface without installation");
            }
            if (candidate.IsComposite)
            {
                var parent = $@"USB\VID_{candidate.Vendor:X4}&PID_{candidate.Product:X4}\{candidate.Serial}";
                Check.That(ProbeUsb.HasActiveInterface(parent, Guid.Parse("A5DCBF10-6530-11D2-901F-00C04FB951ED")),
                    "native interface discovery sees the active composite USB parent");
            }
        }
    }

    private static void GuidChecks()
    {
        const string first = "{11A32CF6-A5A1-4C15-A3C2-778A529F6447}";
        const string second = "{22A32CF6-A5A1-4C15-A3C2-778A529F6447}";
        Check.That(ProbeDriverRules.RegisteredGuid(new[] { first, second }, null) == Guid.Parse(first), "nusb uses only the first plural GUID");
        Check.That(ProbeDriverRules.RegisteredGuid(new[] { "invalid", second }, null) is null, "valid later GUID cannot hide a broken first entry");
        Check.That(ProbeDriverRules.RegisteredGuid(new[] { "invalid", second }, first) == Guid.Parse(first), "singular GUID is the nusb fallback");
        Check.That(ProbeDriverRules.RegisteredGuid(first, null) == Guid.Parse(first), "REG_SZ plural GUID is supported");
        Check.That(ProbeDriverRules.RegisteredGuid(null, new[] { first }) == Guid.Parse(first), "REG_MULTI_SZ singular GUID is supported");
        Check.That(ProbeDriverRules.RegisteredGuid(new byte[] { 1, 2 }, null) is null, "wrong registry value type requires repair");
        var repaired = ProbeDriverRules.RepairGuids(new[] { "invalid", first, Guid.Empty.ToString("B") }, second);
        Check.That(repaired.Length == 3 && repaired.All(value => Guid.TryParseExact(value, "B", out _)), "repair removes invalid entries and preserves existing valid GUIDs");
        Check.That(ProbeDriverRules.RegisteredGuid(repaired, null) == Guid.Parse("DBCE1CD9-A320-4B51-A365-A0C3F3C5FB29"), "ST debug GUID is first after repair");
        Check.That(ProbeDriverRules.RepairGuids(repaired, first).SequenceEqual(repaired), "GUID repair is idempotent");
    }

    private static async Task OtherProbeChecks()
    {
        var standaloneFtdi = new UsbProbeDevice(@"USB\VID_0403&PID_6014\SERIAL", 0x0403, 0x6014,
            "SERIAL", "FTDIBUS", false, false, false, ProbeFamily.Ftdi);
        var ftdiProbe = new ProbeSummary { identifier = "0403:6014:SERIAL", vendor_id = 0x0403,
            product_id = 0x6014, serial_number = "SERIAL", product = "FTDI" };
        var channelZero = ProbeDriver.SelectProbe("0403:6014-0", [ftdiProbe])!;
        var ftdiSystem = new FakeSystem([standaloneFtdi],
            [standaloneFtdi with { Service = "WinUSB", Started = true, InterfaceActive = true }]);
        await ProbeDriver.EnsureInstalled(channelZero, _ => { }, ftdiSystem);
        Check.That(ftdiSystem.Installed.SequenceEqual([standaloneFtdi.InstanceId]),
            "explicit channel zero repairs a standalone FTDI device without MI_00");
        var channelOne = ProbeDriver.SelectProbe("0403:6014-1", [ftdiProbe])!;
        Check.That(ProbeDriver.SelectDevices(channelOne, [standaloneFtdi]).Count == 0,
            "nonzero channel never falls back to a standalone device's default interface");

        foreach (var (family, vendor, product, instance, service) in new (ProbeFamily, ushort, ushort, string, string)[]
        {
            (ProbeFamily.JLink, 0x1366, 0x0101, @"USB\VID_1366&PID_0101\SERIAL", "JLink"),
            (ProbeFamily.Ftdi, 0x0403, 0x6010, @"USB\VID_0403&PID_6010&MI_00\CHILD", "FTDIBUS"),
            (ProbeFamily.WchLink, 0x1a86, 0x8010, @"USB\VID_1A86&PID_8010&MI_00\CHILD", "WCHLink"),
            (ProbeFamily.Ch347, 0x1a86, 0x55de, @"USB\VID_1A86&PID_55DE&MI_04\CHILD", "CH347"),
            (ProbeFamily.CmsisDap, 0xc251, 0xf001, @"USB\VID_C251&PID_F001&MI_02\CHILD", ""),
        })
        {
            var probe = new ProbeSummary { identifier = $"{vendor:x4}:{product:x4}:SERIAL", vendor_id = vendor, product_id = product, serial_number = "SERIAL", product = family.Title() };
            var missing = new UsbProbeDevice(instance, vendor, product, "SERIAL", service, false, false, false, family);
            var ready = missing with { Service = "WinUSB", HasInterfaceGuid = true, Started = true, InterfaceActive = true };
            var system = new FakeSystem([missing], [ready]);
            await ProbeDriver.EnsureInstalled(probe, _ => { }, system);
            Check.That(system.Checked == 1 && system.Installed.SequenceEqual([instance]), $"{family} installs inbox WinUSB on the selected debug interface");

            system = new FakeSystem([ready]);
            await ProbeDriver.EnsureInstalled(probe, _ => { }, system);
            Check.That(system.Checked == 0 && system.Installed.Count == 0, $"{family} leaves a working driver alone");

            var fallback = ProbeDriver.MergeProbes([], [missing]);
            Check.That(fallback.Count == 1 && fallback[0].serial_number is null, $"{family} missing driver is visible without guessing serial casing");
            Check.That(ProbeDriver.MergeProbes([], [missing with { UniqueOnBus = false }]).Count == 0,
                $"{family} accounts for physical siblings without repairable interfaces");
            Check.That(ProbeDriver.SelectDevices(fallback[0], [missing with { UniqueOnBus = false }]).Count == 0,
                $"{family} never binds a serial-free selection after another same-model device appears");
            var replaced = new FakeSystem([missing with { InstanceId = instance + "OTHER", Serial = "OTHER" }]);
            await Check.ThrowsAsync<IOException>(() => ProbeDriver.EnsureInstalled(fallback[0], _ => { }, replaced, [instance]),
                $"{family} stops when a different device replaces a serial-free selection", "停止驱动修复");
            Check.That(replaced.Installed.Count == 0 && replaced.Checked == 0, $"{family} replacement is rejected before preflight or UAC");
            Check.That(ProbeDriver.MergeProbes([], [missing, missing with { InstanceId = instance + "OTHER", Serial = "OTHER" }]).Count == 0,
                $"{family} does not synthesize ambiguous serial-free selectors");
            Check.That(ProbeDriver.MergeProbes([probe], [missing]).Count == 1, $"{family} fallback does not duplicate core discovery");
            if (family is ProbeFamily.JLink or ProbeFamily.WchLink)
                Check.That(ProbeDriver.SelectProbe($"{vendor:x4}:{product:x4}-0", [probe]) == probe, "explicit default interface matches a standalone or composite debug probe");

            if (family == ProbeFamily.Ftdi)
            {
                var chosen = ProbeDriver.SelectProbe("0403:6010-1", [probe]);
                Check.That(chosen?.identifier == "0403:6010-1:SERIAL", "explicit FTDI channel is retained while pinning the serial");
                Check.That(ProbeDriver.SelectDevices(chosen!, [ready]).Count == 0, "an explicit channel B never repairs channel A");
            }
        }

        var ch347 = new ProbeSummary { identifier = "1a86:55de:SERIAL", vendor_id = 0x1a86, product_id = 0x55de, serial_number = "SERIAL", product = "CH347" };
        var channel4 = new UsbProbeDevice(@"USB\VID_1A86&PID_55DE&MI_04\FOUR", 0x1a86, 0x55de, "SERIAL", "", false, false, false, ProbeFamily.Ch347);
        var channel2 = channel4 with { InstanceId = @"USB\VID_1A86&PID_55DE&MI_02\TWO" };
        Check.That(ProbeDriver.SelectDevice(ch347, [channel2, channel4]) == channel4, "CH347F interface 4 wins over the CH347T fallback");
        Check.That(ProbeDriver.MergeProbes([], [channel2, channel4]).Count == 1, "CH347 fallback discovery creates one probe per device");

        var glasgow = new ProbeSummary { identifier = "20b7:9db1:SERIAL:1:2", vendor_id = 0x20b7, product_id = 0x9db1, serial_number = "SERIAL:1:2", product = "Glasgow" };
        var input = new UsbProbeDevice(@"USB\VID_20B7&PID_9DB1&MI_01\IN", 0x20b7, 0x9db1, "SERIAL", "WinUSB", false, true, false, ProbeFamily.Glasgow);
        var output = input with { InstanceId = @"USB\VID_20B7&PID_9DB1&MI_02\OUT" };
        var inputReady = input with { HasInterfaceGuid = true, InterfaceActive = true };
        var outputReady = output with { HasInterfaceGuid = true, InterfaceActive = true };
        var pairSystem = new FakeSystem([input, output], [inputReady, output], [inputReady, outputReady]);
        await ProbeDriver.EnsureInstalled(glasgow, _ => { }, pairSystem);
        Check.That(pairSystem.Installed.SequenceEqual([input.InstanceId, output.InstanceId]), "Glasgow repairs only the selected applet's IN and OUT interfaces");
        Check.That(ProbeDriver.MergeProbes([], [input, output]).Count == 0, "Glasgow applet identity is never invented when core discovery fails");
    }

    private static async Task InstallationChecks(ProbeSummary probe, UsbProbeDevice missing, UsbProbeDevice ready,
        ProbeSummary legacyProbe, UsbProbeDevice standaloneReady)
    {
        var reports = new List<string>();
        var system = new FakeSystem([missing], [ready with { Started = false, InterfaceActive = false }],
            [ready with { InterfaceActive = false }], [ready]);
        await ProbeDriver.EnsureInstalled(probe, reports.Add, system);
        Check.That(system.Installed.SequenceEqual([missing.InstanceId]) && system.Checked == 1, "missing driver is preflighted and installed exactly once");
        Check.That(system.Waits == 2, "connection waits for driver start and active interface publication");

        system = new FakeSystem([ready]);
        await ProbeDriver.EnsureInstalled(probe, reports.Add, system);
        Check.That(system.Installed.Count == 0 && system.Waits == 0, "working drivers never trigger UAC");

        system = new FakeSystem([standaloneReady]);
        await ProbeDriver.EnsureInstalled(legacyProbe, reports.Add, system);
        Check.That(system.Installed.Count == 0 && system.Waits == 0, "working standalone V2 without a GUID never triggers UAC");

        system = new FakeSystem([ready with { Service = "STTub" }], [ready]);
        await ProbeDriver.EnsureInstalled(probe, reports.Add, system);
        Check.That(system.Installed.Count == 1 && system.Checked == 1, "legacy ST-Link vendor driver migrates to inbox WinUSB");

        system = new FakeSystem([ready with { Service = "USBSTOR" }]);
        await Check.ThrowsAsync<IOException>(() => ProbeDriver.EnsureInstalled(probe, reports.Add, system),
            "unexpected driver reports recovery instead of installing", "无法安全自动替换");
        Check.That(system.Installed.Count == 0, "unexpected service is never replaced");

        system = new FakeSystem([ready with { InterfaceActive = false }], [ready]);
        await ProbeDriver.EnsureInstalled(probe, reports.Add, system);
        Check.That(system.Installed.Count == 0 && system.Scans == 2, "installed driver still starting is awaited without reinstalling");

        system = new FakeSystem([ready with { HasInterfaceGuid = false }], [ready]);
        await ProbeDriver.EnsureInstalled(probe, reports.Add, system);
        Check.That(system.Installed.Count == 1 && system.Checked == 0, "broken composite GUID is repaired without rebinding the existing driver");

        system = new FakeSystem([missing]) { InstallError = new OperationCanceledException("UAC cancelled") };
        await Check.ThrowsAsync<OperationCanceledException>(() => ProbeDriver.EnsureInstalled(probe, reports.Add, system), "UAC cancellation is returned to the dialog");
        Check.That(system.Installed.Count == 1 && system.Scans == 1, "cancelled installation is not retried or reported ready");

        system = new FakeSystem([missing]) { CheckError = new IOException("driver missing") };
        await Check.ThrowsAsync<IOException>(() => ProbeDriver.EnsureInstalled(probe, reports.Add, system), "preflight failure stops installation");
        Check.That(system.Installed.Count == 0, "preflight failure never prompts UAC");

        system = new FakeSystem([missing]) { NativeArchitecture = false };
        await Check.ThrowsAsync<PlatformNotSupportedException>(() => ProbeDriver.EnsureInstalled(probe, reports.Add, system), "emulated app explains native architecture requirement");
        Check.That(system.Installed.Count == 0, "wrong architecture is rejected before UAC");

        system = new FakeSystem([missing], []);
        await Check.ThrowsAsync<IOException>(() => ProbeDriver.EnsureInstalled(probe, reports.Add, system), "unplugged probe stops connection", "重新插拔");
        Check.That(system.Installed.Count == 1 && system.Waits == 20, "disconnected device has a bounded wait and no repeated install");

        system = new FakeSystem([missing], [ready with { InstanceId = ready.InstanceId + "OTHER" }]);
        await Check.ThrowsAsync<IOException>(() => ProbeDriver.EnsureInstalled(probe, reports.Add, system), "a different ready probe does not satisfy the installation wait");

        ProbeDriver.CheckInstallerExitCode(0);
        Check.Throws<IOException>(() => ProbeDriver.CheckInstallerExitCode(3010), "reboot-required helper result stops connection", "重新启动");
        Check.Throws<IOException>(() => ProbeDriver.CheckInstallerExitCode(5), "failed helper result stops connection", "安装失败");
        system = new FakeSystem([missing]) { InstallError = new IOException("需要重新启动 Windows") };
        await Check.ThrowsAsync<IOException>(() => ProbeDriver.EnsureInstalled(probe, reports.Add, system), "helper failure is propagated");
        Check.That(system.Scans == 1 && system.Waits == 0, "helper failure never continues to connection readiness");
    }

    private sealed class FakeSystem(params UsbProbeDevice[][] snapshots) : IProbeDriverSystem
    {
        public bool NativeArchitecture { get; init; } = true;
        internal Exception? CheckError { get; init; }
        internal Exception? InstallError { get; init; }
        internal int Checked, Scans, Waits;
        internal List<string> Installed { get; } = [];
        public Task<IReadOnlyList<UsbProbeDevice>> ListDevices() =>
            Task.FromResult<IReadOnlyList<UsbProbeDevice>>(snapshots[Math.Min(Scans++, snapshots.Length - 1)]);
        public Task CheckInboxDriver(string instanceId)
        {
            Checked++;
            return CheckError is null ? Task.CompletedTask : Task.FromException(CheckError);
        }
        public Task Install(string instanceId)
        {
            Installed.Add(instanceId);
            return InstallError is null ? Task.CompletedTask : Task.FromException(InstallError);
        }
        public Task WaitForDevice() { Waits++; return Task.CompletedTask; }
    }
}
