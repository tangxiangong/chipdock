using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Windows.ApplicationModel.DataTransfer;

namespace FlashDesk;

internal sealed class LogsView : UserControl
{
    private readonly WorkbenchStore store;
    private readonly AutoSuggestBox filter = new() { PlaceholderText = "筛选日志", QueryIcon = new SymbolIcon(Symbol.Find) };
    private readonly ToggleButton errorsOnly = new() { Content = Ui.Glyph(Glyphs.Error, 14), Padding = new Thickness(8) };
    private readonly InfoBadge errorCount = new() { VerticalAlignment = VerticalAlignment.Center };
    private readonly ListView list = new() { SelectionMode = ListViewSelectionMode.None, Padding = new Thickness(0, 4, 0, 8) };
    private readonly TextBlock empty = Ui.Secondary();
    private readonly Button copy, export, clear;

    internal LogsView(WorkbenchStore store, MainWindow window)
    {
        this.store = store;
        errorCount.Style = Ui.Style("CriticalValueInfoBadgeStyle");
        Ui.CompactItems(list, new Thickness(16, 0, 16, 0));
        ToolTipService.SetToolTip(errorsOnly, "仅显示错误");
        Ui.Name(errorsOnly, "仅显示错误");
        errorsOnly.Click += (_, _) => Rebuild();
        filter.TextChanged += (_, _) => Rebuild();
        Ui.Name(filter, "筛选日志");
        copy = Ui.IconButton(Glyphs.Copy, "复制全部日志", () =>
        {
            var package = new DataPackage();
            package.SetText(store.LogExportText);
            Clipboard.SetContent(package);
        });
        export = Ui.IconButton(Glyphs.Export, "导出日志", () =>
            _ = window.Export(System.Text.Encoding.UTF8.GetBytes(store.LogExportText), "FlashDesk", "日志文件", ".log", "导出日志"));
        clear = Ui.IconButton(Glyphs.Delete, "清空日志", store.ClearLogs);

        var header = new Grid { ColumnSpacing = 4, Padding = new Thickness(16, 12, 8, 8) };
        header.ColumnDefinitions.Add(new());
        header.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        var title = Ui.Row(8, Ui.Text("日志", 16, semibold: true), errorCount);
        title.VerticalAlignment = VerticalAlignment.Center;
        var tools = Ui.Row(2, errorsOnly, copy, export, clear);
        Grid.SetColumn(tools, 1);
        header.Children.Add(title);
        header.Children.Add(tools);
        filter.Margin = new Thickness(16, 0, 16, 8);

        var body = new Grid();
        body.Children.Add(list);
        empty.HorizontalAlignment = HorizontalAlignment.Center;
        empty.VerticalAlignment = VerticalAlignment.Center;
        body.Children.Add(empty);

        var layout = new Grid();
        layout.RowDefinitions.Add(new() { Height = GridLength.Auto });
        layout.RowDefinitions.Add(new() { Height = GridLength.Auto });
        layout.RowDefinitions.Add(new());
        Grid.SetRow(filter, 1);
        Grid.SetRow(body, 2);
        layout.Children.Add(header);
        layout.Children.Add(filter);
        layout.Children.Add(body);
        Content = layout;

        store.LogAppended += Append;
        store.LogsCleared += Rebuild;
        Loaded += (_, _) => { Rebuild(); ScrollToEnd(); };
    }

    private bool Matches(ActivityEntry entry) =>
        (errorsOnly.IsChecked != true || entry.Level == ActivityLevel.Error)
        && (filter.Text.Length == 0 || $"{entry.Operation} {entry.Message} {entry.Detail}".Contains(filter.Text, StringComparison.CurrentCultureIgnoreCase));

    private void Append(ActivityEntry entry)
    {
        if (Matches(entry)) list.Items.Add(Line(entry));
        UpdateState();
        ScrollToEnd();
    }

    private void Rebuild()
    {
        list.Items.Clear();
        foreach (var entry in store.Logs.Where(Matches)) list.Items.Add(Line(entry));
        UpdateState();
    }

    private void ScrollToEnd()
    {
        if (list.Items.Count > 0) list.ScrollIntoView(list.Items[^1]);
    }

    private void UpdateState()
    {
        var errors = store.Logs.Count(entry => entry.Level == ActivityLevel.Error);
        errorCount.Value = errors;
        Ui.SetVisible(errorCount, errors > 0);
        ToolTipService.SetToolTip(errorCount, $"{errors} 条错误");
        empty.Text = store.Logs.Count == 0 ? "暂无日志" : "没有匹配的日志";
        Ui.SetVisible(empty, list.Items.Count == 0);
        copy.IsEnabled = export.IsEnabled = clear.IsEnabled = store.Logs.Count > 0;
    }

    private static Grid Line(ActivityEntry entry)
    {
        var line = new Grid { ColumnSpacing = 8, RowSpacing = 1, Padding = new Thickness(0, 2, 0, 2) };
        line.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        line.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        line.ColumnDefinitions.Add(new());
        var time = Ui.Mono(entry.Date.ToString("HH:mm:ss"), 12, "TertiaryTextStyle");
        var operation = Ui.Mono($"[{entry.Operation}]", 12, "SecondaryTextStyle");
        var message = Ui.Mono(entry.Message, 12, entry.Level switch
        {
            ActivityLevel.Success => "SuccessTextStyle",
            ActivityLevel.Error => "CriticalTextStyle",
            _ => null,
        });
        message.TextWrapping = TextWrapping.Wrap;
        Grid.SetColumn(operation, 1);
        Grid.SetColumn(message, 2);
        foreach (var text in new[] { time, operation, message })
        {
            text.IsTextSelectionEnabled = true;
            line.Children.Add(text);
        }
        if (entry.Detail.Length > 0)
        {
            line.RowDefinitions.Add(new() { Height = GridLength.Auto });
            line.RowDefinitions.Add(new() { Height = GridLength.Auto });
            var detail = Ui.Mono(entry.Detail, 12, "SecondaryTextStyle");
            detail.TextWrapping = TextWrapping.Wrap;
            detail.IsTextSelectionEnabled = true;
            Grid.SetRow(detail, 1);
            Grid.SetColumn(detail, 2);
            line.Children.Add(detail);
        }
        return line;
    }
}
