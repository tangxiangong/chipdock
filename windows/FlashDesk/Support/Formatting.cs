using FlashDesk.Core;
using System.Globalization;

namespace FlashDesk;

internal static class Formatting
{
    internal static string AddressLabel(ulong? value) => value is { } address ? $"0x{address:X8}" : "—";

    internal static string ByteCountLabel(ulong value) => value switch
    {
        >= 1_048_576 => $"{value / 1_048_576.0:0.0} MiB",
        >= 1024 => $"{value / 1024.0:0.0} KiB",
        _ => $"{value} B",
    };

    /// Hexadecimal with a 0x prefix, otherwise decimal.
    internal static ulong? ParseAddress(string text)
    {
        var trimmed = text.Trim();
        if (trimmed.StartsWith("0x", StringComparison.OrdinalIgnoreCase))
        {
            var digits = trimmed[2..];
            return digits.Length > 0 && digits.All(Uri.IsHexDigit)
                && ulong.TryParse(digits, NumberStyles.AllowHexSpecifier, CultureInfo.InvariantCulture, out var hex) ? hex : null;
        }
        return trimmed.Length > 0 && trimmed.All(char.IsAsciiDigit)
            && ulong.TryParse(trimmed, NumberStyles.None, CultureInfo.InvariantCulture, out var value) ? value : null;
    }

    internal static string RelativeTime(DateTimeOffset date)
    {
        var elapsed = DateTimeOffset.Now - date;
        if (elapsed < TimeSpan.FromMinutes(1)) return "刚刚";
        if (elapsed < TimeSpan.FromHours(1)) return $"{(int)elapsed.TotalMinutes} 分钟前";
        if (elapsed < TimeSpan.FromDays(1)) return $"{(int)elapsed.TotalHours} 小时前";
        if (elapsed < TimeSpan.FromDays(2)) return "昨天";
        if (elapsed < TimeSpan.FromDays(30)) return $"{(int)elapsed.TotalDays} 天前";
        return date.ToLocalTime().ToString("yyyy/M/d", CultureInfo.CurrentCulture);
    }

    internal static DateTimeOffset FromUnixMilliseconds(long milliseconds) =>
        DateTimeOffset.FromUnixTimeMilliseconds(Math.Clamp(milliseconds, -62_135_596_800_000, 253_402_300_799_999));

    /// The folder for display, with the user profile shortened to ~.
    internal static string DisplayDirectory(string path)
    {
        var directory = Path.GetDirectoryName(path) ?? path;
        var home = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        return home.Length > 0 && directory.StartsWith(home, StringComparison.OrdinalIgnoreCase)
            ? "~" + directory[home.Length..]
            : directory;
    }

    internal static string Title(this FirmwareFormat format) =>
        format.IsElf ? "ELF" : format.IsHex ? "HEX" : "BIN";

    internal static string Title(this WireProtocol protocol) => protocol.IsJtag ? "JTAG" : "SWD";

    internal static string Title(this MemoryRegionKind kind) =>
        kind.IsNvm ? "Flash" : kind.IsRam ? "RAM" : "存储区域";

    internal static bool IsTerminal(this JobStage stage) => stage.IsCompleted || stage.IsFailed;

    internal static string DisplayName(this ProbeSummary probe) => probe.product ?? probe.identifier;

    internal static byte[] Bytes(this MemoryReadResult result)
    {
        try { return Convert.FromHexString(result.data_hex); }
        catch (FormatException) { return []; }
    }

    internal static List<MemoryLine> Lines(this MemoryReadResult result, int width)
    {
        var bytes = result.Bytes();
        var lines = new List<MemoryLine>();
        for (var offset = 0; offset < bytes.Length; offset += width)
            lines.Add(new MemoryLine(result.address + (ulong)offset, bytes[offset..Math.Min(offset + width, bytes.Length)]));
        return lines;
    }
}
