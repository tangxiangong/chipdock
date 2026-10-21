using System.ComponentModel;
using Microsoft.UI.Xaml.Controls;
using ChipDock.Core;
namespace ChipDock;

public sealed class TerminalFrame : INotifyPropertyChanged
{
    private readonly SerialFrameEvent frame;
    private string format;
    internal TerminalFrame(SerialFrameEvent frame, string format) { this.frame = frame; this.format = format; }
    public bool IsReceive => frame.direction.IsReceive;
    public string Direction => IsReceive ? "RX" : "TX";
    public Symbol DirectionIcon => IsReceive ? Symbol.Download : Symbol.Upload;
    public string Time => DateTimeOffset.TryParse(frame.timestamp, System.Globalization.CultureInfo.InvariantCulture, System.Globalization.DateTimeStyles.None, out var value)
        ? value.ToLocalTime().ToString("HH:mm:ss.fff", System.Globalization.CultureInfo.InvariantCulture) : frame.timestamp;
    public string Size => frame.bytes.Count.ToString(System.Globalization.CultureInfo.CurrentCulture);
    internal int MemoryCost => frame.bytes.Count * 16 + 1024;
    public string DisplayText => format == "hex" ? frame.hex : (format switch { "ascii" => frame.text.ascii, "gbk" => frame.text.gbk, _ => frame.text.utf8 })
        .Replace("\0", "␀").Replace("\r", "␍").Replace("\n", "␊\n").Replace("\t", "⇥").TrimEnd('\n');
    internal void SetFormat(string value) { format = value; PropertyChanged?.Invoke(this, new(nameof(DisplayText))); }
    public event PropertyChangedEventHandler? PropertyChanged;
}
