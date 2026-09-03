using System.Windows;
using System.Windows.Media;
using System.Windows.Media.Imaging;

namespace DisplayController.Infrastructure;

/// <summary>Draws the "DC" sun/moon logo and rasterizes it for use as the window icon.</summary>
internal static class LogoFactory
{
    private static ImageSource? _icon;

    public static ImageSource CreateIcon()
    {
        if (_icon is not null)
            return _icon;

        var image = new DrawingImage(BuildDrawing());
        image.Freeze();

        const int size = 256;
        var visual = new DrawingVisual();
        using (DrawingContext dc = visual.RenderOpen())
            dc.DrawImage(image, new Rect(0, 0, size, size));

        var bitmap = new RenderTargetBitmap(size, size, 96, 96, PixelFormats.Pbgra32);
        bitmap.Render(visual);
        bitmap.Freeze();

        _icon = bitmap;
        return _icon;
    }

    private static Drawing BuildDrawing()
    {
        var group = new DrawingGroup();

        // Square viewport keeps the mark centered with its proportions.
        group.Children.Add(new GeometryDrawing(Brushes.Transparent, null,
            new RectangleGeometry(new Rect(60, 70, 372, 372))));

        // "D" — golden sun: semicircle, flat edge left, bulging right.
        var d = new PathGeometry();
        var figure = new PathFigure { StartPoint = new Point(181, 160), IsClosed = true };
        figure.Segments.Add(new ArcSegment(new Point(181, 352), new Size(96, 96), 0, false,
            SweepDirection.Clockwise, true));
        d.Figures.Add(figure);
        var gold = new LinearGradientBrush(
            Color.FromRgb(0xF7, 0xCD, 0x4A), Color.FromRgb(0xD6, 0x94, 0x14), 90);
        gold.Freeze();
        group.Children.Add(new GeometryDrawing(gold, null, d));

        // "C" — silver crescent moon opening right (outer circle minus a right-offset bite).
        var outer = new EllipseGeometry(new Point(379, 256), 92, 92);
        var bite = new EllipseGeometry(new Point(423, 250), 92, 92);
        var crescent = new CombinedGeometry(GeometryCombineMode.Exclude, outer, bite);
        var silver = new LinearGradientBrush(
            Color.FromRgb(0xEE, 0xF0, 0xF3), Color.FromRgb(0xA6, 0xAB, 0xB5), 90);
        silver.Freeze();
        group.Children.Add(new GeometryDrawing(silver, null, crescent));

        group.Freeze();
        return group;
    }
}
