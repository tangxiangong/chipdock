using System.Reflection;
using System.Runtime.InteropServices;
using ChipDock.Tests;

// Usage: ChipDock.Tests [path to the built chipdock_csharp library]
var driverRulesOnly = args.SequenceEqual(["--driver-rules-only"]);
if (!driverRulesOnly)
{
    await WorkbenchTests.Run();
    Console.WriteLine($"Portable workbench logic checks passed ({Check.Count} assertions).");
    if (args.SequenceEqual(["--workbench-only"])) return;
}
ProbeDriverRulesTests.Run();
await ProbeDriverTests.RunSimulation();
Console.WriteLine($"Portable USB driver logic checks passed ({Check.Count} assertions).");
if (driverRulesOnly) return;
UpdateFeedTests.Run();
await UpdateSourceTests.Run();
Console.WriteLine($"Update checks passed ({Check.Count} assertions).");
if (OperatingSystem.IsWindows())
{
    var driverBefore = Check.Count;
    ProbeDriverTests.RunNative();
    Console.WriteLine($"USB probe driver checks passed ({Check.Count - driverBefore} assertions).");
}

if (args.Length == 0)
{
    Console.WriteLine("Bridge checks skipped: pass the built chipdock_csharp library to run them.");
    return;
}
var library = Path.GetFullPath(args[0]);
if (!File.Exists(library)) throw new FileNotFoundException("Rust bridge library not found", library);
NativeLibrary.SetDllImportResolver(Assembly.GetExecutingAssembly(), (name, assembly, searchPath) =>
    name == "chipdock_csharp" ? NativeLibrary.Load(library) : nint.Zero);
var before = Check.Count;
await BridgeTests.Run();
if (OperatingSystem.IsWindows()) await ProbeDriverTests.CheckProbeSelectors();
Console.WriteLine($"Bridge checks passed ({Check.Count - before} assertions).");

namespace ChipDock.Tests
{
    internal static class Check
    {
        internal static int Count { get; private set; }

        internal static void That(bool condition, string name)
        {
            if (!condition) throw new InvalidOperationException("FAIL: " + name);
            Count++;
        }

        internal static void Throws<T>(Action action, string name, string? message = null) where T : Exception
        {
            try { action(); }
            catch (T error)
            {
                That(message is null || error.Message.Contains(message, StringComparison.Ordinal), $"{name}: {error.Message}");
                return;
            }
            throw new InvalidOperationException($"FAIL: {name}: expected {typeof(T).Name}");
        }

        internal static async Task ThrowsAsync<T>(Func<Task> action, string name, string? message = null) where T : Exception
        {
            try { await action(); }
            catch (T error)
            {
                That(message is null || error.Message.Contains(message, StringComparison.Ordinal), $"{name}: {error.Message}");
                return;
            }
            throw new InvalidOperationException($"FAIL: {name}: expected {typeof(T).Name}");
        }
    }
}
