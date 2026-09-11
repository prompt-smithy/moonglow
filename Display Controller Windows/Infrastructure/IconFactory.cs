using System.Drawing;
using System.Drawing.Drawing2D;
using Amber.Native;

namespace Amber.Infrastructure;

/// <summary>Draws the system-tray icon at runtime so no binary asset is required.</summary>
internal static class IconFactory
{
    public static Icon CreateTrayIcon()
    {
        const int size = 32;
        using var bitmap = new Bitmap(size, size);
        using (var g = Graphics.FromImage(bitmap))
        {
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.Clear(Color.Transparent);

            float cx = size / 2f;
            float cy = size / 2f;
            var color = Color.FromArgb(255, 255, 196, 64);

            using var pen = new Pen(color, 2.4f) { StartCap = LineCap.Round, EndCap = LineCap.Round };
            for (int i = 0; i < 8; i++)
            {
                double a = i * Math.PI / 4;
                float x1 = cx + (float)Math.Cos(a) * 11.5f;
                float y1 = cy + (float)Math.Sin(a) * 11.5f;
                float x2 = cx + (float)Math.Cos(a) * 15f;
                float y2 = cy + (float)Math.Sin(a) * 15f;
                g.DrawLine(pen, x1, y1, x2, y2);
            }

            using var brush = new SolidBrush(color);
            g.FillEllipse(brush, cx - 8, cy - 8, 16, 16);
        }

        IntPtr hIcon = bitmap.GetHicon();
        try
        {
            using var temp = Icon.FromHandle(hIcon);
            return (Icon)temp.Clone();
        }
        finally
        {
            NativeMethods.DestroyIcon(hIcon);
        }
    }

    /// <summary>Rasterizes the tray icon as a WPF image for use as the window/taskbar icon.</summary>
    public static System.Windows.Media.ImageSource CreateImageSource()
    {
        using Icon icon = CreateTrayIcon();
        var source = System.Windows.Interop.Imaging.CreateBitmapSourceFromHIcon(
            icon.Handle,
            System.Windows.Int32Rect.Empty,
            System.Windows.Media.Imaging.BitmapSizeOptions.FromEmptyOptions());
        source.Freeze();
        return source;
    }
}
