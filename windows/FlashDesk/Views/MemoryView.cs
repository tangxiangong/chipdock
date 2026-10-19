using FlashDesk.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Media;
using Windows.System;
using static FlashDesk.Formatting;

namespace FlashDesk;

internal sealed class MemoryView : UserControl
{
    // Eight bytes per row keeps the pane narrow.
    private const int RowWidth = 8;
    private static readonly uint[] Lengths = [64, 128, 256, 512, 1024, 2048, 4096];

    private readonly WorkbenchStore store;
    private readonly TextBox address = new() { PlaceholderText = "0x08000000" };
    private readonly FontIcon addressError = Ui.Glyph(Glyphs.Warning, 14, "CriticalGlyphStyle");
    private readonly ComboBox length = new();
    private readonly Button read;
    private readonly Button export;
    private readonly Button chipActions = new() { Content = Ui.Glyph(Glyphs.More, 14), Padding = new Thickness(8) };
    private readonly MenuFlyoutItem eraseItem;
    private readonly StackPanel mapSection = new();
    private readonly ListView map = new() { SelectionMode = ListViewSelectionMode.Single, MaxHeight = 168 };
    private readonly TextBlock dump = Ui.Mono(size: 12);
    private readonly ScrollViewer dumpScroller = new()
    {
        HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
        HorizontalScrollMode = ScrollMode.Auto,
        Padding = new Thickness(16, 8, 16, 8),
    };
    private readonly StackPanel placeholder = new() { Spacing = 8, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
    private readonly TextBlock placeholderDetail = Ui.Secondary(size: 13);
    private List<MemoryRegionLayout>? shownRegions;
    private MemoryReadResult? shownResult;
    private bool updatingMap;

    internal MemoryView(WorkbenchStore store, MainWindow window)
    {
        this.store = store;
        address.FontFamily = (FontFamily)Application.Current.Resources["MonoFontFamily"];
        address.TextChanged += (_, _) => store.MemoryAddress = address.Text;
        address.KeyDown += (_, args) =>
        {
            if (args.Key != VirtualKey.Enter) return;
            args.Handled = true;
            _ = store.ReadMemory();
        };
        Ui.Name(address, "起始地址");
        ToolTipService.SetToolTip(addressError, "请输入十六进制地址");
        Ui.Name(addressError, "地址无效");
        foreach (var value in Lengths) length.Items.Add($"{value} B");
        length.SelectionChanged += (_, _) => { if (length.SelectedIndex >= 0) store.MemoryLength = Lengths[length.SelectedIndex]; };
        Ui.Name(length, "长度");
        read = Ui.Button(Glyphs.Download, "读取", () => _ = store.ReadMemory(), "读取内存（Ctrl+Shift+R）");
        export = Ui.IconButton(Glyphs.Export, "导出为二进制", () =>
        {
            if (store.MemoryResult is { } result)
                _ = window.Export(result.Bytes(), $"memory-{AddressLabel(result.address)}", "二进制文件", ".bin", "导出内存");
        });
        chipActions.Background = chipActions.BorderBrush = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        ToolTipService.SetToolTip(chipActions, "芯片操作");
        Ui.Name(chipActions, "芯片操作");
        eraseItem = Ui.MenuItem("擦除整个芯片…", Glyphs.Delete, () => _ = window.ConfirmErase());
        var chipMenu = new MenuFlyout();
        chipMenu.Items.Add(eraseItem);
        chipActions.Flyout = chipMenu;

        var header = new Grid { Padding = new Thickness(16, 12, 8, 8) };
        header.ColumnDefinitions.Add(new());
        header.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        var title = Ui.Text("内存", 16, semibold: true);
        title.VerticalAlignment = VerticalAlignment.Center;
        var tools = Ui.Row(2, export, chipActions);
        Grid.SetColumn(tools, 1);
        header.Children.Add(title);
        header.Children.Add(tools);

        var inputs = new Grid { ColumnSpacing = 8, Padding = new Thickness(16, 0, 16, 12) };
        inputs.ColumnDefinitions.Add(new());
        inputs.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        inputs.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        inputs.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        addressError.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(addressError, 1);
        Grid.SetColumn(length, 2);
        Grid.SetColumn(read, 3);
        inputs.Children.Add(address);
        inputs.Children.Add(addressError);
        inputs.Children.Add(length);
        inputs.Children.Add(read);

        Ui.CompactItems(map, new Thickness(16, 0, 16, 0), 30);
        map.SelectionChanged += (_, _) =>
        {
            if (updatingMap || map.SelectedItem is not FrameworkElement { Tag: MemoryRegionLayout region }) return;
            store.MemoryAddress = AddressLabel(region.start);
            Ui.SetText(address, store.MemoryAddress);
        };
        var mapTitle = Ui.Caption("内存映射");
        mapTitle.Margin = new Thickness(16, 8, 16, 4);
        mapSection.Children.Add(Ui.Divider());
        mapSection.Children.Add(mapTitle);
        mapSection.Children.Add(map);

        dump.IsTextSelectionEnabled = true;
        dump.TextWrapping = TextWrapping.NoWrap;
        dump.TextTrimming = TextTrimming.None;
        dumpScroller.Content = dump;
        ActualThemeChanged += (_, _) => { shownResult = null; Update(); };
        placeholder.Children.Add(new FontIcon { Glyph = Glyphs.Memory, FontSize = 32, Style = Ui.Style("SecondaryGlyphStyle"), HorizontalAlignment = HorizontalAlignment.Center });
        var placeholderTitle = Ui.Text("尚未读取", 16, semibold: true);
        placeholderTitle.HorizontalAlignment = placeholderDetail.HorizontalAlignment = HorizontalAlignment.Center;
        placeholder.Children.Add(placeholderTitle);
        placeholder.Children.Add(placeholderDetail);
        var body = new Grid();
        body.Children.Add(dumpScroller);
        body.Children.Add(placeholder);

        var layout = new Grid();
        for (var row = 0; row < 4; row++) layout.RowDefinitions.Add(new() { Height = row == 3 ? new GridLength(1, GridUnitType.Star) : GridLength.Auto });
        Grid.SetRow(inputs, 1);
        Grid.SetRow(mapSection, 2);
        Grid.SetRow(body, 3);
        layout.Children.Add(header);
        layout.Children.Add(inputs);
        layout.Children.Add(mapSection);
        layout.Children.Add(body);
        var bodyDivider = Ui.Divider();
        bodyDivider.VerticalAlignment = VerticalAlignment.Top;
        Grid.SetRow(bodyDivider, 3);
        layout.Children.Add(bodyDivider);
        Content = layout;
    }

    internal void Update()
    {
        Ui.SetText(address, store.MemoryAddress);
        Ui.SetVisible(addressError, ParseAddress(store.MemoryAddress) is null);
        var lengthIndex = Array.IndexOf(Lengths, store.MemoryLength);
        if (length.SelectedIndex != lengthIndex) length.SelectedIndex = lengthIndex;
        read.IsEnabled = store.CanReadMemory;
        export.IsEnabled = store.MemoryResult is not null && !store.IsBusy;
        eraseItem.IsEnabled = store.CanErase;
        map.IsEnabled = !store.IsBusy;

        Ui.SetVisible(mapSection, store.MemoryRegions.Count > 0);
        if (!ReferenceEquals(shownRegions, store.MemoryRegions))
        {
            shownRegions = store.MemoryRegions;
            map.Items.Clear();
            foreach (var region in store.MemoryRegions) map.Items.Add(RegionRow(region));
        }
        // Selection follows the region containing the read address.
        updatingMap = true;
        var current = ParseAddress(store.MemoryAddress) is { } value
            ? store.MemoryRegions.FindIndex(region => value >= region.start && value < region.end)
            : -1;
        if (map.SelectedIndex != current) map.SelectedIndex = current;
        updatingMap = false;

        Ui.SetVisible(dumpScroller, store.MemoryResult is not null);
        Ui.SetVisible(placeholder, store.MemoryResult is null);
        placeholderDetail.Text = store.Connection is null ? "连接目标后可读取内存" : "输入地址后按 Enter 或点击“读取”";
        if (store.MemoryResult is { } result && !ReferenceEquals(result, shownResult)) Render(result);
        if (store.MemoryResult is null) shownResult = null;
    }

    private static Grid RegionRow(MemoryRegionLayout region)
    {
        var row = new Grid { ColumnSpacing = 8, Tag = region };
        row.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new());
        row.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new() { Width = new GridLength(64) });
        var bar = new Border { Width = 3, Height = 14, CornerRadius = new CornerRadius(1.5), Background = Ui.Brush(region.kind.Tint()) };
        var name = Ui.Text(region.name ?? region.kind.Title(), 13);
        var start = Ui.Mono(AddressLabel(region.start), 12, "SecondaryTextStyle");
        var size = Ui.Text(ByteCountLabel(region.size), 12, "SecondaryTextStyle");
        size.HorizontalAlignment = HorizontalAlignment.Right;
        Grid.SetColumn(name, 1);
        Grid.SetColumn(start, 2);
        Grid.SetColumn(size, 3);
        foreach (var element in new FrameworkElement[] { bar, name, start, size }) { element.VerticalAlignment = VerticalAlignment.Center; row.Children.Add(element); }
        ToolTipService.SetToolTip(row, $"将起始地址设为 {AddressLabel(region.start)}");
        Ui.Name(row, region.name ?? region.kind.Title());
        return row;
    }

    /// Erased (0xFF) and zero bytes are dimmed so meaningful data stands out.
    private void Render(MemoryReadResult result)
    {
        shownResult = result;
        var secondary = Ui.ThemeBrush(this, "TextFillColorSecondaryBrush");
        var dimmed = Ui.ThemeBrush(this, "TextFillColorDisabledBrush");
        var inlines = dump.Inlines;
        inlines.Clear();
        void Add(string text, Brush? brush = null)
        {
            var run = new Run { Text = text };
            if (brush is not null) run.Foreground = brush;
            inlines.Add(run);
        }
        Add($"Offset      {string.Join(" ", Enumerable.Range(0, RowWidth).Select(index => index.ToString("X2")))}  ASCII", secondary);
        foreach (var line in result.Lines(RowWidth))
        {
            inlines.Add(new LineBreak());
            Add(AddressLabel(line.Address) + "  ", secondary);
            // Coalesce neighbouring bytes of the same emphasis into one run.
            var text = new System.Text.StringBuilder();
            bool? dim = null;
            for (var index = 0; index < RowWidth; index++)
            {
                var cell = index < line.Bytes.Length ? line.Bytes[index].ToString("X2") : "  ";
                var cellDim = index < line.Bytes.Length && line.Bytes[index] is 0x00 or 0xFF;
                if (dim is { } current && current != cellDim)
                {
                    Add(text.ToString(), current ? dimmed : null);
                    text.Clear();
                }
                dim = cellDim;
                text.Append(index == 0 ? cell : " " + cell);
            }
            Add(text.ToString(), dim == true ? dimmed : null);
            Add("  " + line.Ascii, secondary);
        }
    }
}
