using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ChipDock;

internal sealed class SaveProfileDialog : ContentDialog
{
    private readonly TextBox name = new() { Header = "配置名称" };

    internal SaveProfileDialog(WorkbenchStore store)
    {
        Title = "保存烧录配置";
        PrimaryButtonText = "保存";
        CloseButtonText = "取消";
        DefaultButton = ContentDialogButton.Primary;
        name.Text = store.EffectiveChip.Length == 0 ? "烧录配置" : store.EffectiveChip;
        name.TextChanged += (_, _) => IsPrimaryButtonEnabled = name.Text.Trim().Length > 0;

        var summary = new Grid { ColumnSpacing = 16, RowSpacing = 6 };
        summary.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
        summary.ColumnDefinitions.Add(new());
        var rows = new[]
        {
            ("目标", store.TargetName),
            ("接口", $"{store.WireProtocol.Title()} · {store.Speed} kHz"),
            ("擦除", store.EraseStrategy.Title()),
        };
        for (var index = 0; index < rows.Length; index++)
        {
            summary.RowDefinitions.Add(new() { Height = GridLength.Auto });
            var label = Ui.Secondary(rows[index].Item1, 13);
            var value = Ui.Text(rows[index].Item2, 13);
            Grid.SetRow(label, index);
            Grid.SetRow(value, index);
            Grid.SetColumn(value, 1);
            summary.Children.Add(label);
            summary.Children.Add(value);
        }
        var content = Ui.Column(16,
            Ui.Secondary("保存探针、目标芯片、连接参数与烧录选项。", 13, wrap: true),
            name,
            new Border { Style = Ui.Style("CardStyle"), Padding = new Thickness(12), Child = summary });
        content.MinWidth = 400;
        Content = content;

        PrimaryButtonClick += async (_, args) =>
        {
            // Close either way: a storage failure is reported in its own dialog.
            var deferral = args.GetDeferral();
            IsPrimaryButtonEnabled = false;
            await store.SaveProfile(name.Text);
            deferral.Complete();
        };
        Opened += (_, _) => { name.Focus(FocusState.Programmatic); name.SelectAll(); };
    }
}
