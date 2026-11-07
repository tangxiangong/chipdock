using System.ComponentModel;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
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
    public string SequenceText => frame.sequence.ToString(System.Globalization.CultureInfo.CurrentCulture);
    public string Source => frame.source;
    public string Hex => frame.hex;
    public string PlainText => frame.text.utf8;
    public string ChecksumText
    {
        get
        {
            var checks = frame.parsed.Select(record => record.checksum_valid).Where(valid => valid.HasValue).ToArray();
            return checks.Length == 0 ? "—" : checks.Contains(false) ? "失败" : "通过";
        }
    }
    internal SerialFrameEvent Record => frame;
    internal int MemoryCost => frame.bytes.Count * 16 + 1024;
    internal object ExportRecord => new
    {
        direction = IsReceive ? "receive" : "send",
        bytes = frame.bytes,
        hex = frame.hex,
        text = new Dictionary<string, string> { ["ascii"] = frame.text.ascii, ["gbk"] = frame.text.gbk, ["latin-1"] = frame.text.latin1, ["utf-8"] = frame.text.utf8 },
        timestamp = frame.timestamp,
    };
    private string Text => format == "hex" && !mixed ? frame.hex : (format switch { "ascii" => frame.text.ascii, "gbk" => frame.text.gbk, "latin-1" => frame.text.latin1, _ => frame.text.utf8 })
        .Replace("\0", "␀").Replace("\r", "␍").Replace("\n", "␊\n").Replace("\t", "⇥").TrimEnd('\n');
    private bool mixed;
    public string DisplayText => mixed ? Text + "\n" + frame.hex : Text;
    public Visibility TimeVisibility { get; private set; } = Visibility.Visible;
    public Brush? DataColor { get; private set; }
    public Brush? MatchBackground { get; private set; }
    internal void SetFormat(string value, bool mix, bool timestamps)
    {
        if (format == value && mixed == mix && (TimeVisibility == Visibility.Visible) == timestamps) return;
        format = value; mixed = mix;
        TimeVisibility = timestamps ? Visibility.Visible : Visibility.Collapsed;
        PropertyChanged?.Invoke(this, new(nameof(DisplayText)));
        PropertyChanged?.Invoke(this, new(nameof(TimeVisibility)));
    }
    internal void Style(Brush? color, Brush? match)
    {
        if (DataColor != color) { DataColor = color; PropertyChanged?.Invoke(this, new(nameof(DataColor))); }
        if (MatchBackground != match) { MatchBackground = match; PropertyChanged?.Invoke(this, new(nameof(MatchBackground))); }
    }
    public event PropertyChangedEventHandler? PropertyChanged;
}
