using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ChipDock;

internal sealed class SerialWorkbench : UserControl
{
    private readonly TabView tabs = new() { IsAddTabButtonVisible = true, CanDragTabs = false, CanReorderTabs = false };
    private readonly SerialView primary = new();
    private bool changing, closed, ipcStarted;
    private readonly DropDownButton control = new() { Content = "本地控制" };
    private readonly Microsoft.UI.Dispatching.DispatcherQueueTimer ipcTimer;
    private readonly MainWindow window;
    private readonly Dictionary<SerialPlotWindow, ulong> plots = [];

    internal SerialWorkbench(MainWindow window)
    {
        this.window = window;
        Content = tabs;
        tabs.TabStripFooter = control;
        ipcTimer = DispatcherQueue.CreateTimer();
        ipcTimer.Interval = TimeSpan.FromMilliseconds(150);
        ipcTimer.Tick += (_, _) => HandlePlotRequest();
        RefreshLocalControl();
        Add(primary);
        tabs.AddTabButtonClick += OnAdd;
        tabs.TabCloseRequested += OnClose;
        tabs.SelectionChanged += (_, _) => ActivityChanged?.Invoke();

    }

    private SerialView Active => (tabs.SelectedItem as TabViewItem)?.Content as SerialView ?? primary;
    private IEnumerable<SerialView> Sessions => tabs.TabItems.Cast<TabViewItem>().Select(tab => (SerialView)tab.Content);
    internal bool UpdateBlocked => changing || Sessions.Any(session => session.UpdateBlocked) || SerialBackend.IsBusy();
    internal bool CanSend => Active.CanSend;
    internal bool CanRefresh => Active.CanRefresh;
    internal bool CanDisconnect => Active.CanDisconnect;
    internal bool CanToggleConnection => Active.CanToggleConnection;
    internal Task RefreshPorts() => Active.RefreshPorts();
    internal Task ToggleConnection() => Active.ToggleConnection();
    internal Task Disconnect() => Active.Disconnect();
    internal Task Send() => Active.Send();
    internal void ClearTerminal() => Active.ClearTerminal();
    internal void Find() => Active.Find();
    internal event Action? ActivityChanged;
    internal event Func<byte[], Task>? ExportRequested;

    internal async Task Start()
    {
        if (closed || ipcStarted) return;
        ipcStarted = true;
        await primary.Start();
        await ConfigureLocalControl(SerialPreferences.Current.LocalControl);
        if (!closed) ipcTimer.Start();
    }

    private async Task ConfigureLocalControl(byte mode)
    {
        control.IsEnabled = false;
        try
        {
            await SerialBackend.ConfigureLocalControl(mode);
            SerialPreferences.Current.LocalControl = mode;
            SerialPreferences.Current.Save();
        }
        catch (Exception error) { if (!closed) primary.ShowError(error.Message); }
        finally { RefreshLocalControl(); control.IsEnabled = true; }
    }

    private void RefreshLocalControl()
    {
        var menu = new MenuFlyout();
        var labels = new[] { "关闭", "只读", "允许控制" };
        for (byte index = 0; index < labels.Length; index++)
        {
            var mode = index;
            var item = new ToggleMenuFlyoutItem { Text = labels[index], IsChecked = SerialPreferences.Current.LocalControl == mode };
            item.Click += async (_, _) => await ConfigureLocalControl(mode);
            menu.Items.Add(item);
        }
        control.Flyout = menu;
    }

    private void HandlePlotRequest()
    {
        if (closed) return;
        try
        {
            var request = SerialBackend.TakePlot();
            if (request.id == 0) return;
            string? failure = null;
            try { ShowPlot(request.session_id); } catch (Exception error) { failure = error.Message; }
            SerialBackend.FinishPlot(request.id, failure);
        }
        catch (Exception error) { primary.ShowError(error.Message); }
    }

    private void Add(SerialView session)
    {
        session.PickExport = window.PickSerialExport;
        session.PickFolder = window.PickSerialFolder;
        session.PickFile = window.PickSerialFile;
        session.OpenPlot = ShowPlot;
        var tab = new TabViewItem { Content = session, Header = session.SessionTitle, IsClosable = session.SessionId != 1 };
        session.ActivityChanged += () =>
        {
            tab.Header = session.SessionTitle;
            ActivityChanged?.Invoke();
        };
        session.ExportRequested += data => ExportRequested?.Invoke(data) ?? Task.CompletedTask;
        tabs.TabItems.Add(tab);
        tabs.SelectedItem = tab;
    }

    private async void OnAdd(TabView sender, object args)
    {
        if (changing || closed || tabs.TabItems.Count >= 16) return;
        changing = true;
        tabs.IsAddTabButtonVisible = false;
        var source = Active;
        var preferences = source.CopyPreferences();
        try
        {
            var id = await SerialBackend.Create();
            if (closed) { await SerialBackend.Close(id); return; }
            var view = new SerialView(id, preferences);
            Add(view);
            await view.Start();
        }
        catch (Exception error) { if (!closed) source.ShowError(error.Message); }
        finally
        {
            changing = false;
            tabs.IsAddTabButtonVisible = tabs.TabItems.Count < 16;
            ActivityChanged?.Invoke();
        }
    }

    private async void OnClose(TabView sender, TabViewTabCloseRequestedEventArgs args)
    {
        if (changing || closed || args.Tab.Content is not SerialView session || session.SessionId == 1) return;
        changing = true;
        session.Stop();
        try { await SerialBackend.Close(session.SessionId); }
        catch (Exception error) { primary.ShowError(error.Message); }
        finally
        {
            tabs.TabItems.Remove(args.Tab);
            foreach (var plot in plots.Where(item => item.Value == session.SessionId).Select(item => item.Key).ToArray()) plot.Close();
            changing = false;
            tabs.IsAddTabButtonVisible = true;
            ActivityChanged?.Invoke();
        }
    }

    internal void ShowPlot(ulong id)
    {
        if (closed || !Sessions.Any(session => session.SessionId == id)) throw new InvalidOperationException("串口会话不存在");
        var plot = new SerialPlotWindow(id, window.PickSerialExport);
        plots.Add(plot, id);
        plot.Closed += (_, _) => plots.Remove(plot);
        plot.Activate();
    }

    internal void Shutdown()
    {
        closed = true;
        ipcTimer.Stop();
        foreach (var plot in plots.Keys.ToArray()) plot.Close();
        foreach (var session in Sessions) session.Stop();
        try { SerialBackend.Shutdown(); }
        catch (Exception error) { System.Diagnostics.Trace.TraceError("{0}", error); }
    }
}
