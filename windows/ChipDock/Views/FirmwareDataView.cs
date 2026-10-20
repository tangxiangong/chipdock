using ChipDock.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Foundation;
using static ChipDock.Formatting;

namespace ChipDock;

/// File bytes at their load addresses, without reading the target device.
internal sealed class FirmwareDataView : UserControl
{
    private const double ColumnSpacing = 12;
    private readonly TextBlock range = Ui.Mono(size: 12, style: "SecondaryTextStyle");
    private readonly TextBlock addressHeader = Ui.Text("地址", 12, "SecondaryTextStyle");
    private readonly TextBlock hexHeader = Ui.Text("十六进制", 12, "SecondaryTextStyle");
    private readonly TextBlock asciiHeader = Ui.Text("ASCII", 12, "SecondaryTextStyle");
    private readonly TextBlock addresses = Cell(secondary: true);
    private readonly TextBlock hex = Cell();
    private readonly TextBlock ascii = Cell(secondary: true);
    private readonly Grid header = new() { ColumnSpacing = ColumnSpacing, Padding = new Thickness(12, 6, 12, 6) };
    private readonly Grid rows = new() { ColumnSpacing = ColumnSpacing, Margin = new Thickness(12, 4, 12, 4) };
    private readonly ScrollViewer scroll = new()
    {
        HorizontalContentAlignment = HorizontalAlignment.Stretch,
        HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
        HorizontalScrollMode = ScrollMode.Disabled,
        VerticalScrollBarVisibility = ScrollBarVisibility.Hidden,
        VerticalScrollMode = ScrollMode.Enabled,
        MinHeight = 32,
    };
    private FirmwarePreview? preview, renderedPreview;
    private int shownColumns;

    internal FirmwareDataView()
    {
        var title = new Grid { ColumnSpacing = 12 };
        title.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        title.ColumnDefinitions.Add(new());
        title.Children.Add(Ui.Text("固件内容", 13));
        range.HorizontalAlignment = HorizontalAlignment.Right;
        range.MaxLines = 1;
        Grid.SetColumn(range, 1);
        title.Children.Add(range);
        ToolTipService.SetToolTip(title, "固件文件起始最多 4 KiB 的实际数据；·· 表示文件未写入的地址");

        for (var index = 0; index < 3; index++)
        {
            header.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
            rows.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        }
        Grid.SetColumn(hexHeader, 1);
        Grid.SetColumn(asciiHeader, 2);
        header.Children.Add(addressHeader);
        header.Children.Add(hexHeader);
        header.Children.Add(asciiHeader);
        Grid.SetColumn(hex, 1);
        Grid.SetColumn(ascii, 2);
        rows.Children.Add(addresses);
        rows.Children.Add(hex);
        rows.Children.Add(ascii);
        scroll.Content = rows;
        Ui.Name(scroll, "固件内容，地址、十六进制与 ASCII");

        var table = new Grid();
        table.RowDefinitions.Add(new() { Height = GridLength.Auto });
        table.RowDefinitions.Add(new());
        table.Children.Add(new Border { Style = Ui.Style("WellStyle"), Child = header });
        Grid.SetRow(scroll, 1);
        table.Children.Add(scroll);
        var border = new Border { Style = Ui.Style("CardStyle"), CornerRadius = new CornerRadius(6), Child = table };
        var content = new Grid { RowSpacing = 8 };
        content.RowDefinitions.Add(new() { Height = GridLength.Auto });
        content.RowDefinitions.Add(new());
        content.Children.Add(title);
        Grid.SetRow(border, 1);
        content.Children.Add(border);
        Content = content;
        MinHeight = 88;
        SizeChanged += (_, args) =>
        {
            if (Math.Abs(args.NewSize.Width - args.PreviousSize.Width) > 0.5) ArrangeColumns();
        };
    }

    protected override Size MeasureOverride(Size availableSize)
    {
        // The outer ScrollViewer offers infinite height. Report a compact
        // natural height so Grid can allocate spare height during arrangement.
        if (double.IsPositiveInfinity(availableSize.Height)) availableSize.Height = MinHeight;
        return base.MeasureOverride(availableSize);
    }

    internal void Update(FirmwarePreview? value)
    {
        if (ReferenceEquals(preview, value)) return;
        preview = value;
        if (value is null)
        {
            range.Text = addresses.Text = hex.Text = ascii.Text = "";
            renderedPreview = null;
            return;
        }
        var end = value.start_address + (ulong)Math.Max(value.bytes.Count - 1, 0);
        range.Text = $"{AddressLabel(value.start_address)} – {AddressLabel(end)}";
        scroll.ChangeView(null, 0, null, disableAnimation: true);
        ArrangeColumns();
    }

    private void ArrangeColumns()
    {
        if (preview is null || ActualWidth <= 0) return;
        // Measure the actual WinUI font, including hole markers, before choosing
        // a row length. All three columns remain readable in the narrow pane.
        var available = Math.Max(0, ActualWidth - 26);
        var end = preview.start_address + (ulong)Math.Max(preview.bytes.Count - 1, 0);
        var addressWidth = Math.Max(MeasureWidth(CellText(AddressLabel(end))), MeasureWidth(addressHeader));
        var columns = 1;
        double hexWidth = 0, asciiWidth = 0;
        foreach (var count in new[] { 16, 8, 4, 2, 1 })
        {
            columns = count;
            hexWidth = Math.Max(MeasureWidth(hexHeader), Math.Max(
                MeasureWidth(CellText(string.Join(" ", Enumerable.Repeat("00", count)))),
                MeasureWidth(CellText(string.Join(" ", Enumerable.Repeat("··", count))))));
            asciiWidth = Math.Max(MeasureWidth(asciiHeader), Math.Max(
                MeasureWidth(CellText(new string('W', count))), MeasureWidth(CellText(new string('·', count)))));
            if (addressWidth + hexWidth + asciiWidth + 2 * ColumnSpacing <= available) break;
        }
        var widths = new[] { addressWidth, hexWidth, asciiWidth };
        for (var index = 0; index < widths.Length; index++)
        {
            header.ColumnDefinitions[index].Width = new GridLength(widths[index]);
            rows.ColumnDefinitions[index].Width = new GridLength(widths[index]);
        }
        if (shownColumns == columns && ReferenceEquals(renderedPreview, preview)) return;
        shownColumns = columns;
        renderedPreview = preview;
        var addressLines = new List<string>();
        var hexLines = new List<string>();
        var asciiLines = new List<string>();
        for (var offset = 0; offset < preview.bytes.Count; offset += columns)
        {
            var bytes = preview.bytes.Skip(offset).Take(columns).ToArray();
            addressLines.Add(AddressLabel(preview.start_address + (ulong)offset));
            hexLines.Add(string.Join(" ", bytes.Select(value => value?.ToString("X2") ?? "··")));
            asciiLines.Add(new string(bytes.Select(value => value is { } data && data is >= 32 and <= 126 ? (char)data : '·').ToArray()));
        }
        addresses.Text = string.Join('\n', addressLines);
        hex.Text = string.Join('\n', hexLines);
        ascii.Text = string.Join('\n', asciiLines);
    }

    private static TextBlock Cell(bool secondary = false)
    {
        var block = Ui.Mono(size: 12, style: secondary ? "SecondaryTextStyle" : null);
        block.TextWrapping = TextWrapping.NoWrap;
        block.TextTrimming = TextTrimming.None;
        block.IsTextSelectionEnabled = true;
        block.LineHeight = 18;
        block.LineStackingStrategy = LineStackingStrategy.BlockLineHeight;
        return block;
    }

    private static TextBlock CellText(string text)
    {
        var block = Cell();
        block.Text = text;
        return block;
    }

    private static double MeasureWidth(TextBlock block)
    {
        block.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        return Math.Ceiling(block.DesiredSize.Width);
    }
}
