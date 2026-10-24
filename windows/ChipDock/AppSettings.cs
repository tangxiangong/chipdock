using System.Text.Json;

namespace ChipDock;

/// Native preferences: appearance, the flash defaults for new sessions and update scheduling.
/// Profiles and recent firmware are stored by the Rust Core instead.
internal static class AppSettings
{
    private static readonly string FilePath =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "ChipDock", "settings.json");
    private static readonly Dictionary<string, string> Values = Load();

    /// The Rust Core's profile and recent-file directory, alongside the user's roaming data.
    internal static string StorageDirectory { get; } =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "ChipDock");

    internal static Appearance Appearance
    {
        get => Enum.TryParse<Appearance>(Values.GetValueOrDefault("appearance"), out var value) ? value : Appearance.System;
        set => Set("appearance", value.ToString());
    }

    internal static bool DefaultVerify
    {
        get => !bool.TryParse(Values.GetValueOrDefault("defaultVerify"), out var value) || value;
        set => Set("defaultVerify", value.ToString());
    }

    internal static bool DefaultReset
    {
        get => !bool.TryParse(Values.GetValueOrDefault("defaultReset"), out var value) || value;
        set => Set("defaultReset", value.ToString());
    }

    // Like Sparkle's SUEnableAutomaticChecks and SUAutomaticallyUpdate on macOS, both default to on.
    internal static bool AutomaticUpdateChecks
    {
        get => !bool.TryParse(Values.GetValueOrDefault("automaticUpdateChecks"), out var value) || value;
        set => Set("automaticUpdateChecks", value.ToString());
    }

    internal static bool AutomaticUpdateDownloads
    {
        get => !bool.TryParse(Values.GetValueOrDefault("automaticUpdateDownloads"), out var value) || value;
        set => Set("automaticUpdateDownloads", value.ToString());
    }

    /// The version a background download prepared; only that exact package may be installed unattended.
    internal static string PendingUpdate
    {
        get => Values.GetValueOrDefault("pendingUpdate") ?? "";
        set => Set("pendingUpdate", value);
    }

    internal static DateTimeOffset? LastUpdateCheck
    {
        get => long.TryParse(Values.GetValueOrDefault("lastUpdateCheck"), out var value) ? DateTimeOffset.FromUnixTimeMilliseconds(value) : null;
        set => Set("lastUpdateCheck", value?.ToUnixTimeMilliseconds().ToString() ?? "");
    }

    private static Dictionary<string, string> Load()
    {
        try { return JsonSerializer.Deserialize<Dictionary<string, string>>(File.ReadAllText(FilePath)) ?? new(); }
        catch (JsonException)
        {
            // Keep malformed preferences for recovery without blocking startup.
            try { File.Move(FilePath, FilePath + ".corrupt-" + Guid.NewGuid().ToString("N")); }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException) { }
            return new();
        }
        // Missing or unreadable preferences fall back to defaults; this runs before the window exists.
        catch (Exception error) when (error is IOException or UnauthorizedAccessException) { return new(); }
    }

    private static void Set(string key, string value)
    {
        var next = new Dictionary<string, string>(Values) { [key] = value };
        Directory.CreateDirectory(Path.GetDirectoryName(FilePath)!);
        var temporary = FilePath + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try
        {
            File.WriteAllText(temporary, JsonSerializer.Serialize(next));
            File.Move(temporary, FilePath, true);
        }
        finally { if (File.Exists(temporary)) File.Delete(temporary); }
        Values[key] = value;
    }
}
