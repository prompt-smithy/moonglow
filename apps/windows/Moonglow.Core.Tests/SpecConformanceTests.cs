using System.Text.Json;
using Moonglow.Core;
using Xunit;

namespace Moonglow.Core.Tests;

/// <summary>
/// Verifies the platform against the shared contract in <c>spec/vectors</c>.
/// </summary>
/// <remarks>
/// These vectors are the only thing the three platform implementations share. If one of
/// these fails, this implementation has drifted from the contract — fix the implementation,
/// or change the spec and update every platform together.
/// </remarks>
public sealed class SpecConformanceTests
{
    private const double Tolerance = 1e-6;

    private static readonly string VectorsDirectory =
        Path.Combine(AppContext.BaseDirectory, "vectors");

    private static JsonDocument Load(string name)
    {
        string path = Path.Combine(VectorsDirectory, name);
        return JsonDocument.Parse(File.ReadAllText(path));
    }

    [Fact]
    public void KelvinVectorsMatchWithinTolerance()
    {
        using JsonDocument document = Load("kelvin.json");
        JsonElement rows = document.RootElement.GetProperty("vectors");
        Assert.NotEqual(0, rows.GetArrayLength());

        foreach (JsonElement row in rows.EnumerateArray())
        {
            int kelvin = row.GetProperty("kelvin").GetInt32();
            var scale = KelvinConversion.RgbScale(kelvin);

            Assert.True(
                Math.Abs(scale.R - row.GetProperty("red").GetDouble()) <= Tolerance,
                $"red at {kelvin} K: {scale.R} != {row.GetProperty("red").GetDouble()}");
            Assert.True(
                Math.Abs(scale.G - row.GetProperty("green").GetDouble()) <= Tolerance,
                $"green at {kelvin} K: {scale.G} != {row.GetProperty("green").GetDouble()}");
            Assert.True(
                Math.Abs(scale.B - row.GetProperty("blue").GetDouble()) <= Tolerance,
                $"blue at {kelvin} K: {scale.B} != {row.GetProperty("blue").GetDouble()}");
        }
    }

    [Fact]
    public void ScheduleVectorsMatchExactly()
    {
        using JsonDocument document = Load("schedule.json");
        JsonElement profiles = document.RootElement.GetProperty("settings");
        JsonElement cases = document.RootElement.GetProperty("cases");
        Assert.NotEqual(0, cases.GetArrayLength());

        foreach (JsonElement entry in cases.EnumerateArray())
        {
            string profileName = entry.GetProperty("settings").GetString()!;
            Assert.True(
                profiles.TryGetProperty(profileName, out JsonElement profile),
                $"vector references unknown settings profile {profileName}");

            string timeText = entry.GetProperty("time").GetString()!;
            Assert.True(
                AppSettings.TryParseTime(timeText, out TimeSpan time),
                $"vector has malformed time {timeText}");

            int expected = entry.GetProperty("kelvin").GetInt32();
            int actual = ScheduleCalculator.TemperatureFor(ToSettings(profile), time);
            Assert.True(expected == actual, $"at {timeText} with {profileName}: {actual} != {expected}");
        }
    }

    [Fact]
    public void SnappingVectorsMatchExactly()
    {
        using JsonDocument document = Load("temperature-range.json");

        foreach (JsonElement row in document.RootElement.GetProperty("snap").EnumerateArray())
        {
            int input = row.GetProperty("input").GetInt32();
            int expected = row.GetProperty("output").GetInt32();
            Assert.True(expected == TemperatureRange.Snap(input), $"snap({input}): {TemperatureRange.Snap(input)} != {expected}");
        }
    }

    [Fact]
    public void RangeOrderingVectorsMatchExactly()
    {
        using JsonDocument document = Load("temperature-range.json");

        foreach (JsonElement row in document.RootElement.GetProperty("range").EnumerateArray())
        {
            int night = row.GetProperty("night").GetInt32();
            int day = row.GetProperty("day").GetInt32();
            var range = new TemperatureRange(night, day);

            int expectedNight = row.GetProperty("nightOut").GetInt32();
            int expectedDay = row.GetProperty("dayOut").GetInt32();
            Assert.True(expectedNight == range.Night, $"night for {night}/{day}: {range.Night} != {expectedNight}");
            Assert.True(expectedDay == range.Day, $"day for {night}/{day}: {range.Day} != {expectedDay}");
        }
    }

    private static AppSettings ToSettings(JsonElement profile) => new()
    {
        DayColorTempK = profile.GetProperty("dayColorTempK").GetInt32(),
        NightColorTempK = profile.GetProperty("nightColorTempK").GetInt32(),
        MorningStart = TimeFromPair(profile.GetProperty("morningStart")),
        MorningEnd = TimeFromPair(profile.GetProperty("morningEnd")),
        EveningStart = TimeFromPair(profile.GetProperty("eveningStart")),
        EveningEnd = TimeFromPair(profile.GetProperty("eveningEnd")),
        AutoColorTemperature = profile.GetProperty("autoColorTemperature").GetBoolean(),
        ManualBrightness = profile.GetProperty("manualBrightness").GetInt32(),
        ManualColorTempK = profile.GetProperty("manualColorTempK").GetInt32(),
        LaunchAtLogin = profile.GetProperty("launchAtLogin").GetBoolean(),
    };

    /// <summary>Vector times are two-element <c>[hour, minute]</c> arrays.</summary>
    private static string TimeFromPair(JsonElement pair)
        => AppSettings.FormatTime(new TimeSpan(pair[0].GetInt32(), pair[1].GetInt32(), 0));
}
