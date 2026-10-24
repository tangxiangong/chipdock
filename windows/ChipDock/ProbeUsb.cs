using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Runtime.Versioning;
using System.Text;
using Microsoft.Win32;
using Microsoft.Win32.SafeHandles;

namespace ChipDock;

/// SetupAPI is used instead of CIM/WMI so driver discovery needs no administrator rights.
[SupportedOSPlatform("windows")]
internal static class ProbeUsb
{
    private const uint ClassDriver = 1, ServiceProperty = 4, PresentAllClasses = 6;
    private const int NoMoreItems = 259, InvalidData = 13, InsufficientBuffer = 122;

    internal static bool IsDebugInterface(string instanceId) =>
        ProbeDriverRules.DebugInterface(instanceId, [], "") is not null;

    // Dynamic CMSIS-DAP / Glasgow identity is checked again against live descriptors
    // in the elevated helper. Obvious non-debug interfaces never reach native APIs.
    internal static bool CanInspect(string instanceId)
    {
        var id = UsbProbeIdentity.Parse(instanceId);
        if (id is null) return false;
        var family = ProbeDriverRules.KnownFamily(id.Vendor, id.Product);
        return family is null || family == ProbeFamily.Glasgow && id.Interface is not null || IsDebugInterface(instanceId);
    }

    internal static IReadOnlyList<UsbProbeDevice> ListDevices()
    {
        using var set = GetPresentDevices();
        var result = new List<UsbProbeDevice>();
        var physicalDevices = new Dictionary<(ushort Vendor, ushort Product), HashSet<string>>();
        void TrackPhysical(UsbProbeIdentity id, string parent)
        {
            var key = (id.Vendor, id.Product);
            if (!physicalDevices.TryGetValue(key, out var ids)) physicalDevices[key] = ids = new(StringComparer.OrdinalIgnoreCase);
            ids.Add(parent);
        }
        for (uint index = 0; ; index++)
        {
            var info = DeviceInfo.Create();
            if (!SetupDiEnumDeviceInfo(set, index, ref info))
            {
                if (Marshal.GetLastWin32Error() == NoMoreItems) break;
                throw NativeError("无法枚举 USB 设备");
            }
            var instanceId = InstanceId(info.DevInst);
            var id = UsbProbeIdentity.Parse(instanceId);
            if (id is null) continue;
            // Count all physical devices, including siblings with an unknown or
            // protected driver that never become repair candidates.
            if (id.Interface is null) TrackPhysical(id, instanceId);
            if (!CanInspect(instanceId)) continue;
            var compatibleIds = RegistryStrings(set, ref info, 2); // SPDRP_COMPATIBLEIDS
            var description = BusDescription(info.DevInst);
            var family = ProbeDriverRules.DebugInterface(instanceId, compatibleIds, description);
            if (family is null) continue;
            var identity = instanceId.Split('\\');
            var serial = identity[2];
            if (identity[1].Contains("&MI_", StringComparison.OrdinalIgnoreCase))
            {
                if (CM_Get_Parent(out var parent, info.DevInst, 0) != 0)
                    throw new IOException("无法读取烧录器的 USB 父设备。");
                var parentId = InstanceId(parent);
                TrackPhysical(id, parentId);
                serial = parentId.Split('\\')[^1];
            }
            var service = Service(set, ref info);
            if (service.Equals("usbccgp", StringComparison.OrdinalIgnoreCase)) continue;
            using var key = OpenParameters(set, ref info, create: false);
            var guid = ProbeDriverRules.RegisteredGuid(key?.GetValue("DeviceInterfaceGUIDs"), key?.GetValue("DeviceInterfaceGUID"));
            var started = CM_Get_DevNode_Status(out var status, out _, info.DevInst, 0) == 0
                && (status & 0x408) == 0x8; // DN_STARTED, without DN_HAS_PROBLEM
            var accessGuid = identity[1].Contains("&MI_", StringComparison.OrdinalIgnoreCase)
                ? guid : new Guid("A5DCBF10-6530-11D2-901F-00C04FB951ED"); // GUID_DEVINTERFACE_USB_DEVICE
            result.Add(new UsbProbeDevice(instanceId, id.Vendor, id.Product, serial, service, guid is not null,
                started, started && accessGuid is { } value && HasActiveInterface(instanceId, value), family.Value));
        }
        return result.Select(device => device with
        {
            UniqueOnBus = physicalDevices.TryGetValue((device.Vendor, device.Product), out var ids) && ids.Count == 1,
        }).ToArray();
    }

    /// Builds an in-memory driver list without selecting or installing anything.
    internal static void CheckInboxDriver(string instanceId)
    {
        using var set = GetPresentDevices();
        var info = DeviceInfo.Create();
        Require(SetupDiOpenDeviceInfoW(set, instanceId, 0, 0, ref info), "无法打开烧录器调试接口");
        BuildWinUsbDriverList(set, ref info);
        try { _ = FindWinUsbDriver(set, ref info); }
        finally { SetupDiDestroyDriverInfoList(set, ref info, ClassDriver); }
    }

    internal static bool Install(string instanceId)
    {
        // Revalidate against currently present devices in the elevated process.
        if (!CanInspect(instanceId)) throw new Win32Exception(87);
        var device = ListDevices().SingleOrDefault(candidate => candidate.InstanceId.Equals(instanceId, StringComparison.OrdinalIgnoreCase))
            ?? throw new Win32Exception(1167, "烧录器已断开。");
        if (device.Ready) return false;
        if (!device.NeedsRepair) throw new Win32Exception(50, "烧录器已绑定其他驱动。");

        using var set = GetPresentDevices();
        var info = DeviceInfo.Create();
        Require(SetupDiOpenDeviceInfoW(set, instanceId, 0, 0, ref info), "无法打开烧录器调试接口");
        if (!Service(set, ref info).Equals(device.Service, StringComparison.OrdinalIgnoreCase))
            throw new Win32Exception(1237, "烧录器驱动状态已改变，请重试。");

        var reboot = false;
        if (!device.UsesWinUsb)
        {
            BuildWinUsbDriverList(set, ref info);
            try
            {
                var driver = FindWinUsbDriver(set, ref info);
                Require(SetupDiSetSelectedDriverW(set, ref info, ref driver), "无法选择 WinUSB 驱动");
                AddGuid(set, ref info, device.Family);
                Require(DiInstallDevice(0, set, ref info, ref driver, 0, out reboot), "无法安装 WinUSB 驱动");
            }
            finally { SetupDiDestroyDriverInfoList(set, ref info, ClassDriver); }
        }
        else AddGuid(set, ref info, device.Family);

        if (!reboot)
        {
            // WinUSB reads DeviceInterfaceGUIDs on device start. Restart only the selected debug interface.
            var change = new PropertyChange
            {
                HeaderSize = 8, InstallFunction = 0x12, StateChange = 3, Scope = 2,
            }; // DIF_PROPERTYCHANGE, DICS_PROPCHANGE, DICS_FLAG_CONFIGSPECIFIC
            Require(SetupDiSetClassInstallParamsW(set, ref info, ref change, (uint)Marshal.SizeOf<PropertyChange>()), "无法准备烧录器接口重启");
            Require(SetupDiCallClassInstaller(0x12, set, ref info), "无法重启烧录器调试接口，请重新插拔设备");
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
            if (detail.Section.Split('.')[0].Equals("WINUSB", StringComparison.OrdinalIgnoreCase)) return driver;
        }
        throw new Win32Exception(1168, "Windows 内置 WinUSB 驱动不可用。");
    }

    private static void AddGuid(DeviceSet set, ref DeviceInfo info, ProbeFamily family)
    {
        using var key = OpenParameters(set, ref info, create: true) ?? throw new IOException("无法打开烧录器设备参数。");
        var guids = ProbeDriverRules.RepairGuids(key.GetValue("DeviceInterfaceGUIDs"), key.GetValue("DeviceInterfaceGUID"), family);
        key.SetValue("DeviceInterfaceGUIDs", guids, RegistryValueKind.MultiString);
    }

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
            throw new Win32Exception(error, "无法读取烧录器设备参数。");
        }
        return RegistryKey.FromHandle(new SafeRegistryHandle(handle, ownsHandle: true));
    }

    private static string Service(DeviceSet set, ref DeviceInfo info) =>
        RegistryStrings(set, ref info, ServiceProperty).FirstOrDefault() ?? "";

    private static string[] RegistryStrings(DeviceSet set, ref DeviceInfo info, uint property)
    {
        var buffer = new byte[8192];
        if (SetupDiGetDeviceRegistryPropertyW(set, ref info, property, out _, buffer, (uint)buffer.Length, out var size))
            return Encoding.Unicode.GetString(buffer, 0, checked((int)size)).Split('\0', StringSplitOptions.RemoveEmptyEntries);
        if (Marshal.GetLastWin32Error() == InvalidData) return [];
        throw NativeError("无法读取烧录器 USB 属性");
    }

    private static string BusDescription(uint devInst)
    {
        var key = new DevicePropertyKey { Format = new Guid("540b947e-8b40-45bc-a8a2-6a0b894cbda2"), Id = 4 };
        var buffer = new byte[8192];
        var size = (uint)buffer.Length;
        // DEVPKEY_Device_BusReportedDeviceDesc is the unlocalized USB descriptor,
        // unlike the friendly name installed by a third-party driver.
        return CM_Get_DevNode_PropertyW(devInst, ref key, out var type, buffer, ref size, 0) == 0 && type == 0x12
            ? Encoding.Unicode.GetString(buffer, 0, checked((int)size)).TrimEnd('\0') : "";
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
    private struct DevicePropertyKey { public Guid Format; public uint Id; }

    [DllImport("cfgmgr32.dll", ExactSpelling = true, CharSet = CharSet.Unicode)]
    private static extern uint CM_Get_DevNode_PropertyW(uint devInst, ref DevicePropertyKey key, out uint type, byte[] buffer, ref uint size, uint flags);

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
