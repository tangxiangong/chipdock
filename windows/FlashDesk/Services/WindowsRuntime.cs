using System.Diagnostics;
using System.Net.Http;
using System.Runtime.InteropServices;
using System.Security.Cryptography;

namespace FlashDesk;

// Runs before loading WinUI. An existing compatible runtime is reused unchanged.
internal static class WindowsRuntime
{
    // Keep in sync with Microsoft.WindowsAppSDK.Runtime in the project file.
    internal const string Version = "2.5.1";
    private const uint MajorMinor = 0x00020005;
    private const ulong MinimumVersion = 0x0002000500010000;

    internal static (string Url, string Hash) Installer(Architecture architecture) => architecture switch
    {
        Architecture.X64 => (
            "https://download.microsoft.com/download/de664922-b046-432f-bb2a-54aec3c5f1f6/WindowsAppRuntimeInstall-x64.exe",
            "931a421e8dc3e6e67724806cb67fecdbb88dfe323f0170842eb4a4b4b149f1e2"),
        Architecture.Arm64 => (
            "https://download.microsoft.com/download/83fdef97-1ca8-491d-80f3-a94b5adc5b1c/WindowsAppRuntimeInstall-arm64.exe",
            "d5e4d34547eb4e31c64bf1532415b3019c0d92b750e72eb18d3d95bd00feacbb"),
        _ => throw new PlatformNotSupportedException($"不支持的处理器架构：{architecture}")
    };

    internal static void EnsureInstalled()
    {
        using var mutex = new Mutex(false, @"Local\FlashDesk.WindowsAppRuntime.Install");
        var acquired = false;
        try
        {
            try { acquired = mutex.WaitOne(TimeSpan.FromMinutes(10)); }
            catch (AbandonedMutexException) { acquired = true; }
            if (!acquired) throw new TimeoutException("Windows App Runtime 仍在安装。");
            Ensure(() => Initialize(MajorMinor, "", MinimumVersion, 0), Install);
        }
        finally { if (acquired) mutex.ReleaseMutex(); }
    }

    internal static void Ensure(Func<int> initialize, Action install)
    {
        var result = initialize();
        if (result >= 0) return;
        // MddBootstrap reports this HRESULT when no compatible framework is registered.
        // Permission, corruption and unsupported-process failures must not trigger installation.
        if (result != unchecked((int)0x80670016)) Marshal.ThrowExceptionForHR(result);
        install();
        result = initialize();
        if (result < 0) Marshal.ThrowExceptionForHR(result);
    }

    private static void Install()
    {
        // The ARM64 installer also registers x64 framework packages for emulated apps.
        var installer = Installer(RuntimeInformation.OSArchitecture);
        var directory = Directory.CreateTempSubdirectory("FlashDesk-runtime-");
        try
        {
            var path = Path.Combine(directory.FullName, "WindowsAppRuntimeInstall.exe");
            using (var client = new HttpClient { Timeout = TimeSpan.FromMinutes(5) })
            using (var response = client.GetAsync(installer.Url, HttpCompletionOption.ResponseHeadersRead).GetAwaiter().GetResult())
            {
                response.EnsureSuccessStatusCode();
                using var cancellation = new CancellationTokenSource(TimeSpan.FromMinutes(5));
                using var output = File.Create(path);
                response.Content.CopyToAsync(output, cancellation.Token).GetAwaiter().GetResult();
            }
            VerifyInstaller(path, installer.Hash);
            using var process = Process.Start(new ProcessStartInfo(path, "--quiet") { UseShellExecute = false, CreateNoWindow = true })
                ?? throw new IOException("无法启动 Windows App Runtime 安装程序。");
            // Do not kill an in-flight system installation: wait for its final result.
            process.WaitForExit();
            if (process.ExitCode != 0)
                throw new InvalidOperationException($"Windows App Runtime 安装失败（0x{process.ExitCode:X8}）。");
        }
        finally
        {
            try { directory.Delete(true); }
            catch (IOException) { }
            catch (UnauthorizedAccessException) { }
        }
    }

    internal static void VerifyInstaller(string path, string expectedHash)
    {
        using var stream = File.OpenRead(path);
        if (!CryptographicOperations.FixedTimeEquals(SHA256.HashData(stream), Convert.FromHexString(expectedHash)))
            throw new InvalidDataException("Windows App Runtime 安装程序未通过完整性校验。");
    }

    internal static void ShowError(Exception error) =>
        MessageBox(0, "无法安装或初始化 Windows App Runtime。请检查网络后重新启动 ChipDock。\n\n" + error.Message, "ChipDock", 0x10);

    [DllImport("Microsoft.WindowsAppRuntime.Bootstrap.dll", EntryPoint = "MddBootstrapInitialize2", CharSet = CharSet.Unicode, ExactSpelling = true)]
    private static extern int Initialize(uint majorMinor, string versionTag, ulong minimumVersion, uint options);

    [DllImport("Microsoft.WindowsAppRuntime.Bootstrap.dll", EntryPoint = "MddBootstrapShutdown", ExactSpelling = true)]
    internal static extern void Shutdown();

    [DllImport("user32.dll", EntryPoint = "MessageBoxW", CharSet = CharSet.Unicode, ExactSpelling = true)]
    private static extern int MessageBox(nint owner, string text, string caption, uint type);
}
