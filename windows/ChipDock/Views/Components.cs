using ChipDock.Core;
using Microsoft.UI;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.System;
using Windows.UI;

namespace ChipDock;

/// Segoe Fluent Icons code points used across the app.
internal static class Glyphs
{
    internal const string OpenFile = "\uE8E5", Document = "\uE8A5", Folder = "\uE838", Refresh = "\uE72C";
    internal const string Link = "\uE71B", Cancel = "\uE711", Bookmark = "\uE8A4", Save = "\uE74E";
    internal const string Delete = "\uE74D", Settings = "\uE713", Info = "\uE946", Copy = "\uE8C8";
    internal const string Export = "\uEDE1", Download = "\uE896", Flash = "\uE945", Shield = "\uEA18";
    internal const string Warning = "\uE7BA", Error = "\uE783", More = "\uE712", Logs = "\uE8FD";
    internal const string Memory = "\uEEA0", Chip = "\uE950", Search = "\uE721", ChevronDown = "\uE70D";
    internal const string Sidebar = "\uE90D";
    internal const string Probe = "\uE88E", Close = "\uE8BB", Filter = "\uE71C", Erase = "\uE75C";
    internal const string Verify = "\uE73E", Reset = "\uE777", Swap = "\uE8AB", Edit = "\uE70F";
}

internal static class Ui
{
    private static readonly SemaphoreSlim DialogGate = new(1, 1);

    internal static Style Style(string key) => (Style)Application.Current.Resources[key];

    internal static TextBlock Text(string text = "", double size = 14, string? style = null, bool semibold = false, bool wrap = false) => new()
    {
        Text = text,
        FontSize = size,
        Style = style is null ? null : Style(style),
        FontWeight = semibold ? FontWeights.SemiBold : FontWeights.Normal,
        TextWrapping = wrap ? TextWrapping.Wrap : TextWrapping.NoWrap,
        TextTrimming = wrap ? TextTrimming.None : TextTrimming.CharacterEllipsis,
    };

    internal static TextBlock Secondary(string text = "", double size = 14, bool wrap = false) =>
        Text(text, size, "SecondaryTextStyle", wrap: wrap);

    internal static TextBlock Caption(string text = "") => Text(text, 12, "CaptionSecondaryTextStyle");

    internal static TextBlock Mono(string text = "", double size = 13, string? style = null)
    {
        var block = Text(text, size, style);
        block.FontFamily = (FontFamily)Application.Current.Resources["MonoFontFamily"];
        return block;
    }

    internal static FontIcon Glyph(string glyph, double size = 16, string? style = null) => new()
    {
        Glyph = glyph,
        FontSize = size,
        Style = style is null ? null : Style(style),
    };

    internal static StackPanel Row(double spacing, params UIElement[] children)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = spacing };
        foreach (var child in children) row.Children.Add(child);
        return row;
    }

    internal static StackPanel Column(double spacing, params UIElement[] children)
    {
        var column = new StackPanel { Spacing = spacing };
        foreach (var child in children) column.Children.Add(child);
        return column;
    }

    internal static Border Divider(bool vertical = false) => new()
    {
        Style = Style("DividerStyle"),
        Height = vertical ? double.NaN : 1,
        Width = vertical ? 1 : double.NaN,
    };

    /// A labelled button with a leading icon.
    internal static Button Button(string glyph, string label, Action click, string? tip = null)
    {
        var button = new Button { Content = Row(8, Glyph(glyph, 14), Text(label)) };
        button.Click += (_, _) => click();
        Name(button, label);
        if (tip is not null) ToolTipService.SetToolTip(button, tip);
        return button;
    }

    /// A borderless icon button, for pane headers.
    internal static Button IconButton(string glyph, string tip, Action click)
    {
        var button = new Button
        {
            Content = Glyph(glyph, 14),
            Background = new SolidColorBrush(Colors.Transparent),
            BorderBrush = new SolidColorBrush(Colors.Transparent),
            Padding = new Thickness(8),
        };
        button.Click += (_, _) => click();
        ToolTipService.SetToolTip(button, tip);
        Name(button, tip);
        return button;
    }

    /// List rows sized to their content rather than the 40px touch default.
    internal static void CompactItems(ListView list, Thickness padding, double minHeight = 0) =>
        list.ItemContainerStyle = new Style(typeof(ListViewItem))
        {
            BasedOn = Style("DefaultListViewItemStyle"),
            Setters =
            {
                new Setter(FrameworkElement.MinHeightProperty, minHeight),
                new Setter(Control.PaddingProperty, padding),
                new Setter(Control.HorizontalContentAlignmentProperty, HorizontalAlignment.Stretch),
            },
        };

    internal static void Name(DependencyObject element, string name) => AutomationProperties.SetName(element, name);

    internal static MenuFlyoutItem MenuItem(string text, string? glyph, Action click, string? accelerator = null)
    {
        var item = new MenuFlyoutItem { Text = text, KeyboardAcceleratorTextOverride = accelerator ?? "" };
        if (glyph is not null) item.Icon = new FontIcon { Glyph = glyph };
        item.Click += (_, _) => click();
        return item;
    }

    internal static KeyboardAccelerator Accelerator(VirtualKey key, VirtualKeyModifiers modifiers, Action invoke)
    {
        var accelerator = new KeyboardAccelerator { Key = key, Modifiers = modifiers };
        accelerator.Invoked += (_, args) => { args.Handled = true; invoke(); };
        return accelerator;
    }

    internal static void SetVisible(UIElement element, bool visible) =>
        element.Visibility = visible ? Visibility.Visible : Visibility.Collapsed;

    internal static void SetText(TextBox box, string value)
    {
        // Rewriting identical text would move the caret while the user types.
        if (box.Text != value) box.Text = value;
    }

    /// WinUI permits one ContentDialog per XamlRoot, so dialogs queue behind each other.
    internal static async Task<ContentDialogResult> Show(this ContentDialog dialog, FrameworkElement owner)
    {
        await DialogGate.WaitAsync();
        try
        {
            dialog.XamlRoot = owner.XamlRoot;
            dialog.Style = Style("DefaultContentDialogStyle");
            dialog.RequestedTheme = ((FrameworkElement)owner.XamlRoot.Content).RequestedTheme;
            return await dialog.ShowAsync();
        }
        finally { DialogGate.Release(); }
    }

    /// A theme brush for inline Runs, which cannot take a Style. Callers re-render on ActualThemeChanged.
    internal static Brush ThemeBrush(FrameworkElement element, string key)
    {
        string[] themes = element.ActualTheme == ElementTheme.Dark ? ["Dark", "Default"] : ["Light"];
        if (new Windows.UI.ViewManagement.AccessibilitySettings().HighContrast) themes = ["HighContrast"];
        foreach (var dictionary in Dictionaries(Application.Current.Resources))
            foreach (var theme in themes)
                if (dictionary.ThemeDictionaries.TryGetValue(theme, out var themed)
                    && themed is ResourceDictionary resources && resources.TryGetValue(key, out var value) && value is Brush brush)
                    return brush;
        return (Brush)Application.Current.Resources[key];
    }

    private static IEnumerable<ResourceDictionary> Dictionaries(ResourceDictionary dictionary)
    {
        yield return dictionary;
        foreach (var merged in dictionary.MergedDictionaries)
            foreach (var nested in Dictionaries(merged))
                yield return nested;
    }

    internal static Color Tint(this FirmwareFormat format) =>
        format.IsElf ? Color.FromArgb(255, 0x87, 0x64, 0xB8) : format.IsHex ? Color.FromArgb(255, 0xCA, 0x50, 0x10) : Color.FromArgb(255, 0x03, 0x83, 0x87);

    internal static Color Tint(this MemoryRegionKind kind) =>
        kind.IsNvm ? Color.FromArgb(255, 0xCA, 0x50, 0x10) : kind.IsRam ? Color.FromArgb(255, 0x03, 0x83, 0x87) : Color.FromArgb(255, 0x7A, 0x75, 0x74);

    internal static SolidColorBrush Brush(Color color, double opacity = 1) => new(color) { Opacity = opacity };
}
