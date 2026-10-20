namespace ChipDock.Tests;

internal static class ProbeDriverRulesTests
{
    internal static void Run()
    {
        const string bulk = @"USB\Class_FF&SubClass_00&Prot_00";
        foreach (var (id, family) in new (string, ProbeFamily)[]
        {
            (@"USB\VID_0483&PID_3748\SERIAL", ProbeFamily.StLink),
            (@"USB\VID_0483&PID_374B&MI_00\CHILD", ProbeFamily.StLink),
            (@"USB\VID_1366&PID_0101\SERIAL", ProbeFamily.JLink),
            (@"USB\VID_1366&PID_1020&MI_00\CHILD", ProbeFamily.JLink),
            (@"USB\VID_0403&PID_6010&MI_00\CHILD", ProbeFamily.Ftdi),
            (@"USB\VID_0403&PID_6014\SERIAL", ProbeFamily.Ftdi),
            (@"USB\VID_15BA&PID_002B&MI_00\CHILD", ProbeFamily.Ftdi),
            (@"USB\VID_1A86&PID_8010&MI_00\CHILD", ProbeFamily.WchLink),
            (@"USB\VID_1A86&PID_55DE&MI_04\CHILD", ProbeFamily.Ch347),
            (@"USB\VID_1A86&PID_55DD&MI_02\CHILD", ProbeFamily.Ch347),
            (@"USB\VID_C251&PID_F001&MI_02\CHILD", ProbeFamily.CmsisDap),
            (@"USB\VID_20B7&PID_9DB1&MI_03\CHILD", ProbeFamily.Glasgow),
        })
            Check.That(ProbeDriverRules.DebugInterface(id, [bulk], "CMSIS-DAP") == family, $"recognized debug interface: {id}");

        foreach (var id in new[]
        {
            @"USB\VID_0483&PID_374B\PARENT", @"USB\VID_0483&PID_374B&MI_01\CHILD",
            @"USB\VID_0483&PID_374B&MI_02\CHILD", @"USB\VID_0403&PID_6010\PARENT",
            @"USB\VID_0403&PID_6010&MI_01\UART", @"USB\VID_0403&PID_6001\UART",
            @"USB\VID_1A86&PID_55DE\PARENT", @"USB\VID_1A86&PID_55DE&MI_00\UART",
            @"USB\VID_1366&PID_1000\UNKNOWN", @"ROOT\VID_0483&PID_3748\DEVICE", "",
        })
            Check.That(ProbeDriverRules.DebugInterface(id, [bulk], "USB device") is null, $"parent, serial or unrelated device excluded: {id}");

        const string dap = @"USB\VID_C251&PID_F001&MI_02\CHILD";
        Check.That(ProbeDriverRules.DebugInterface(dap, [bulk], "USB device") is null, "vendor class alone cannot authorize automatic binding");
        Check.That(ProbeDriverRules.DebugInterface(dap, [], "CMSIS-DAP") is null, "name alone cannot authorize automatic binding");
        Check.That(ProbeDriverRules.DebugInterface(@"USB\VID_1366&PID_1020\PARENT", [@"USB\COMPOSITE"], "J-Link") is null,
            "J-Link composite parent is never rebound");
        foreach (var code in new[] { "02", "03", "08", "09", "0A" })
            Check.That(ProbeDriverRules.DebugInterface(dap, [$@"USB\Class_{code}&SubClass_00"], "CMSIS-DAP") is null,
                "CDC, HID, storage and hub classes are protected");
        foreach (var family in Enum.GetValues<ProbeFamily>())
        {
            foreach (var service in new[] { "USBSTOR", "HidUsb", "usbser", "usbccgp", "FTSER2K", "unknown" })
                Check.That(!ProbeDriverRules.CanReplaceDriver(family, service), $"{family} protects {service}");
            Check.That(ProbeDriverRules.CanReplaceDriver(family, ""), $"{family} can install a missing driver");
            Check.That(ProbeDriverRules.CanReplaceDriver(family, "WINUSB"), $"{family} can repair WinUSB metadata");
        }
        Check.That(ProbeDriverRules.CanReplaceDriver(ProbeFamily.StLink, "STTub30"), "legacy ST-Link vendor driver can migrate");
        Check.That(ProbeDriverRules.CanReplaceDriver(ProbeFamily.JLink, "JLink"), "legacy J-Link vendor driver can migrate");
        Check.That(ProbeDriverRules.CanReplaceDriver(ProbeFamily.Ftdi, "FTDIBUS"), "selected FTDI debug channel can migrate");
        Check.That(ProbeDriverRules.CanReplaceDriver(ProbeFamily.Ch347, "CH341WDM"), "CH347 vendor driver can migrate");
        Check.That(ProbeSelector.Parse(" 0403:6010-1:serial ") is { Interface: 1, Serial: "serial" }, "FTDI channel selector is preserved");
        Check.That(ProbeSelector.Parse("20b7:9db1:serial:1:2")?.Serial == "serial:1:2", "Glasgow applet selector preserves both pipe numbers");
        Check.That(ProbeSelector.Parse("c251:f001-256") is null, "out-of-range interface is rejected");
        Check.That(UsbProbeIdentity.Parse(@"USB\VID_1234&PID_5678&MI_FF\CHILD")?.Interface == 255, "Windows interface number is hexadecimal");
    }
}
