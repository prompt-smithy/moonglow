using Amber.MacOS.Services;

namespace Amber.MacOS.Tests;

public sealed class DdcBrightnessTests
{
    [Theory]
    [InlineData(0, 0x01, 0xa9)]
    [InlineData(50, 0x32, 0x9a)]
    [InlineData(101, 0x64, 0xcc)]
    public void BuildWritePacket_ClampsValueAndComputesChecksum(int percent, byte value, byte checksum)
    {
        byte[] packet = MacDdcBrightnessService.BuildWritePacket(percent);

        Assert.Equal(new byte[] { 0x84, 0x03, 0x10, 0x00, value, checksum }, packet);
    }
}