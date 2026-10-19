using System.Runtime.Versioning;
using FlashDesk.Core;

namespace FlashDesk.Tests;

[SupportedOSPlatform("windows")]
internal static class StLinkDriverTests
{
    internal static async Task CheckProbeSelectors()
    {
        var devices = StLinkUsb.ListDevices();
        foreach (var probe in (await RustCore.ListProbes()).Where(probe => StLinkDriver.IsSupported(probe.vendor_id, probe.product_id)))
            Check.That(StLinkDriver.SelectDevice(probe, devices) is not null, "native debug interface matches the real probe-rs selector and serial");
    }

    internal static async Task Run()
    {
        const string debugId = @"USB\VID_0483&PID_374B&MI_00\7&123&0&0000";
        Check.That(StLinkUsb.IsDebugInterface(debugId), "V2-1 debug interface can be repaired");
        Check.That(StLinkUsb.IsDebugInterface(@"USB\VID_0483&PID_3748\SERIAL"), "standalone V2 can be repaired");
        Check.That(StLinkUsb.IsDebugInterface(@"usb\vid_0483&pid_3757&mi_00\serial"), "V3PWR debug ID is case insensitive");
        foreach (var id in new[] {
            debugId.Replace("MI_00", "MI_01"), debugId.Replace("MI_00", "MI_02"),
            debugId.Replace("&MI_00", ""), debugId.Replace("VID_0483", "VID_1234"),
            debugId.Replace("PID_374B", "PID_0001"), @"ROOT\VID_0483&PID_3748\SERIAL", "", debugId + @"\extra",
        }) Check.That(!StLinkUsb.IsDebugInterface(id), "unrelated device, composite parent, storage and serial are excluded");

        var probe = new ProbeSummary { identifier = "0483:374b:SERIAL", vendor_id = 0x0483, product_id = 0x374b, serial_number = "SERIAL", product = "STLink V2-1" };
        var device = new StLinkUsb.Device(debugId, 0x0483, 0x374b, "SERIAL", "", false, false, false);
        var other = device with { InstanceId = debugId + "OTHER", Serial = "OTHER" };
        Check.That(device.NeedsRepair && !device.Ready, "missing driver triggers repair");
        Check.That(!(device with { Service = "USBSTOR" }).NeedsRepair, "existing other driver is preserved");
        Check.That(!(device with { Service = "WinUSB", HasInterfaceGuid = true }).NeedsRepair, "working WinUSB is left alone");
        Check.That((device with { Service = "WINUSB" }).NeedsRepair, "missing WinUSB interface GUID is repairable");
        var ready = device with { Service = "WinUSB", HasInterfaceGuid = true, Started = true, InterfaceActive = true };
        Check.That(ready.Ready && !ready.NeedsRepair, "active and started WinUSB interface is ready");
        Check.That(!(ready with { Started = false }).Ready, "registry configuration alone does not mean driver is started");
        Check.That(!(ready with { InterfaceActive = false }).Ready, "driver must publish an active device interface");
        Check.That(StLinkDriver.SelectDevice(probe, [other, device]) == device, "repair selects the exact probe serial");
        Check.That(StLinkDriver.SelectDevice(probe, [other]) is null, "another probe never substitutes for selected serial");
        probe.serial_number = null;
        Check.That(StLinkDriver.SelectDevice(probe, [other, device]) is null, "ambiguous probes are never repaired automatically");
        Check.That(StLinkDriver.SelectProbe(null, [probe]) == probe, "one probe is selected automatically");
        Check.That(StLinkDriver.SelectProbe(null, [probe, probe]) is null, "multiple probes require selection");
        Check.That(StLinkDriver.SelectProbe("other", [probe]) is null, "stale selection is not replaced");
        Check.That(StLinkDriver.SelectProbe(" 0483:374B ", [probe]) == probe, "profile may select a unique VID:PID without serial");
        Check.That(StLinkDriver.SelectProbe("0483:374b-0", [probe]) == probe, "explicit debug interface is accepted");
        Check.That(StLinkDriver.SelectProbe("0483:374b-", [probe]) == probe, "empty interface syntax agrees with probe-rs");
        Check.That(StLinkDriver.SelectProbe("0483:374b", [probe, probe]) is null, "VID:PID profile cannot auto-repair ambiguous probes");
        Check.That(StLinkDriver.SelectProbe("0483:374b-256", [probe]) is null, "invalid interface is not repaired");
        Check.That(StLinkDriver.SelectProbe("0483:374b:", [probe]) == probe, "empty selector serial matches a probe without serial as in probe-rs");
        probe.serial_number = "SERIAL";
        Check.That(StLinkDriver.SelectProbe("0483:374b:OTHER", [probe]) is null, "missing profile serial never selects a replacement");
        Check.That(StLinkDriver.SelectProbe("0483:374b:serial", [probe]) is null, "selector serial matching agrees with probe-rs case sensitivity");
        var dap = new ProbeSummary { identifier = "c251:f001-1:SERIAL", vendor_id = 0xc251, product_id = 0xf001, serial_number = "SERIAL", product = "CMSIS-DAP" };
        Check.That(StLinkDriver.SelectProbe("c251:f001-2:SERIAL", [dap]) is null, "ST-Link repair never rewrites another probe's interface selection");

        const string shortSerial = "123456789ABC";
        var legacySerial = Convert.ToHexString(System.Text.Encoding.UTF8.GetBytes(shortSerial));
        var legacyProbe = new ProbeSummary { identifier = $"0483:3748:{legacySerial}", vendor_id = 0x0483, product_id = 0x3748, serial_number = legacySerial, product = "STLink V2" };
        var standalone = device with { InstanceId = @"USB\VID_0483&PID_3748\123456789ABC", Product = 0x3748, Serial = shortSerial };
        Check.That(StLinkDriver.SelectDevice(legacyProbe, [standalone]) == standalone, "ordinary standalone V2 short serial matches probe-rs hex encoding");
        legacyProbe.serial_number = Convert.ToHexString(System.Text.Encoding.UTF8.GetBytes(shortSerial.ToLowerInvariant()));
        Check.That(StLinkDriver.SelectDevice(legacyProbe, [standalone]) == standalone, "Windows serial casing does not break encoded descriptor matching");
        var nonAscii = standalone with { Serial = new string('\u00e9', 12) };
        legacyProbe.serial_number = Convert.ToHexString(System.Text.Encoding.UTF8.GetBytes(nonAscii.Serial));
        Check.That(StLinkDriver.SelectDevice(legacyProbe, [nonAscii]) is null, "encoding threshold uses UTF-8 bytes rather than UTF-16 character count");
        legacyProbe.serial_number = legacySerial;
        var standaloneReady = standalone with { Service = "WinUSB", Started = true, InterfaceActive = true };
        Check.That(standaloneReady.Ready && !standaloneReady.NeedsRepair, "standalone V2 needs no composite interface GUID to work with nusb");

        GuidChecks();
        await InstallationChecks(probe, device, ready, legacyProbe, standaloneReady);

        Check.That(StLinkDriver.RunInstaller([]) == 87, "helper rejects missing arguments before touching devices");
        Check.That(StLinkDriver.RunInstaller([debugId, "extra"]) == 87, "helper rejects extra arguments");
        Check.That(StLinkDriver.RunInstaller([debugId.Replace("MI_00", "MI_02")]) == 87, "elevated helper cannot target the serial interface");

        // Read-only integration check exercises the native ABI on both x64 and ARM64 CI.
        var devices = StLinkUsb.ListDevices();
        Check.That(devices.All(candidate => StLinkUsb.IsDebugInterface(candidate.InstanceId)), "native discovery only returns supported debug interfaces");
        foreach (var candidate in devices)
        {
            Console.WriteLine($"ST-Link {candidate.Product:X4}: service={candidate.Service}, interface GUID={candidate.HasInterfaceGuid}, started={candidate.Started}, active={candidate.InterfaceActive}");
            if (candidate.Service.Length == 0)
            {
                StLinkUsb.CheckInboxDriver(candidate.InstanceId);
                Check.That(true, "inbox WinUSB driver resolves for the connected driverless interface without installation");
            }
            if (candidate.IsComposite)
            {
                var parent = $@"USB\VID_0483&PID_{candidate.Product:X4}\{candidate.Serial}";
                Check.That(StLinkUsb.HasActiveInterface(parent, Guid.Parse("A5DCBF10-6530-11D2-901F-00C04FB951ED")),
                    "native interface discovery sees the active composite USB parent");
            }
        }
    }

    private static void GuidChecks()
    {
        const string first = "{11A32CF6-A5A1-4C15-A3C2-778A529F6447}";
        const string second = "{22A32CF6-A5A1-4C15-A3C2-778A529F6447}";
        Check.That(StLinkUsb.RegisteredGuid(new[] { first, second }, null) == Guid.Parse(first), "nusb uses only the first plural GUID");
        Check.That(StLinkUsb.RegisteredGuid(new[] { "invalid", second }, null) is null, "valid later GUID cannot hide a broken first entry");
        Check.That(StLinkUsb.RegisteredGuid(new[] { "invalid", second }, first) == Guid.Parse(first), "singular GUID is the nusb fallback");
        Check.That(StLinkUsb.RegisteredGuid(first, null) == Guid.Parse(first), "REG_SZ plural GUID is supported");
        Check.That(StLinkUsb.RegisteredGuid(null, new[] { first }) == Guid.Parse(first), "REG_MULTI_SZ singular GUID is supported");
        Check.That(StLinkUsb.RegisteredGuid(new byte[] { 1, 2 }, null) is null, "wrong registry value type requires repair");
        var repaired = StLinkUsb.RepairGuids(new[] { "invalid", first, Guid.Empty.ToString("B") }, second);
        Check.That(repaired.Length == 3 && repaired.All(value => Guid.TryParseExact(value, "B", out _)), "repair removes invalid entries and preserves existing valid GUIDs");
        Check.That(StLinkUsb.RegisteredGuid(repaired, null) == Guid.Parse("DBCE1CD9-A320-4B51-A365-A0C3F3C5FB29"), "ST debug GUID is first after repair");
        Check.That(StLinkUsb.RepairGuids(repaired, first).SequenceEqual(repaired), "GUID repair is idempotent");
    }

    private static async Task InstallationChecks(ProbeSummary probe, StLinkUsb.Device missing, StLinkUsb.Device ready,
        ProbeSummary legacyProbe, StLinkUsb.Device standaloneReady)
    {
        var reports = new List<string>();
        var system = new FakeSystem([missing], [ready with { Started = false, InterfaceActive = false }],
            [ready with { InterfaceActive = false }], [ready]);
        await StLinkDriver.EnsureInstalled(probe, reports.Add, system);
        Check.That(system.Installed.SequenceEqual([missing.InstanceId]) && system.Checked == 1, "missing driver is preflighted and installed exactly once");
        Check.That(system.Waits == 2, "connection waits for driver start and active interface publication");

        system = new FakeSystem([ready]);
        await StLinkDriver.EnsureInstalled(probe, reports.Add, system);
        Check.That(system.Installed.Count == 0 && system.Waits == 0, "working drivers never trigger UAC");

        system = new FakeSystem([standaloneReady]);
        await StLinkDriver.EnsureInstalled(legacyProbe, reports.Add, system);
        Check.That(system.Installed.Count == 0 && system.Waits == 0, "working standalone V2 without a GUID never triggers UAC");

        system = new FakeSystem([ready with { Service = "STTub" }]);
        await StLinkDriver.EnsureInstalled(probe, reports.Add, system);
        Check.That(system.Installed.Count == 0, "automatic repair does not replace an existing vendor driver");

        system = new FakeSystem([ready with { InterfaceActive = false }], [ready]);
        await StLinkDriver.EnsureInstalled(probe, reports.Add, system);
        Check.That(system.Installed.Count == 0 && system.Scans == 2, "installed driver still starting is awaited without reinstalling");

        system = new FakeSystem([ready with { HasInterfaceGuid = false }], [ready]);
        await StLinkDriver.EnsureInstalled(probe, reports.Add, system);
        Check.That(system.Installed.Count == 1 && system.Checked == 0, "broken composite GUID is repaired without rebinding the existing driver");

        system = new FakeSystem([missing]) { InstallError = new OperationCanceledException("UAC cancelled") };
        await Check.ThrowsAsync<OperationCanceledException>(() => StLinkDriver.EnsureInstalled(probe, reports.Add, system), "UAC cancellation is returned to the dialog");
        Check.That(system.Installed.Count == 1 && system.Scans == 1, "cancelled installation is not retried or reported ready");

        system = new FakeSystem([missing]) { CheckError = new IOException("driver missing") };
        await Check.ThrowsAsync<IOException>(() => StLinkDriver.EnsureInstalled(probe, reports.Add, system), "preflight failure stops installation");
        Check.That(system.Installed.Count == 0, "preflight failure never prompts UAC");

        system = new FakeSystem([missing]) { NativeArchitecture = false };
        await Check.ThrowsAsync<PlatformNotSupportedException>(() => StLinkDriver.EnsureInstalled(probe, reports.Add, system), "emulated app explains native architecture requirement");
        Check.That(system.Installed.Count == 0, "wrong architecture is rejected before UAC");

        system = new FakeSystem([missing], []);
        await Check.ThrowsAsync<IOException>(() => StLinkDriver.EnsureInstalled(probe, reports.Add, system), "unplugged probe stops connection", "重新插拔");
        Check.That(system.Installed.Count == 1 && system.Waits == 20, "disconnected device has a bounded wait and no repeated install");

        system = new FakeSystem([missing], [ready with { InstanceId = ready.InstanceId + "OTHER" }]);
        await Check.ThrowsAsync<IOException>(() => StLinkDriver.EnsureInstalled(probe, reports.Add, system), "a different ready probe does not satisfy the installation wait");

        StLinkDriver.CheckInstallerExitCode(0);
        Check.Throws<IOException>(() => StLinkDriver.CheckInstallerExitCode(3010), "reboot-required helper result stops connection", "重新启动");
        Check.Throws<IOException>(() => StLinkDriver.CheckInstallerExitCode(5), "failed helper result stops connection", "安装失败");
        system = new FakeSystem([missing]) { InstallError = new IOException("需要重新启动 Windows") };
        await Check.ThrowsAsync<IOException>(() => StLinkDriver.EnsureInstalled(probe, reports.Add, system), "helper failure is propagated");
        Check.That(system.Scans == 1 && system.Waits == 0, "helper failure never continues to connection readiness");
    }

    private sealed class FakeSystem(params StLinkUsb.Device[][] snapshots) : IStLinkDriverSystem
    {
        public bool NativeArchitecture { get; init; } = true;
        internal Exception? CheckError { get; init; }
        internal Exception? InstallError { get; init; }
        internal int Checked, Scans, Waits;
        internal List<string> Installed { get; } = [];
        public Task<IReadOnlyList<StLinkUsb.Device>> ListDevices() =>
            Task.FromResult<IReadOnlyList<StLinkUsb.Device>>(snapshots[Math.Min(Scans++, snapshots.Length - 1)]);
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
