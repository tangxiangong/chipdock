using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Runtime.Versioning;
using System.Text;
using Microsoft.Win32;
using Microsoft.Win32.SafeHandles;

namespace FlashDesk;

/// SetupAPI is used instead of CIM/WMI so driver discovery needs no administrator rights.
[SupportedOSPlatform("windows")]
internal static class StLinkUsb
{
    private const uint ClassDriver = 1, ServiceProperty = 4, PresentAllClasses = 6;
    private const int NoMoreItems = 259, InvalidData = 13, InsufficientBuffer = 122;
    // ST's debug interface GUID, also used by its programming tools.
    private const string DebugInterfaceGuid = "{DBCE1CD9-A320-4B51-A365-A0C3F3C5FB29}";

    internal sealed record Device(string InstanceId, ushort Vendor, ushort Product, string Serial, string Service,
        bool HasInterfaceGuid, bool Started, bool InterfaceActive)
    {
        internal bool UsesWinUsb => Service.Equals("WinUSB", StringComparison.OrdinalIgnoreCase);
        internal bool IsComposite => InstanceId.Split('\\')[1].Contains("&MI_", StringComparison.OrdinalIgnoreCase);
        // nusb opens a standalone V2 through GUID_DEVINTERFACE_USB_DEVICE; a custom
        // interface GUID is needed only for a composite device's debug child.
        internal bool NeedsRepair => Service.Length == 0 || (UsesWinUsb && IsComposite && !HasInterfaceGuid);
        internal bool Ready => UsesWinUsb && (!IsComposite || HasInterfaceGuid) && Started && InterfaceActive;
    }

    internal static bool IsDebugInterface(string instanceId)
    {
        var parts = instanceId.Split('\\');
        if (parts.Length != 3 || !parts[0].Equals("USB", StringComparison.OrdinalIgnoreCase) || parts[2].Length == 0) return false;
        var ids = parts[1].Split('&');
        if (ids.Length is not (2 or 3) || !ids[0].Equals("VID_0483", StringComparison.OrdinalIgnoreCase)
            || !ids[1].StartsWith("PID_", StringComparison.OrdinalIgnoreCase)
            || !ushort.TryParse(ids[1].AsSpan(4), System.Globalization.NumberStyles.HexNumber, null, out var product)
            || !StLinkDriver.IsSupported(0x0483, product)) return false;
        // V2 is a single-function device. Composite V2-1/V3 expose debug on MI_00.
        return ids.Length == 3 ? ids[2].Equals("MI_00", StringComparison.OrdinalIgnoreCase) : product == 0x3748;
    }

    internal static IReadOnlyList<Device> ListDevices()
    {
        using var set = GetPresentDevices();
        var result = new List<Device>();
        for (uint index = 0; ; index++)
        {
            var info = DeviceInfo.Create();
            if (!SetupDiEnumDeviceInfo(set, index, ref info))
            {
                if (Marshal.GetLastWin32Error() == NoMoreItems) break;
                throw NativeError("无法枚举 USB 设备");
            }
            var instanceId = InstanceId(info.DevInst);
            if (!IsDebugInterface(instanceId)) continue;
            var identity = instanceId.Split('\\');
            var product = Convert.ToUInt16(identity[1].Split('&')[1][4..], 16);
            var serial = identity[2];
            if (identity[1].Contains("&MI_", StringComparison.OrdinalIgnoreCase))
            {
                if (CM_Get_Parent(out var parent, info.DevInst, 0) != 0)
                    throw new IOException("无法读取 ST-Link 的 USB 父设备。");
                serial = InstanceId(parent).Split('\\')[^1];
            }
            using var key = OpenParameters(set, ref info, create: false);
            var guid = RegisteredGuid(key?.GetValue("DeviceInterfaceGUIDs"), key?.GetValue("DeviceInterfaceGUID"));
            var started = CM_Get_DevNode_Status(out var status, out _, info.DevInst, 0) == 0
                && (status & 0x408) == 0x8; // DN_STARTED, without DN_HAS_PROBLEM
            var accessGuid = identity[1].Contains("&MI_", StringComparison.OrdinalIgnoreCase)
                ? guid : new Guid("A5DCBF10-6530-11D2-901F-00C04FB951ED"); // GUID_DEVINTERFACE_USB_DEVICE
            result.Add(new Device(instanceId, 0x0483, product, serial, Service(set, ref info), guid is not null,
                started, started && accessGuid is { } value && HasActiveInterface(instanceId, value)));
        }
        return result;
    }

    /// Builds an in-memory driver list without selecting or installing anything.
    internal static void CheckInboxDriver(string instanceId)
    {
        using var set = GetPresentDevices();
        var info = DeviceInfo.Create();
        Require(SetupDiOpenDeviceInfoW(set, instanceId, 0, 0, ref info), "无法打开 ST-Link 调试接口");
        BuildWinUsbDriverList(set, ref info);
        try { _ = FindWinUsbDriver(set, ref info); }
        finally { SetupDiDestroyDriverInfoList(set, ref info, ClassDriver); }
    }

    internal static bool Install(string instanceId)
    {
        // Revalidate against currently present devices in the elevated process.
        if (!IsDebugInterface(instanceId)) throw new Win32Exception(87);
        var device = ListDevices().SingleOrDefault(candidate => candidate.InstanceId.Equals(instanceId, StringComparison.OrdinalIgnoreCase))
            ?? throw new Win32Exception(1167, "ST-Link 已断开。");
        if (device.Ready) return false;
        if (!device.NeedsRepair) throw new Win32Exception(50, "ST-Link 已绑定其他驱动。");

        using var set = GetPresentDevices();
        var info = DeviceInfo.Create();
        Require(SetupDiOpenDeviceInfoW(set, instanceId, 0, 0, ref info), "无法打开 ST-Link 调试接口");
        if (!Service(set, ref info).Equals(device.Service, StringComparison.OrdinalIgnoreCase))
            throw new Win32Exception(1237, "ST-Link 驱动状态已改变，请重试。");

        var reboot = false;
        if (device.Service.Length == 0)
        {
            BuildWinUsbDriverList(set, ref info);
            try
            {
                var driver = FindWinUsbDriver(set, ref info);
                Require(SetupDiSetSelectedDriverW(set, ref info, ref driver), "无法选择 WinUSB 驱动");
                AddGuid(set, ref info);
                Require(DiInstallDevice(0, set, ref info, ref driver, 0, out reboot), "无法安装 WinUSB 驱动");
            }
            finally { SetupDiDestroyDriverInfoList(set, ref info, ClassDriver); }
        }
        else AddGuid(set, ref info);

        if (!reboot)
        {
            // WinUSB reads DeviceInterfaceGUIDs on device start. Restart only debug MI_00.
            var change = new PropertyChange
            {
                HeaderSize = 8, InstallFunction = 0x12, StateChange = 3, Scope = 2,
            }; // DIF_PROPERTYCHANGE, DICS_PROPCHANGE, DICS_FLAG_CONFIGSPECIFIC
            Require(SetupDiSetClassInstallParamsW(set, ref info, ref change, (uint)Marshal.SizeOf<PropertyChange>()), "无法准备 ST-Link 接口重启");
            Require(SetupDiCallClassInstaller(0x12, set, ref info), "无法重启 ST-Link 调试接口，请重新插拔设备");
            var parameters = InstallParameters.Create();
            Require(SetupDiGetDeviceInstallParamsW(set, ref info, ref parameters), "无法读取驱动安装结果");
            reboot = (parameters.Flags & 0x180) != 0; // DI_NEEDRESTART | DI_NEEDREBOOT
        }
        return reboot;
    }

    private static void BuildWinUsbDriverList(DeviceSet set, ref DeviceInfo info)
    {
        var parameters = InstallParameters.Create();
        Require(SetupDiGetDeviceInstallParamsW(set, ref info, ref parameters), "无法读取驱动安装参数");
        parameters.Flags |= 0x00010000 | 0x00800000; // DI_ENUMSINGLEINF | DI_QUIETINSTALL
        parameters.FlagsEx |= 0x00000800; // DI_FLAGSEX_ALLOWEXCLUDEDDRVS
        parameters.DriverPath = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Windows), "INF", "winusb.inf");
        Require(SetupDiSetDeviceInstallParamsW(set, ref info, ref parameters), "无法设置驱动安装参数");
        Require(SetupDiBuildDriverInfoList(set, ref info, ClassDriver), "无法查找 Windows WinUSB 驱动");
    }

    private static DriverInfo FindWinUsbDriver(DeviceSet set, ref DeviceInfo info)
    {
        for (uint index = 0; ; index++)
        {
            var driver = new DriverInfo { Size = (uint)Marshal.SizeOf<DriverInfo>() };
            if (!SetupDiEnumDriverInfoW(set, ref info, ClassDriver, index, ref driver))
            {
                if (Marshal.GetLastWin32Error() == NoMoreItems) break;
                throw NativeError("无法枚举 WinUSB 驱动");
            }
            var detail = new DriverDetail { Size = (uint)Marshal.SizeOf<DriverDetail>() };
            // Static fields are filled even when the trailing hardware-ID buffer is too small.
            if (!SetupDiGetDriverInfoDetailW(set, ref info, ref driver, ref detail, detail.Size, out _)
                && Marshal.GetLastWin32Error() != InsufficientBuffer) throw NativeError("无法读取 WinUSB 驱动信息");
            if (detail.Section.Equals("WINUSB", StringComparison.OrdinalIgnoreCase)) return driver;
        }
        throw new Win32Exception(1168, "Windows 内置 WinUSB 驱动不可用。");
    }

    private static void AddGuid(DeviceSet set, ref DeviceInfo info)
    {
        using var key = OpenParameters(set, ref info, create: true) ?? throw new IOException("无法打开 ST-Link 设备参数。");
        var guids = RepairGuids(key.GetValue("DeviceInterfaceGUIDs"), key.GetValue("DeviceInterfaceGUID"));
        key.SetValue("DeviceInterfaceGUIDs", guids, RegistryValueKind.MultiString);
    }

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

    internal static string[] RepairGuids(object? multiple, object? single) =>
        new[] { DebugInterfaceGuid }.Concat(GuidValues(multiple)).Concat(GuidValues(single))
            .Select(value => Guid.TryParse(value, out var guid) ? guid : Guid.Empty)
            .Where(guid => guid != Guid.Empty).Distinct().Select(guid => guid.ToString("B")).ToArray();

    internal static bool HasActiveInterface(string instanceId, Guid guid)
    {
        // The registry can be ready before WinUSB has published the device path.
        for (var attempt = 0; attempt < 3; attempt++)
        {
            var result = CM_Get_Device_Interface_List_SizeW(out var length, ref guid, instanceId, 0);
            if (result != 0) return false;
            if (length <= 1) return false;
            var buffer = new char[length];
            result = CM_Get_Device_Interface_ListW(ref guid, instanceId, buffer, length, 0);
            if (result == 0) return buffer[0] != '\0';
            if (result != 0x1a) return false; // CR_BUFFER_SMALL: the list changed; retry.
        }
        return false;
    }

    private static RegistryKey? OpenParameters(DeviceSet set, ref DeviceInfo info, bool create)
    {
        var handle = create
            ? SetupDiCreateDevRegKeyW(set, ref info, 1, 0, 1, 0, null)
            : SetupDiOpenDevRegKey(set, ref info, 1, 0, 1, 0x20019);
        if (handle == -1)
        {
            var error = Marshal.GetLastWin32Error();
            if (!create && error is 2 or unchecked((int)0xe0000204)) return null;
            throw new Win32Exception(error, "无法读取 ST-Link 设备参数。");
        }
        return RegistryKey.FromHandle(new SafeRegistryHandle(handle, ownsHandle: true));
    }

    private static string Service(DeviceSet set, ref DeviceInfo info)
    {
        var buffer = new byte[1024];
        if (SetupDiGetDeviceRegistryPropertyW(set, ref info, ServiceProperty, out _, buffer, (uint)buffer.Length, out var size))
            return Encoding.Unicode.GetString(buffer, 0, (int)size).TrimEnd('\0');
        if (Marshal.GetLastWin32Error() == InvalidData) return "";
        throw NativeError("无法读取 ST-Link 驱动状态");
    }

    private static string InstanceId(uint devInst)
    {
        var buffer = new StringBuilder(1024);
        if (CM_Get_Device_IDW(devInst, buffer, buffer.Capacity, 0) != 0) throw new IOException("无法读取 USB 设备标识。");
        return buffer.ToString();
    }

    private static DeviceSet GetPresentDevices()
    {
        var set = SetupDiGetClassDevsW(0, "USB", 0, PresentAllClasses);
        if (!set.IsInvalid) return set;
        var error = NativeError("无法读取 USB 设备列表");
        set.Dispose();
        throw error;
    }

    private static void Require(bool success, string message) { if (!success) throw NativeError(message); }
    private static Win32Exception NativeError(string message) => new(Marshal.GetLastWin32Error(), message);

    private sealed class DeviceSet : SafeHandleZeroOrMinusOneIsInvalid
    {
        public DeviceSet() : base(true) { }
        protected override bool ReleaseHandle() => SetupDiDestroyDeviceInfoList(handle);
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct DeviceInfo
    {
        public uint Size;
        public Guid ClassGuid;
        public uint DevInst;
        public nuint Reserved;
        public static DeviceInfo Create() => new() { Size = (uint)Marshal.SizeOf<DeviceInfo>() };
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct InstallParameters
    {
        public uint Size, Flags, FlagsEx;
        public nint Parent, Callback, Context, FileQueue;
        public nuint ClassReserved;
        public uint Reserved;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)] public string DriverPath;
        public static InstallParameters Create() => new() { Size = (uint)Marshal.SizeOf<InstallParameters>(), DriverPath = "" };
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct DriverInfo
    {
        public uint Size, Type;
        public nuint Reserved;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 256)] public string Description;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 256)] public string Manufacturer;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 256)] public string Provider;
        public System.Runtime.InteropServices.ComTypes.FILETIME Date;
        public ulong Version;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct DriverDetail
    {
        public uint Size;
        public System.Runtime.InteropServices.ComTypes.FILETIME Date;
        public uint CompatibleOffset, CompatibleLength;
        public nuint Reserved;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 256)] public string Section;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)] public string InfPath;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 256)] public string Description;
        public char HardwareId;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct PropertyChange { public uint HeaderSize, InstallFunction, StateChange, Scope, Profile; }

    [DllImport("setupapi.dll", ExactSpelling = true, CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern DeviceSet SetupDiGetClassDevsW(nint classGuid, string enumerator, nint parent, uint flags);
    [DllImport("setupapi.dll", ExactSpelling = true, SetLastError = true)]
    private static extern bool SetupDiEnumDeviceInfo(DeviceSet set, uint index, ref DeviceInfo info);
    [DllImport("setupapi.dll", ExactSpelling = true)]
    private static extern bool SetupDiDestroyDeviceInfoList(nint set);
    [DllImport("setupapi.dll", ExactSpelling = true, CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool SetupDiOpenDeviceInfoW(DeviceSet set, string instanceId, nint parent, uint flags, ref DeviceInfo info);
    [DllImport("setupapi.dll", ExactSpelling = true, SetLastError = true)]
    private static extern nint SetupDiOpenDevRegKey(DeviceSet set, ref DeviceInfo info, uint scope, uint profile, uint type, uint access);
    [DllImport("setupapi.dll", ExactSpelling = true, CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern nint SetupDiCreateDevRegKeyW(DeviceSet set, ref DeviceInfo info, uint scope, uint profile, uint type, nint inf, string? section);
    [DllImport("setupapi.dll", ExactSpelling = true, SetLastError = true)]
    private static extern bool SetupDiGetDeviceRegistryPropertyW(DeviceSet set, ref DeviceInfo info, uint property, out uint type, byte[] buffer, uint size, out uint required);
    [DllImport("cfgmgr32.dll", ExactSpelling = true)]
    private static extern uint CM_Get_Parent(out uint parent, uint devInst, uint flags);
    [DllImport("cfgmgr32.dll", ExactSpelling = true)]
    private static extern uint CM_Get_DevNode_Status(out uint status, out uint problem, uint devInst, uint flags);
    [DllImport("cfgmgr32.dll", ExactSpelling = true, CharSet = CharSet.Unicode)]
    private static extern uint CM_Get_Device_Interface_List_SizeW(out uint length, ref Guid guid, string instanceId, uint flags);
    [DllImport("cfgmgr32.dll", ExactSpelling = true, CharSet = CharSet.Unicode)]
    private static extern uint CM_Get_Device_Interface_ListW(ref Guid guid, string instanceId, [Out] char[] buffer, uint length, uint flags);
    [DllImport("cfgmgr32.dll", ExactSpelling = true, CharSet = CharSet.Unicode)]
    private static extern uint CM_Get_Device_IDW(uint devInst, StringBuilder buffer, int length, uint flags);
    [DllImport("setupapi.dll", ExactSpelling = true, CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool SetupDiGetDeviceInstallParamsW(DeviceSet set, ref DeviceInfo info, ref InstallParameters parameters);
    [DllImport("setupapi.dll", ExactSpelling = true, CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool SetupDiSetDeviceInstallParamsW(DeviceSet set, ref DeviceInfo info, ref InstallParameters parameters);
    [DllImport("setupapi.dll", ExactSpelling = true, SetLastError = true)]
    private static extern bool SetupDiBuildDriverInfoList(DeviceSet set, ref DeviceInfo info, uint type);
    [DllImport("setupapi.dll", ExactSpelling = true, CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool SetupDiEnumDriverInfoW(DeviceSet set, ref DeviceInfo info, uint type, uint index, ref DriverInfo driver);
    [DllImport("setupapi.dll", ExactSpelling = true, CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool SetupDiGetDriverInfoDetailW(DeviceSet set, ref DeviceInfo info, ref DriverInfo driver, ref DriverDetail detail, uint size, out uint required);
    [DllImport("setupapi.dll", ExactSpelling = true, CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool SetupDiSetSelectedDriverW(DeviceSet set, ref DeviceInfo info, ref DriverInfo driver);
    [DllImport("setupapi.dll", ExactSpelling = true, SetLastError = true)]
    private static extern bool SetupDiDestroyDriverInfoList(DeviceSet set, ref DeviceInfo info, uint type);
    [DllImport("newdev.dll", ExactSpelling = true, SetLastError = true)]
    private static extern bool DiInstallDevice(nint parent, DeviceSet set, ref DeviceInfo info, ref DriverInfo driver, uint flags, out bool reboot);
    [DllImport("setupapi.dll", ExactSpelling = true, SetLastError = true)]
    private static extern bool SetupDiSetClassInstallParamsW(DeviceSet set, ref DeviceInfo info, ref PropertyChange change, uint size);
    [DllImport("setupapi.dll", ExactSpelling = true, SetLastError = true)]
    private static extern bool SetupDiCallClassInstaller(uint function, DeviceSet set, ref DeviceInfo info);
}
