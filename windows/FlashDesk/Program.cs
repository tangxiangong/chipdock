using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using Velopack;

namespace FlashDesk;

internal static class Program
{
    private const string InstanceKey = "FlashDesk";

    [STAThread]
    private static void Main(string[] args)
    {
        AppDomain.CurrentDomain.UnhandledException += (_, error) =>
        {
            if (error.ExceptionObject is Exception exception) CrashLog.Write(exception, "Unhandled managed exception");
        };
        if (args.Length > 0 && args[0] == ProbeDriver.InstallArgument)
        {
            Environment.ExitCode = ProbeDriver.RunInstaller(args[1..]);
            return;
        }
        // Installer hooks must finish before WinUI, single-instance registration or Rust startup.
        // Downloaded updates are applied by Updater, never implicitly at launch.
        VelopackApp.Build().SetAutoApplyOnStartup(false).Run();
        var prepareRuntimeOnly = args.Length == 1 && args[0] == "--prepare-runtime";
        try { WindowsRuntime.EnsureInstalled(); }
        catch (Exception error)
        {
            if (prepareRuntimeOnly) Console.Error.WriteLine(error);
            else WindowsRuntime.ShowError(error);
            Environment.ExitCode = 1;
            return;
        }
        try { if (!prepareRuntimeOnly) StartApplication(); }
        finally { WindowsRuntime.Shutdown(); }
    }

    // Do not resolve WinUI types while JIT-compiling the runtime bootstrap entry point.
    [MethodImpl(MethodImplOptions.NoInlining)]
    private static void StartApplication()
    {
        WinRT.ComWrappersSupport.InitializeComWrappers();
        // One window owns the probe; later launches hand their firmware argument to it.
        var instance = AppInstance.FindOrRegisterForKey(InstanceKey);
        if (!instance.IsCurrent)
        {
            Redirect(instance, AppInstance.GetCurrent().GetActivatedEventArgs());
            return;
        }
        // Only the instance that owns the probe may replace the installation.
        Updater.ApplyPending();
        Application.Start(parameters =>
        {
            SynchronizationContext.SetSynchronizationContext(
                new DispatcherQueueSynchronizationContext(DispatcherQueue.GetForCurrentThread()));
            _ = new App();
        });
    }

    /// Redirection must not block the STA without pumping COM, so the wait uses CoWaitForMultipleObjects.
    private static void Redirect(AppInstance instance, AppActivationArguments activation)
    {
        using var redirected = new ManualResetEvent(false);
        _ = Task.Run(() =>
        {
            try { instance.RedirectActivationToAsync(activation).AsTask().Wait(); }
            finally { redirected.Set(); }
        });
        CoWaitForMultipleObjects(0, uint.MaxValue, 1, [redirected.SafeWaitHandle.DangerousGetHandle()], out _);
    }

    internal static readonly string[] FirmwareExtensions = [".elf", ".axf", ".hex", ".bin"];

    internal static bool IsFirmwarePath(string path) =>
        FirmwareExtensions.Contains(Path.GetExtension(path), StringComparer.OrdinalIgnoreCase);

    internal static string? FirmwareArgument(IEnumerable<string> arguments) =>
        arguments.FirstOrDefault(argument => IsFirmwarePath(argument) && File.Exists(argument)) is { } path
            ? Path.GetFullPath(path)
            : null;

    internal static string? FirmwareArgument(AppActivationArguments activation)
    {
        if (activation.Kind != ExtendedActivationKind.Launch
            || activation.Data is not Windows.ApplicationModel.Activation.ILaunchActivatedEventArgs launch)
            return null;
        return FirmwareArgument(SplitCommandLine(launch.Arguments));
    }

    /// Splits a redirected command line; quoted paths may contain spaces.
    private static IEnumerable<string> SplitCommandLine(string? commandLine)
    {
        if (string.IsNullOrWhiteSpace(commandLine)) yield break;
        var current = new System.Text.StringBuilder();
        var quoted = false;
        foreach (var character in commandLine)
        {
            if (character == '"') { quoted = !quoted; continue; }
            if (char.IsWhiteSpace(character) && !quoted)
            {
                if (current.Length > 0) { yield return current.ToString(); current.Clear(); }
                continue;
            }
            current.Append(character);
        }
        if (current.Length > 0) yield return current.ToString();
    }

    [DllImport("ole32.dll", ExactSpelling = true)]
    private static extern int CoWaitForMultipleObjects(uint flags, uint timeout, uint count, nint[] handles, out uint index);
}
