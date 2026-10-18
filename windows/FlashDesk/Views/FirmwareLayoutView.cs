using FlashDesk.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using static FlashDesk.Formatting;

namespace FlashDesk;

/// A proportional bar of the address ranges the firmware writes, over the Flash it touches.
internal sealed class FirmwareLayoutView : UserControl
{
    private readonly Canvas segments = new();
    private readonly Border track = new() { Style = Ui.Style("WellStyle"), Height = 8, CornerRadius = new CornerRadius(3) };
    private readonly TextBlock startLabel = Ui.Mono(size: 12, style: "SecondaryTextStyle");
    private readonly TextBlock endLabel = Ui.Mono(size: 12, style: "SecondaryTextStyle");
    private readonly TextBlock capacityLabel = Ui.Text(size: 12);
    private FirmwareUsage? usage;
    private ulong start, end;

    internal FirmwareLayoutView()
    {
        track.Child = segments;
        segments.SizeChanged += (_, _) => Layout();
        var labels = new Grid { ColumnSpacing = 8 };
        labels.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        labels.ColumnDefinitions.Add(new());
        labels.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        capacityLabel.HorizontalAlignment = HorizontalAlignment.Center;
        Grid.SetColumn(capacityLabel, 1);
        Grid.SetColumn(endLabel, 2);
        labels.Children.Add(startLabel);
        labels.Children.Add(capacityLabel);
        labels.Children.Add(endLabel);
        Content = Ui.Column(6, track, labels);
        AutomationProperties.SetName(track, "固件写入布局");
    }

    internal void Update(FirmwareUsage value, List<MemoryRegionLayout> regions)
    {
        usage = value;
        // Only the Flash regions the firmware touches, so distant OTP or system
        // memory does not squeeze the firmware into a sliver.
        var flash = regions.Where(region => region.kind.IsNvm && !region.is_alias).ToList();
        if (value.start_address is { } low && value.end_address is { } high)
            flash = flash.Where(region => region.start < high && region.end > low).ToList();
        start = Math.Min(value.start_address ?? 0, flash.Count > 0 ? flash.Min(region => region.start) : value.start_address ?? 0);
        end = Math.Max(value.end_address ?? 1, flash.Count > 0 ? flash.Max(region => region.end) : value.end_address ?? 1);
        var capacity = flash.Aggregate(0UL, (total, region) => total + region.size);

        startLabel.Text = AddressLabel(start);
        endLabel.Text = AddressLabel(end > 0 ? end - 1 : 0);
        capacityLabel.Text = capacity > 0 ? $"Flash {ByteCountLabel(capacity)} · 已用 {(double)value.used_bytes / capacity * 100:0.0}%" : "";
        capacityLabel.Style = Ui.Style(value.used_bytes > capacity ? "CriticalTextStyle" : "SecondaryTextStyle");
        AutomationProperties.SetHelpText(track, $"{value.segments.Count} 个区段，共 {ByteCountLabel(value.used_bytes)}");
        Layout();
    }

    private void Layout()
    {
        segments.Children.Clear();
        if (usage is null || segments.ActualWidth <= 0) return;
        var span = (double)Math.Max(end - start, 1);
        foreach (var segment in usage.segments)
        {
            var bar = new Border
            {
                Style = Ui.Style("AccentFillStyle"),
                Width = Math.Max(segments.ActualWidth * segment.size / span, 2),
                Height = 8,
            };
            Canvas.SetLeft(bar, segments.ActualWidth * (segment.start - start) / span);
            segments.Children.Add(bar);
        }
    }
}
