using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Linq;
using Moonglow.Native;

namespace Moonglow.Infrastructure;

/// <summary>
/// Draws the Moonglow crescent at runtime: a black moon with a white keyline, the same
/// two-tone mark as the packaged icon, in every theme. Geometry mirrors
/// tools/generate-logo-assets.ps1.
/// </summary>
internal static class IconFactory
{
    private const int Design = 1024;
    private const int Supersample = 4;
    private const int WindowSize = 64;
    private const int SM_CXSMICON = 49;

    private const float CrescentRadius = 480f;
    private const float CutAngleDegrees = -45f;
    private const float CutRadiusFactor = 1.05f;
    private const float CrescentThickness = 0.60f;

    /// <summary>Creates the tray icon: the two-tone mark, identical in every theme.</summary>
    public static Icon CreateTrayIcon()
    {
        // Draw at the shell's small-icon size so the tray does not have to rescale the bitmap.
        int size = Math.Max(16, NativeMethods.GetSystemMetrics(SM_CXSMICON));
        using Bitmap bitmap = DrawMark(size);
        return ToIcon(bitmap);
    }

    /// <summary>Rasterizes the mark as a WPF image for use as the window/taskbar icon.</summary>
    public static System.Windows.Media.ImageSource CreateImageSource()
    {
        using Bitmap bitmap = DrawMark(WindowSize);
        using Icon icon = ToIcon(bitmap);
        var source = System.Windows.Interop.Imaging.CreateBitmapSourceFromHIcon(
            icon.Handle,
            System.Windows.Int32Rect.Empty,
            System.Windows.Media.Imaging.BitmapSizeOptions.FromEmptyOptions());
        source.Freeze();
        return source;
    }

    private static Icon ToIcon(Bitmap bitmap)
    {
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

    private static Bitmap DrawMark(int size)
    {
        int large = size * Supersample;
        float scale = large / (float)Design;
        var result = new Bitmap(size, size, PixelFormat.Format32bppArgb);

        using (var canvas = new Bitmap(large, large, PixelFormat.Format32bppArgb))
        {
            using (var g = Graphics.FromImage(canvas))
            {
                g.SmoothingMode = SmoothingMode.AntiAlias;
                g.PixelOffsetMode = PixelOffsetMode.HighQuality;
                g.CompositingQuality = CompositingQuality.HighQuality;
                g.Clear(Color.Transparent);

                using var path = CreateCrescentPath(scale); // Keyline mirrors the generator, but tiny icons exaggerate it 2.5x so the ring stays white instead of blurring to gray.
                using var keyPen = new Pen(Color.White, (size <= 32 ? 2.5f : 1f) * Math.Max(24f, 1.15f * Design / size) * scale) { LineJoin = LineJoin.Round };
                g.DrawPath(keyPen, path); using var brush = new SolidBrush(Color.Black); g.FillPath(brush, path);
            }

            using (var g = Graphics.FromImage(result))
            {
                g.InterpolationMode = InterpolationMode.HighQualityBicubic;
                g.PixelOffsetMode = PixelOffsetMode.HighQuality;
                g.CompositingMode = CompositingMode.SourceCopy;
                g.DrawImage(canvas, new Rectangle(0, 0, size, size));
            }
        }

        return result;
    }

    /// <summary>
    /// The crescent: the disc rim on the side away from the cut, then the rim of the cut circle
    /// back to where it started. The finished outline is shifted so its bounding box is centred,
    /// which is what keeps the mark optically centred without the old rays.
    /// </summary>
    private static GraphicsPath CreateCrescentPath(float scale)
    {
        float center = Design / 2f * scale;
        float radius = CrescentRadius * scale;
        float cutRadius = radius * CutRadiusFactor;
        float distance = cutRadius - radius + radius * CrescentThickness;
        double theta = CutAngleDegrees * Math.PI / 180.0;

        float along = (distance * distance + radius * radius - cutRadius * cutRadius) / (2f * distance);
        float halfChord = (float)Math.Sqrt(Math.Max(0.0, radius * radius - along * along));
        float cutX = center + (float)Math.Cos(theta) * distance;
        float cutY = center + (float)Math.Sin(theta) * distance;

        float beta = (float)(Math.Atan2(halfChord, along) * 180.0 / Math.PI);
        float gamma = (float)(Math.Atan2(halfChord, along - distance) * 180.0 / Math.PI);

        var points = ArcPoints(center, center, radius, CutAngleDegrees + beta, CutAngleDegrees - beta, CutAngleDegrees + 180f);
        var inner = ArcPoints(cutX, cutY, cutRadius, CutAngleDegrees + gamma, CutAngleDegrees - gamma, CutAngleDegrees + 180f);
        // Walk the inner rim back to the start so the outline stays continuous.
        inner.Reverse();
        points.AddRange(inner.Skip(1).Take(inner.Count - 2));

        float minX = points.Min(p => p.X);
        float maxX = points.Max(p => p.X);
        float minY = points.Min(p => p.Y);
        float maxY = points.Max(p => p.Y);
        float shiftX = center - (minX + maxX) / 2f;
        float shiftY = center - (minY + maxY) / 2f;
        for (int i = 0; i < points.Count; i++)
            points[i] = new PointF(points[i].X + shiftX, points[i].Y + shiftY);

        var path = new GraphicsPath();
        path.AddPolygon(points.ToArray());
        path.CloseFigure();
        return path;
    }

    private static List<PointF> ArcPoints(float cx, float cy, float radius, float startDeg, float endDeg, float midDeg)
    {
        float clockwiseEnd = Normalize(endDeg - startDeg);
        float clockwiseMid = Normalize(midDeg - startDeg);
        float sweep = clockwiseMid <= clockwiseEnd ? clockwiseEnd : clockwiseEnd - 360f;

        int steps = Math.Max(8, (int)Math.Ceiling(Math.Abs(sweep)));
        var points = new List<PointF>(steps + 1);
        for (int i = 0; i <= steps; i++)
        {
            double angle = (startDeg + sweep * i / steps) * Math.PI / 180.0;
            points.Add(new PointF(cx + (float)Math.Cos(angle) * radius, cy + (float)Math.Sin(angle) * radius));
        }

        return points;
    }

    private static float Normalize(float angle) => (angle % 360f + 360f) % 360f;
}
