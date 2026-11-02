using Moonglow.Services;
using Xunit;

namespace Moonglow.Platform.Tests;

public sealed class SettingsServiceTests
{
    [Fact]
    public void DeferredSavesFlushTheLatestSnapshotBeforeRestart()
    {
        string directory = Path.Combine(Path.GetTempPath(), "moonglow-settings-" + Guid.NewGuid());
        string path = Path.Combine(directory, "settings.json");
        try
        {
            using (var store = new SettingsService(path))
            {
                for (int value = 20; value <= 60; value++)
                {
                    store.Settings.ManualBrightness = value;
                    store.ScheduleSave();
                }
            }
            using var restarted = new SettingsService(path);
            restarted.Load();
            Assert.Equal(60, restarted.Settings.ManualBrightness);
        }
        finally { if (Directory.Exists(directory)) Directory.Delete(directory, true); }
    }
}
