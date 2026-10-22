using System.Drawing;
using System.Drawing.Drawing2D;
using System.IO;
using System.Windows.Media.Imaging;
using Moonglow.Native;

namespace Moonglow.Infrastructure;

/// <summary>Supplies the app logo for the tray and window from the packaged artwork.</summary>
internal static class IconFactory
{
    private const string FullLogo = "Moonglow.Assets.Moonglow.png";
    private const string TrayLogo = "Moonglow.Assets.Moonglow-small.png";

    /// <summary>Renders the tray icon. The crescent-only artwork keeps it legible at 16 px.</summary>
    public static Icon CreateTrayIcon()
    {
        const int size = 32;
        using Stream stream = OpenLogo(TrayLogo);
        using var logo = new Bitmap(stream);
        using var bitmap = new Bitmap(size, size);
        using (var graphics = Graphics.FromImage(bitmap))
        {
            graphics.Clear(Color.Transparent);
            graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
            graphics.PixelOffsetMode = PixelOffsetMode.HighQuality;
            graphics.SmoothingMode = SmoothingMode.AntiAlias;
            graphics.DrawImage(logo, new Rectangle(0, 0, size, size));
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

    /// <summary>Loads the full-resolution logo for the window and taskbar icon.</summary>
    public static System.Windows.Media.ImageSource CreateImageSource()
    {
        using Stream stream = OpenLogo(FullLogo);
        var image = new BitmapImage();
        image.BeginInit();
        image.StreamSource = stream;
        image.CacheOption = BitmapCacheOption.OnLoad;
        image.EndInit();
        image.Freeze();
        return image;
    }

    private static Stream OpenLogo(string name)
        => typeof(IconFactory).Assembly.GetManifestResourceStream(name)
            ?? throw new InvalidOperationException($"Missing embedded logo resource: {name}");
}
