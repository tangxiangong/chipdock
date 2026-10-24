using Microsoft.UI.Xaml;

namespace ChipDock;

internal enum EraseStrategy { Automatic, Full, Skip }

internal enum Appearance { System, Light, Dark }

internal enum ActivityLevel { Info, Success, Error }

internal enum WorkbenchOperation { Scan, Connect, Flash, Erase, Read }

internal enum InspectorPane { Logs, Memory }

internal sealed record ActivityEntry(DateTimeOffset Date, string Operation, ActivityLevel Level, string Message, string Detail = "");

internal sealed record UserFacingError(string Message, string Detail, string Recovery);

internal sealed record MemoryLine(ulong Address, byte[] Bytes)
{
    internal string Ascii => new(Bytes.Select(value => value is >= 32 and <= 126 ? (char)value : '·').ToArray());
}

internal static class WorkspaceModelText
{
    internal static string Title(this EraseStrategy strategy) => strategy switch
    {
        EraseStrategy.Full => "整片擦除",
        EraseStrategy.Skip => "跳过擦除",
        _ => "按需擦除",
    };

    internal static string Detail(this EraseStrategy strategy) => strategy switch
    {
        EraseStrategy.Full => "写入前擦除整个 Flash，其他数据会丢失",
        EraseStrategy.Skip => "不擦除，目标区域须已擦除",
        _ => "仅擦除固件覆盖的扇区",
    };

    internal static string Title(this Appearance appearance) => appearance switch
    {
        Appearance.Light => "浅色",
        Appearance.Dark => "深色",
        _ => "跟随系统",
    };

    internal static ElementTheme Theme(this Appearance appearance) => appearance switch
    {
        Appearance.Light => ElementTheme.Light,
        Appearance.Dark => ElementTheme.Dark,
        _ => ElementTheme.Default,
    };

    internal static string Title(this ActivityLevel level) => level switch
    {
        ActivityLevel.Success => "完成",
        ActivityLevel.Error => "错误",
        _ => "信息",
    };

    internal static string Title(this WorkbenchOperation operation) => operation switch
    {
        WorkbenchOperation.Scan => "扫描探针",
        WorkbenchOperation.Connect => "连接目标",
        WorkbenchOperation.Flash => "固件烧录",
        WorkbenchOperation.Erase => "擦除芯片",
        _ => "读取内存",
    };
}
