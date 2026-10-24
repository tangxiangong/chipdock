using System.Drawing;

namespace ChipDock;

internal static class WindowLayout
{
    internal static Rectangle Fit(Rectangle window, Rectangle workArea)
    {
        var width = Math.Clamp(window.Width, 1, Math.Max(1, workArea.Width));
        var height = Math.Clamp(window.Height, 1, Math.Max(1, workArea.Height));
        return new Rectangle(
            Math.Clamp(window.X, workArea.Left, workArea.Right - width),
            Math.Clamp(window.Y, workArea.Top, workArea.Bottom - height), width, height);
    }
}
