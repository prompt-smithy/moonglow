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
    public void HotkeyStringsParseAndCanonicalize()
    {
        Assert.True(Hotkey.TryParse("Ctrl+Alt+PageUp", out Hotkey? up));
        Assert.Equal(Hotkey.ModControl | Hotkey.ModAlt, up!.Modifiers);
        Assert.Equal(0x21, up.VirtualKey);
        Assert.Equal("Ctrl+Alt+PageUp", up.ToString());

        Assert.True(Hotkey.TryParse("alt+ctrl+]", out Hotkey? bracket));
        Assert.Equal("Ctrl+Alt+]", bracket!.ToString());

        Assert.True(Hotkey.TryParse("Win+Up", out Hotkey? win));
        Assert.Equal(Hotkey.ModWin, win!.Modifiers);
        Assert.Equal("Super+Up", win.ToString());

        Assert.True(Hotkey.TryGetKeyName(0x21, out string? name));
        Assert.Equal("PageUp", name);
        Assert.False(Hotkey.TryGetKeyName(0xFF, out _));

        Assert.False(Hotkey.TryParse("Ctrl+Alt", out _));
        Assert.False(Hotkey.TryParse("Ctrl+Nonsense", out _));
        Assert.False(Hotkey.TryParse("JustAKey", out _));
        Assert.False(Hotkey.TryParse(null, out _));
        Assert.False(Hotkey.TryParse("  ", out _));
    }

    [Fact]
    public void ShortcutSettingsFallBackLeniently()
    {
        var settings = new AppSettings
        {
            IncreaseBrightnessShortcut = "bogus",
            DecreaseBrightnessShortcut = null,
        }.Normalized();

        Assert.Equal(AppSettings.DefaultIncreaseBrightnessShortcut, settings.IncreaseBrightnessShortcut);
        Assert.Null(settings.DecreaseBrightnessShortcut);
        Assert.Equal(AppSettings.DefaultIncreaseTemperatureShortcut, settings.IncreaseTemperatureShortcut);
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
