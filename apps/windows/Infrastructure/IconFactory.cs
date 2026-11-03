using System.Drawing;
using System.IO;
using Moonglow.Native;

namespace Moonglow.Infrastructure;

/// <summary>Loads the amber logo used by the executable, installer, and macOS app.</summary>
internal static class IconFactory
{
    private const int SM_CXSMICON = 49;

    public static Icon CreateTrayIcon()
        => LoadIcon(Math.Max(16, NativeMethods.GetSystemMetrics(SM_CXSMICON)));

    public static System.Windows.Media.ImageSource CreateImageSource()
    {
        using Icon icon = LoadIcon(64);
        var source = System.Windows.Interop.Imaging.CreateBitmapSourceFromHIcon(
            icon.Handle,
            System.Windows.Int32Rect.Empty,
            System.Windows.Media.Imaging.BitmapSizeOptions.FromEmptyOptions());
        source.Freeze();
        return source;
    }

    private static Icon LoadIcon(int size)
    {
        using Stream stream = typeof(IconFactory).Assembly.GetManifestResourceStream("Moonglow.Icon.ico")
            ?? throw new InvalidOperationException("The Moonglow icon resource is missing.");
        using var icon = new Icon(stream, new Size(size, size));
        return (Icon)icon.Clone();
    }
}
