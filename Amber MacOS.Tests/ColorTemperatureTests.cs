using Amber.MacOS.Services;

namespace Amber.MacOS.Tests;

public sealed class ColorTemperatureTests
{
    [Fact]
    public void KelvinToRgbScale_WarmTemperatureReducesBlueChannel()
    {
        var warm = MacColorTemperatureService.KelvinToRgbScale(1200);
        var neutral = MacColorTemperatureService.KelvinToRgbScale(6500);

        Assert.Equal(1.0, warm.Red);
        Assert.True(warm.Blue < warm.Green);
        Assert.True(warm.Green < warm.Red);
        Assert.True(neutral.Blue > warm.Blue);
    }

    [Theory]
    [InlineData(900)]
    [InlineData(1000)]
    [InlineData(6500)]
    [InlineData(40000)]
    public void KelvinToRgbScale_ChannelsStayNormalized(int kelvin)
    {
        var scale = MacColorTemperatureService.KelvinToRgbScale(kelvin);

        Assert.InRange(scale.Red, 0, 1);
        Assert.InRange(scale.Green, 0, 1);
        Assert.InRange(scale.Blue, 0, 1);
    }

    [Fact]
    public void KelvinToRgbScale_900KelvinIsWarmerThan1200Kelvin()
    {
        var minimum = MacColorTemperatureService.KelvinToRgbScale(900);
        var nightDefault = MacColorTemperatureService.KelvinToRgbScale(1200);

        Assert.True(minimum.Green < nightDefault.Green);
        Assert.True(minimum.Blue <= nightDefault.Blue);
    }
}