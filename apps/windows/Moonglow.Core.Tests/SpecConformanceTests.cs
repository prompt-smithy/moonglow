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
    public void SettingsNormalizationVectorsMatch()
    {
        using var document = Load("settings.json");
        var options = new JsonSerializerOptions { PropertyNameCaseInsensitive = true, PropertyNamingPolicy = JsonNamingPolicy.CamelCase };
        foreach (var row in document.RootElement.GetProperty("cases").EnumerateArray())
        {
            var settings = JsonSerializer.Deserialize<AppSettings>(row.GetProperty("input").GetRawText(), options)!.Normalized();
            using var actual = JsonDocument.Parse(JsonSerializer.Serialize(settings, options));
            foreach (var expected in row.GetProperty("expected").EnumerateObject())
                Assert.Equal(expected.Value.GetRawText(), actual.RootElement.GetProperty(expected.Name).GetRawText());
        }
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
        HardwareBrightnessEnabled = profile.TryGetProperty("hardwareBrightnessEnabled", out var hw) ? hw.GetBoolean() : true,
        SoftwareBrightnessEnabled = profile.TryGetProperty("softwareBrightnessEnabled", out var sw) ? sw.GetBoolean() : true,
        SplitBrightnessControls = profile.TryGetProperty("splitBrightnessControls", out var sp) ? sp.GetBoolean() : false,
        ManualHardwareBrightness = profile.TryGetProperty("manualHardwareBrightness", out var mhw) ? mhw.GetInt32() : 80,
        ManualSoftwareBrightness = profile.TryGetProperty("manualSoftwareBrightness", out var msw) ? msw.GetInt32() : 100,
    };

    [Fact]
    public void BrightnessRoutingVectorsMatch()
    {
        using var document = Load("brightness-routing.json");
        int floor = document.RootElement.GetProperty("softwareBrightnessFloor").GetInt32();
        Assert.Equal(AppSettings.SoftwareBrightnessFloor, floor);

        foreach (var row in document.RootElement.GetProperty("vectors").EnumerateArray())
        {
            int master = row.GetProperty("master").GetInt32();
            int hwInput = row.GetProperty("hardwareInput").GetInt32();
            int swInput = row.GetProperty("softwareInput").GetInt32();
            bool split = row.GetProperty("split").GetBoolean();
            bool hwAvail = row.GetProperty("hardwareAvailable").GetBoolean();
            bool swAvail = row.GetProperty("softwareAvailable").GetBoolean();
            bool recReg = row.GetProperty("recoveryRegistered").GetBoolean();

            var output = BrightnessPlan.ComputeRouting(
                master, split, hwInput, swInput, hwAvail, swAvail, recReg);

            int? expectedHw = row.GetProperty("expectedHardware").ValueKind == JsonValueKind.Null
                ? null
                : row.GetProperty("expectedHardware").GetInt32();
            int expectedSw = row.GetProperty("expectedSoftware").GetInt32();

            Assert.Equal(expectedHw, output.Hardware);
            Assert.Equal(expectedSw, output.Software);
        }

        foreach (var row in document.RootElement.GetProperty("transitions").EnumerateArray())
        {
            bool hwAvail = row.GetProperty("hardwareAvailable").GetBoolean();
            bool swAvail = row.GetProperty("softwareAvailable").GetBoolean();
            bool recReg = row.GetProperty("recoveryRegistered").GetBoolean();

            if (row.TryGetProperty("expectedMaster", out var expMasterProp))
            {
                int hw = row.GetProperty("hardware").GetInt32();
                int sw = row.GetProperty("software").GetInt32();
                int master = BrightnessPlan.SplitToUnified(hw, sw, hwAvail, swAvail, recReg);
                Assert.Equal(expMasterProp.GetInt32(), master);
            }
            else
            {
                int master = row.GetProperty("master").GetInt32();
                var split = BrightnessPlan.UnifiedToSplit(master, hwAvail, swAvail, recReg);
                Assert.Equal(row.GetProperty("expectedHardware").GetInt32(), split.Hardware);
                Assert.Equal(row.GetProperty("expectedSoftware").GetInt32(), split.Software);
            }
        }
    }

    /// <summary>Vector times are two-element <c>[hour, minute]</c> arrays.</summary>
    private static string TimeFromPair(JsonElement pair)
        => AppSettings.FormatTime(new TimeSpan(pair[0].GetInt32(), pair[1].GetInt32(), 0));
}
