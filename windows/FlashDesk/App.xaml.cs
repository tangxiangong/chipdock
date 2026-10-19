using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;

namespace FlashDesk;

public partial class App : Application
{
    private MainWindow? main;

    public App()
    {
        UnhandledException += (_, args) => CrashLog.Write(args.Exception, args.Message);
        InitializeComponent();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        var dispatcher = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
        main = new MainWindow();
        // A second launch, e.g. opening a firmware from Explorer, is redirected here by Program.Main.
        AppInstance.GetCurrent().Activated += (_, activation) => dispatcher.TryEnqueue(() =>
        {
            if (main is null) return;
            main.Activate();
            if (Program.FirmwareArgument(activation) is { } path) main.Store.SelectFirmware(path);
        });
        main.Activate();
        if (Program.FirmwareArgument(Environment.GetCommandLineArgs().Skip(1)) is { } firmware) main.Store.SelectFirmware(firmware);
    }
}
