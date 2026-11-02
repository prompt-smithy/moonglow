using Moonglow.Core;
using Xunit;

namespace Moonglow.Core.Tests;

public sealed class StartupPathsTests
{
    [Fact]
    public void PortableStartupUsesTheStableLauncherInsteadOfTheExtractedChild()
    {
        string launcher = Path.GetFullPath("Moonglow_Portable.exe");
        Assert.Equal(launcher, StartupPaths.Resolve("temporary-child.exe", launcher, _ => true));
        Assert.Equal("temporary-child.exe", StartupPaths.Resolve("temporary-child.exe", launcher, _ => false));
        Assert.Equal("installed-app.exe", StartupPaths.Resolve("installed-app.exe", null, _ => true));
    }
}
