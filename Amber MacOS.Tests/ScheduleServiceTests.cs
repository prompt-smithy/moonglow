using Amber.MacOS.Models;
using Amber.MacOS.Services;

namespace Amber.MacOS.Tests;

public sealed class ScheduleServiceTests
{
    private readonly AppSettings _settings = new();

    [Theory]
    [InlineData(5, 30, 1200)]
    [InlineData(6, 0, 1200)]
    [InlineData(6, 30, 3800)]
    [InlineData(7, 0, 6500)]
    [InlineData(12, 0, 6500)]
    [InlineData(18, 0, 3800)]
    [InlineData(18, 30, 1200)]
    [InlineData(23, 0, 1200)]
    public void ComputeTemperature_FollowsDailySchedule(int hour, int minute, int expectedKelvin)
    {
        var now = new DateTime(2026, 1, 1, hour, minute, 0);

        int result = ScheduleService.ComputeTemperature(_settings, now);

        Assert.Equal(expectedKelvin, result);
    }

    [Fact]
    public void ComputeTemperature_InvalidStoredTimeUsesDefault()
    {
        _settings.MorningStart = "invalid";

        int result = ScheduleService.ComputeTemperature(
            _settings,
            new DateTime(2026, 1, 1, 5, 30, 0));

        Assert.Equal(_settings.NightColorTempK, result);
    }
}