using System.Text.Json;
namespace ChipDock;

internal sealed class SerialPreferences
{
    private static readonly string DirectoryPath = AppSettings.StorageDirectory;
    private static readonly string FilePath = Path.Combine(DirectoryPath, "serial-preferences.json");
    internal static SerialPreferences Current { get; } = Load();
    private bool transient;
    public byte LocalControl { get; set; }
    public int Version { get; set; } = 1;
    public string Draft { get; set; } = "";
    public List<string> History { get; set; } = [];
    public string Packets { get; set; } = "[]";
    public string Replies { get; set; } = "[]";
    public bool Mixed { get; set; }
    public bool Timestamps { get; set; } = true;
    public bool ResetOnClear { get; set; }
    public string ColorRules { get; set; } = "";
    public string TxColor { get; set; } = "default";
    public bool HexLineEnding { get; set; }
    public uint SkipPrefix { get; set; }
    public uint SkipSuffix { get; set; }
    public string FramingMode { get; set; } = "read";
    public string FramingValue { get; set; } = "";
    public bool AutoConnect { get; set; }
    public bool AutoReconnect { get; set; }
    public string ProtocolRules { get; set; } = "[]";
    public uint BaudRate { get; set; } = 115200;
    public byte DataBits { get; set; } = 8;
    public byte StopBits { get; set; } = 1;
    public int Parity { get; set; }
    public int FlowControl { get; set; }
    public string SelectedPath { get; set; } = "";
    public string DisplayFormat { get; set; } = "utf-8";
    public string SendFormat { get; set; } = "utf-8";
    public string LineEnding { get; set; } = "none";
    public bool AutoScroll { get; set; } = true;
    public int Checksum { get; set; }
    public int AutoSendIntervalMilliseconds { get; set; } = 1000;
    private static SerialPreferences Load()
    {
        try { return JsonSerializer.Deserialize<SerialPreferences>(File.ReadAllText(FilePath)) ?? new(); }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or JsonException) { return new(); }
    }
    internal SerialPreferences CopyForSession()
    {
        var copy = (SerialPreferences)MemberwiseClone();
        copy.transient = true;
        copy.AutoConnect = false;
        copy.Draft = "";
        copy.History = [];
        return copy;
    }
    internal void Save()
    {
        if (transient) return;
        Directory.CreateDirectory(DirectoryPath);
        var temporary = Path.Combine(DirectoryPath, $"serial-preferences.{Guid.NewGuid():N}.tmp");
        try { File.WriteAllText(temporary, JsonSerializer.Serialize(this)); File.Move(temporary, FilePath, true); }
        finally { if (File.Exists(temporary)) File.Delete(temporary); }
    }
}
