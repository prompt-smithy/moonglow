using System.Xml.Linq;
using Xunit;

namespace Moonglow.Core.Tests;

/// <summary>
/// Guards the warm-to-cool ramp used by the popup slider and the range slider.
/// The stops are the original Windows values: a saturated warm-to-cool run with
/// no grey dip in the middle. A two-stop macOS-style ramp interpolates through
/// grey, so saturation bottoms out mid-track and both ends feel stronger.
/// </summary>
public sealed class TemperatureGradientTests
{
    private static readonly (string Color, double Offset)[] ExpectedStops =
    {
        ("#E87925", 0),
        ("#F0B479", 0.48),
        ("#9BB9D7", 0.72),
        ("#5E91C2", 1),
    };

    [Fact]
    public void GradientStopsAreWarmToCoolWithoutGreyMiddle()
    {
        string appXaml = FindAppXaml();
        var document = XDocument.Load(appXaml);
        XNamespace ns = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";

        var brush = document
            .Descendants(ns + "LinearGradientBrush")
            .FirstOrDefault(e => e.Attributes()
                .Any(a => a.Name.LocalName == "Key" && a.Value == "TemperatureRangeGradient"));
        Assert.NotNull(brush);
        var stops = brush!.Elements(ns + "GradientStop").ToList();

        Assert.Equal(ExpectedStops.Length, stops.Count);

        // The macOS two-stop ramp (#FFA94D -> #8FB8FF, see TemperatureGradient.swift)
        // must never be pasted here: interpolating between those endpoints passes
        // through grey mid-track. Rule recorded in
        // spec/learnings/windows-popup-color-slider.md.
        string[] actualColors = stops
            .Select(s => ((string?)s.Attribute("Color") ?? "").ToUpperInvariant())
            .ToArray();
        Assert.DoesNotContain("#FFA94D", actualColors);
        Assert.DoesNotContain("#8FB8FF", actualColors);

        for (int i = 0; i < ExpectedStops.Length; i++)
        {
            Assert.Equal(
                ExpectedStops[i].Color,
                (string?)stops[i].Attribute("Color"),
                StringComparer.OrdinalIgnoreCase);
            Assert.Equal(
                ExpectedStops[i].Offset,
                double.Parse((string?)stops[i].Attribute("Offset") ?? "nan"),
                precision: 3);
        }

        // No stop may be near-grey: saturation must stay well above the ~0.06
        // dip a macOS-style two-stop interpolation leaves mid-track.
        foreach (var stop in stops)
        {
            Assert.True(
                Saturation((string?)stop.Attribute("Color") ?? "") > 0.25,
                $"gradient stop {stop.Attribute("Color")} looks grey; mid-track washes out");
        }
    }

    private static double Saturation(string color)
    {
        string hex = color.TrimStart('#');
        if (hex.Length == 8)
            hex = hex.Substring(2);
        int r = Convert.ToInt32(hex.Substring(0, 2), 16);
        int g = Convert.ToInt32(hex.Substring(2, 2), 16);
        int b = Convert.ToInt32(hex.Substring(4, 2), 16);
        int max = Math.Max(r, Math.Max(g, b));
        int min = Math.Min(r, Math.Min(g, b));
        return max == 0 ? 0 : (max - min) / (double)max;
    }

    private static string FindAppXaml()
    {
        var directory = new DirectoryInfo(AppContext.BaseDirectory);
        while (directory != null)
        {
            string candidate = Path.Combine(directory.FullName, "App.xaml");
            if (File.Exists(candidate))
                return candidate;
            string nested = Path.Combine(directory.FullName, "apps", "windows", "App.xaml");
            if (File.Exists(nested))
                return nested;
            directory = directory.Parent;
        }

        throw new FileNotFoundException("Could not locate apps/windows/App.xaml from test output.");
    }
}
