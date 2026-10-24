using Moonglow.Core;
using Xunit;

namespace Moonglow.Core.Tests;

/// <summary>Unit coverage for the documented settings-normalization contract.</summary>
public sealed class AppSettingsTests
{
    [Fact]
    public void NormalizedClampsValuesIntoSupportedRanges()
    {
        var settings = new AppSettings
        {
            ManualBrightness = 500,
            ManualColorTempK = 100,
            DayColorTempK = 9000,
            NightColorTempK = 800,
        };

        AppSettings normalized = settings.Normalized();

        Assert.Equal(AppSettings.MaxBrightnessPercent, normalized.ManualBrightness);
        Assert.Equal(AppSettings.MinKelvin, normalized.ManualColorTempK);
        Assert.Equal(AppSettings.MaxKelvin, normalized.DayColorTempK);
        Assert.Equal(AppSettings.MinKelvin, normalized.NightColorTempK);
    }

    [Fact]
    public void UnparseableScheduleTimeFallsBackToItsDefault()
    {
        var settings = new AppSettings
        {
            MorningStart = "not-a-time",
            MorningEnd = "07:00",
        };

        AppSettings normalized = settings.Normalized();

        Assert.Equal(AppSettings.DefaultMorningStart, normalized.MorningStart);
        Assert.Equal(AppSettings.DefaultMorningEnd, normalized.MorningEnd);
    }

    [Fact]
    public void TimesRoundTripThroughCanonicalForm()
    {
        Assert.True(AppSettings.TryParseTime("6:05", out TimeSpan time));
        Assert.Equal("06:05", AppSettings.FormatTime(time));
        Assert.False(AppSettings.TryParseTime("24:00", out _));
    }

    [Fact]
    public void RangeHandlesPinInsteadOfCrossing()
    {
        var range = new TemperatureRange(1200, 6500);

        Assert.Equal(6500, range.MovingNight(6600).Night);
        Assert.Equal(1200, range.MovingDay(900).Day);
        Assert.Equal(1200, range.Night);
        Assert.Equal(6500, range.Day);
    }
}
