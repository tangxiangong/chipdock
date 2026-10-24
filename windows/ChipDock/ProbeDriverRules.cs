using System.Globalization;

namespace ChipDock;

internal enum ProbeFamily { StLink, JLink, CmsisDap, Ftdi, WchLink, Ch347, Glasgow }

internal sealed record ProbeSelector(ushort Vendor, ushort Product, byte? Interface, string? Serial)
{
    internal static ProbeSelector? Parse(string selector)
    {
        var parts = selector.Trim().Split(':', 3);
        if (parts.Length < 2 || !ushort.TryParse(parts[0], NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var vendor)) return null;
        var product = parts[1].Split('-', 2);
        if (!ushort.TryParse(product[0], NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var pid)) return null;
        byte? number = null;
        if (product.Length == 2 && product[1].Length > 0)
        {
            if (!byte.TryParse(product[1], NumberStyles.None, CultureInfo.InvariantCulture, out var value)) return null;
            number = value;
        }
        return new(vendor, pid, number, parts.Length == 3 ? parts[2] : null);
    }
}

internal sealed record UsbProbeIdentity(ushort Vendor, ushort Product, byte? Interface)
{
    internal static UsbProbeIdentity? Parse(string instanceId)
    {
        var parts = instanceId.Split('\\');
        if (parts.Length != 3 || !parts[0].Equals("USB", StringComparison.OrdinalIgnoreCase) || parts[2].Length == 0) return null;
        var ids = parts[1].Split('&');
        if (ids.Length is not (2 or 3) || !ids[0].StartsWith("VID_", StringComparison.OrdinalIgnoreCase)
            || !ids[1].StartsWith("PID_", StringComparison.OrdinalIgnoreCase)
            || !ushort.TryParse(ids[0].AsSpan(4), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var vendor)
            || !ushort.TryParse(ids[1].AsSpan(4), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var product)) return null;
        byte? number = null;
        if (ids.Length == 3)
        {
            if (!ids[2].StartsWith("MI_", StringComparison.OrdinalIgnoreCase)
                || !byte.TryParse(ids[2].AsSpan(3), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out var value)) return null;
            number = value;
        }
        return new(vendor, product, number);
    }
}

internal sealed record UsbProbeDevice(string InstanceId, ushort Vendor, ushort Product, string Serial, string Service,
    bool HasInterfaceGuid, bool Started, bool InterfaceActive, ProbeFamily Family = ProbeFamily.StLink, bool UniqueOnBus = true)
{
    internal bool UsesWinUsb => Service.Equals("WinUSB", StringComparison.OrdinalIgnoreCase);
    internal bool IsComposite => InstanceId.Split('\\')[1].Contains("&MI_", StringComparison.OrdinalIgnoreCase);
    // nusb opens a standalone V2 through GUID_DEVINTERFACE_USB_DEVICE; a custom
    // interface GUID is needed only for a composite device's debug child.
    internal bool NeedsRepair => ProbeDriverRules.CanReplaceDriver(Family, Service)
        && (!UsesWinUsb || IsComposite && !HasInterfaceGuid);
    internal bool Ready => UsesWinUsb && (!IsComposite || HasInterfaceGuid) && Started && InterfaceActive;
}

/// Mirrors the USB backends in the locked probe-rs 0.32. Never infer a debug
/// interface from a vendor ID alone; composite parents and CDC/HID/storage stay intact.
internal static class ProbeDriverRules
{
    internal static bool IsStLink(ushort vendor, ushort product) => vendor == 0x0483 && product is
        0x3748 or 0x374b or 0x374a or 0x3742 or 0x3752 or 0x374e or 0x374f or 0x3753 or 0x3754 or 0x3757;

    internal static ProbeFamily? KnownFamily(ushort vendor, ushort product) => (vendor, product) switch
    {
        _ when IsStLink(vendor, product) => ProbeFamily.StLink,
        (0x1366, _) when product is 0x0101 or 0x0102 or 0x0103 or 0x0104 or 0x0105 or 0x0107 or 0x0108
            || (product & 0x1000) != 0 && (product & 0x0030) != 0 => ProbeFamily.JLink,
        (0x0403, 0x6010 or 0x6011 or 0x6014) or (0x15ba, 0x0003 or 0x0004 or 0x002a or 0x002b) => ProbeFamily.Ftdi,
        (0x1a86, 0x8010) => ProbeFamily.WchLink,
        (0x1a86, 0x55de or 0x55dd or 0x55e8) => ProbeFamily.Ch347,
        (0x20b7, 0x9db1) => ProbeFamily.Glasgow,
        _ => null,
    };

    internal static bool IsCmsisDap(string description) => new[] { "CMSIS-DAP", "CMSIS_DAP", "CMSIS DAP", "CMSISDAP" }
        .Any(value => description.Contains(value, StringComparison.OrdinalIgnoreCase));

    internal static ProbeFamily? DebugInterface(string instanceId, IReadOnlyList<string> compatibleIds, string description)
    {
        var id = UsbProbeIdentity.Parse(instanceId);
        if (id is null) return null;
        if (compatibleIds.Any(value => value.Equals(@"USB\COMPOSITE", StringComparison.OrdinalIgnoreCase))) return null;
        // Interface class descriptors take precedence over an allowlisted VID/PID.
        if (compatibleIds.Any(value => new[] { "02", "03", "08", "09", "0A" }
            .Any(code => value.StartsWith($@"USB\Class_{code}", StringComparison.OrdinalIgnoreCase)))) return null;
        var family = KnownFamily(id.Vendor, id.Product);
        return family switch
        {
            ProbeFamily.StLink when id.Interface == 0 || id.Interface is null && id.Product == 0x3748 => family,
            ProbeFamily.JLink or ProbeFamily.WchLink when id.Interface is null or 0 => family,
            ProbeFamily.Ftdi when id.Interface == 0 || id.Interface is null && id.Product == 0x6014 => family,
            ProbeFamily.Ch347 when id.Interface is 2 or 4 => family,
            ProbeFamily.Glasgow when id.Interface is not null && HasVendorClass(compatibleIds) => family,
            null when HasVendorClass(compatibleIds) && IsCmsisDap(description) => ProbeFamily.CmsisDap,
            _ => null,
        };
    }

    private static bool HasVendorClass(IReadOnlyList<string> ids) => ids.Any(value =>
        value.StartsWith(@"USB\Class_FF&SubClass_00", StringComparison.OrdinalIgnoreCase));

    internal static bool CanReplaceDriver(ProbeFamily family, string service) =>
        service.Length == 0 || service.Equals("WinUSB", StringComparison.OrdinalIgnoreCase)
        || service.Equals("libusb0", StringComparison.OrdinalIgnoreCase)
        || service.Equals("libusbK", StringComparison.OrdinalIgnoreCase)
        || family switch
        {
            ProbeFamily.StLink => service.StartsWith("STTub", StringComparison.OrdinalIgnoreCase),
            ProbeFamily.JLink => service.Equals("JLink", StringComparison.OrdinalIgnoreCase),
            ProbeFamily.Ftdi => service.Equals("FTDIBUS", StringComparison.OrdinalIgnoreCase),
            ProbeFamily.WchLink or ProbeFamily.Ch347 => service.Equals("WCHLink", StringComparison.OrdinalIgnoreCase)
                || service.Equals("WCHLinkRV", StringComparison.OrdinalIgnoreCase)
                || service.Equals("CH341WDM", StringComparison.OrdinalIgnoreCase)
                || service.Equals("CH347", StringComparison.OrdinalIgnoreCase),
            _ => false,
        };

    private static IEnumerable<string> GuidValues(object? value) => value switch
    {
        string[] values => values,
        string single => [single],
        _ => [],
    };

    internal static Guid? RegisteredGuid(object? multiple, object? single)
    {
        // nusb reads only the first value in REG_MULTI_SZ, falling back to the
        // singular registry value if that first value cannot be parsed.
        foreach (var first in new[] { GuidValues(multiple).FirstOrDefault(), GuidValues(single).FirstOrDefault() })
            if (Guid.TryParseExact(first, "B", out var guid) && guid != Guid.Empty) return guid;
        return null;
    }

    internal static string[] RepairGuids(object? multiple, object? single, ProbeFamily family = ProbeFamily.StLink) =>
        new[] { family switch
        {
            ProbeFamily.StLink => "{DBCE1CD9-A320-4B51-A365-A0C3F3C5FB29}",
            ProbeFamily.CmsisDap => "{CDB3B5AD-293B-4663-AA36-1AAE46463776}",
            _ => "{D8733B38-1A21-4CA1-AF1F-A4F66CE27351}",
        } }.Concat(GuidValues(multiple)).Concat(GuidValues(single))
            .Select(value => Guid.TryParse(value, out var guid) ? guid : Guid.Empty)
            .Where(guid => guid != Guid.Empty).Distinct().Select(guid => guid.ToString("B")).ToArray();

    internal static string Title(this ProbeFamily family) => family switch
    {
        ProbeFamily.StLink => "ST-Link", ProbeFamily.JLink => "J-Link", ProbeFamily.CmsisDap => "CMSIS-DAP v2",
        ProbeFamily.Ftdi => "FTDI", ProbeFamily.WchLink => "WCH-Link", ProbeFamily.Ch347 => "CH347", _ => "Glasgow",
    };
}
