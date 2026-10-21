using System.Text.Json;
namespace ChipDock;

internal sealed class SerialPreferences
{
    private static readonly string DirectoryPath = AppSettings.StorageDirectory;
    private static readonly string FilePath = Path.Combine(DirectoryPath, "serial-preferences.json");
    internal static SerialPreferences Current { get; } = Load();
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
    private static SerialPreferences Load()
    {
        try { return JsonSerializer.Deserialize<SerialPreferences>(File.ReadAllText(FilePath)) ?? new(); }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or JsonException) { return new(); }
    }
    internal void Save()
    {
        Directory.CreateDirectory(DirectoryPath);
        var temporary = Path.Combine(DirectoryPath, $"serial-preferences.{Guid.NewGuid():N}.tmp");
        try { File.WriteAllText(temporary, JsonSerializer.Serialize(this)); File.Move(temporary, FilePath, true); }
        finally { if (File.Exists(temporary)) File.Delete(temporary); }
    }
}
