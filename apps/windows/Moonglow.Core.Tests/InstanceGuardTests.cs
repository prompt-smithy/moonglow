using Moonglow.Core;
using Xunit;

namespace Moonglow.Core.Tests;

/// <summary>Coverage for the process-scan side of single-instance protection.</summary>
public sealed class InstanceGuardTests
{
    [Fact]
    public void OwnProcessAloneIsNotAnotherCopy()
    {
        var processes = new[] { ("Moonglow", 100) };
        Assert.False(InstanceGuard.IsAnotherCopyRunning(processes, 100, InstanceGuard.OwnPrefixes));
    }

    [Fact]
    public void SecondPayloadProcessIsDetected()
    {
        var processes = new[] { ("Moonglow", 100), ("Moonglow", 101) };
        Assert.True(InstanceGuard.IsAnotherCopyRunning(processes, 100, InstanceGuard.OwnPrefixes));
    }

    [Fact]
    public void PortableLauncherAndLegacyBuildsAreDetectedCaseInsensitively()
    {
        var processes = new[]
        {
            ("Moonglow", 100),
            ("moonglow_portable_test3", 101),
            ("AMBER", 102),
        };
        Assert.True(InstanceGuard.IsAnotherCopyRunning(processes, 100, InstanceGuard.OwnPrefixes));
        Assert.True(InstanceGuard.IsAnotherCopyRunning(processes, 101, InstanceGuard.OwnPrefixes));
    }

    [Fact]
    public void DirectParentLauncherIsExemptButSiblingsAreNot()
    {
        var processes = new[]
        {
            ("Moonglow", 100),
            ("Moonglow_Portable_Test4", 90),
            ("Moonglow", 101),
        };
        Assert.True(InstanceGuard.IsAnotherCopyRunning(processes, 100, InstanceGuard.OwnPrefixes, parentPid: 90));

        var launcherOnly = new[]
        {
            ("Moonglow", 100),
            ("Moonglow_Portable_Test4", 90),
        };
        Assert.False(InstanceGuard.IsAnotherCopyRunning(launcherOnly, 100, InstanceGuard.OwnPrefixes, parentPid: 90));
    }

    [Fact]
    public void UnrelatedProcessesAreIgnored()
    {
        var processes = new[]
        {
            ("Moonglow", 100),
            ("explorer", 4),
            ("AmberAlert", 200),
        };
        Assert.False(InstanceGuard.IsAnotherCopyRunning(processes, 100, InstanceGuard.OwnPrefixes));
    }
}
