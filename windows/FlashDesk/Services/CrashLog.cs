namespace FlashDesk;

/// Keeps startup/XAML failures diagnosable even before the activity log exists.
internal static class CrashLog
{
    internal static void Write(Exception error, string context)
    {
        try
        {
            var directory = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "FlashDesk", "logs");
            Directory.CreateDirectory(directory);
            File.AppendAllText(Path.Combine(directory, "crash.log"),
                $"{DateTimeOffset.UtcNow:O} {context}{Environment.NewLine}{error}{Environment.NewLine}{Environment.NewLine}");
        }
        catch (Exception) { /* Diagnostics must not replace the original failure. */ }
    }
}
