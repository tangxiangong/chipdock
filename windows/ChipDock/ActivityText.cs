using System.Text.RegularExpressions;

namespace ChipDock;

internal static class ActivityText
{
    internal static string Message(string text, bool hasProgress) => hasProgress
        ? Regex.Replace(text, @"\s*[（(]\d+%[）)]\s*$", "")
        : text;

    internal static string Percentage(float value) => $"{MathF.Round(value * 100, MidpointRounding.AwayFromZero):0}%";
}
