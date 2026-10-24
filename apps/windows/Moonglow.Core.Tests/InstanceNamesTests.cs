using Moonglow.Core;
using Xunit;

namespace Moonglow.Core.Tests;

/// <summary>Pins the single-instance contract across the Amber rename.</summary>
public sealed class InstanceNamesTests
{
    [Fact]
    public void LegacyMutexNameMatchesPreMergeBuild()
    {
        // The original App.xaml.cs acquired Local\Amber.SingleInstance. If this drifts,
        // the old and merged builds run side by side and fight over the gamma ramps.
        Assert.Equal(@"Local\Amber.SingleInstance", InstanceNames.LegacyMutexName);
    }

    [Fact]
    public void MutexNamesAreSessionLocalAndDistinct()
    {
        Assert.Equal(@"Local\Moonglow.SingleInstance", InstanceNames.MutexName);
        Assert.NotEqual(InstanceNames.MutexName, InstanceNames.LegacyMutexName);
        Assert.StartsWith(@"Local\", InstanceNames.MutexName);
        Assert.StartsWith(@"Local\", InstanceNames.LegacyMutexName);
    }
}
