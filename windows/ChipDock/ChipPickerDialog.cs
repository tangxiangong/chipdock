using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ChipDock;

/// Searches probe-rs targets, or offers the candidates left by a failed identification.
internal sealed class ChipPickerDialog : ContentDialog
{
    private readonly List<string> candidates;
    private readonly AutoSuggestBox query = new() { PlaceholderText = "搜索芯片型号，例如 STM32F407", QueryIcon = new SymbolIcon(Symbol.Find) };
    private readonly ProgressRing searching = new() { Width = 16, Height = 16, IsActive = true, Visibility = Visibility.Collapsed };
    private readonly TextBlock error = Ui.Text(size: 12, style: "CriticalTextStyle", wrap: true);
    private readonly ListView results = new() { SelectionMode = ListViewSelectionMode.Single, Height = 300 };
    private readonly TextBlock empty = Ui.Secondary();
    private readonly TextBlock count = Ui.Caption();
    private CancellationTokenSource? search;
    private bool closingWithSelection;

    /// The chosen chip; an empty string selects automatic identification.
    internal string? Selection { get; private set; }

    internal ChipPickerDialog(WorkbenchStore store)
    {
        candidates = store.TargetCandidates.Select(candidate => candidate.name).ToList();
        Title = candidates.Count == 0 ? "选择芯片" : "选择识别到的目标";
        PrimaryButtonText = candidates.Count == 0 ? "选择" : "选择并连接";
        CloseButtonText = "取消";
        DefaultButton = ContentDialogButton.Primary;

        query.TextChanged += (sender, args) => { if (args.Reason == AutoSuggestionBoxTextChangeReason.UserInput) _ = Search(); };
        query.QuerySubmitted += (_, _) => { if (Selection is not null) Hide(true); };
        Ui.Name(query, "搜索芯片型号");
        Ui.CompactItems(results, new Thickness(12, 0, 12, 0), 32);
        results.SelectionChanged += (_, _) => UpdateSelection();
        results.DoubleTapped += (_, _) => { if (Selection is not null) Hide(true); };
        empty.HorizontalAlignment = HorizontalAlignment.Center;
        empty.VerticalAlignment = VerticalAlignment.Center;
        var list = new Grid();
        list.Children.Add(new Border { Style = Ui.Style("CardStyle"), Child = results });
        list.Children.Add(empty);

        var search = new Grid { ColumnSpacing = 8 };
        search.ColumnDefinitions.Add(new());
        search.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        searching.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(searching, 1);
        search.Children.Add(query);
        search.Children.Add(searching);

        var subtitle = Ui.Secondary(candidates.Count == 0 ? "搜索 probe-rs 支持的芯片型号。" : "目标无法唯一识别，请从候选型号中选择。", 13, wrap: true);
        var content = Ui.Column(12, subtitle, search, error, list, count);
        content.MinWidth = 460;
        Content = content;

        PrimaryButtonClick += (_, args) => args.Cancel = Selection is null;
        CloseButtonClick += (_, _) => Selection = null;
        ShowResults(candidates.Count == 0 ? [""] : candidates, candidates.FirstOrDefault() ?? store.Chip);
    }

    private void Hide(bool withSelection)
    {
        closingWithSelection = withSelection;
        Hide();
    }

    /// The result after the dialog closes: the selection only when the user confirmed it.
    internal async Task<string?> Pick(FrameworkElement owner)
    {
        var result = await this.Show(owner);
        return result == ContentDialogResult.Primary || closingWithSelection ? Selection : null;
    }

    private async Task Search()
    {
        search?.Cancel();
        var cancellation = search = new CancellationTokenSource();
        var text = query.Text.Trim();
        error.Visibility = Visibility.Collapsed;
        if (text.Length == 0)
        {
            searching.Visibility = Visibility.Collapsed;
            ShowResults(candidates.Count == 0 ? [""] : candidates, candidates.FirstOrDefault() ?? "");
            return;
        }
        searching.Visibility = Visibility.Visible;
        IsPrimaryButtonEnabled = false;
        try
        {
            await Task.Delay(200, cancellation.Token);
            var found = await RustCore.SearchChips(text, 150);
            if (cancellation.IsCancellationRequested) return;
            ShowResults(found, found.FirstOrDefault());
        }
        catch (OperationCanceledException) { return; }
        catch (Exception failure)
        {
            if (cancellation.IsCancellationRequested) return;
            error.Text = failure.Message;
            error.Visibility = Visibility.Visible;
            ShowResults([], null);
        }
        searching.Visibility = Visibility.Collapsed;
        UpdateSelection();
    }

    private void ShowResults(List<string> names, string? selected)
    {
        results.Items.Clear();
        foreach (var name in names)
        {
            var label = name.Length == 0 ? Ui.Text("自动识别") : Ui.Mono(name, 13);
            var row = Ui.Row(10, Ui.Glyph(name.Length == 0 ? Glyphs.Search : Glyphs.Chip, 14), label);
            row.Tag = name;
            Ui.Name(row, name.Length == 0 ? "自动识别" : name);
            results.Items.Add(row);
        }
        results.SelectedItem = results.Items.Cast<FrameworkElement>().FirstOrDefault(row => (string)row.Tag == selected)
            ?? results.Items.FirstOrDefault();
        var real = names.Count(name => name.Length > 0);
        count.Text = real > 0 ? $"{real} 个目标" : "";
        empty.Text = query.Text.Trim().Length > 0 && real == 0 && error.Visibility == Visibility.Collapsed ? $"没有与“{query.Text.Trim()}”匹配的芯片" : "";
        UpdateSelection();
    }

    private void UpdateSelection()
    {
        Selection = (results.SelectedItem as FrameworkElement)?.Tag as string;
        IsPrimaryButtonEnabled = Selection is not null && searching.Visibility == Visibility.Collapsed;
    }
}
