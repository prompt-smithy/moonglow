using System.Threading.Tasks;
using Windows.ApplicationModel;

namespace Moonglow.Infrastructure;

/// <summary>Uses the startup task declared in the MSIX manifest; never writes Run registry keys.</summary>
internal sealed class PackagedStartupRegistration : IStartupRegistration
{
    internal const string TaskId = "MoonglowStartup";

    public bool RestorePreferenceOnLaunch => false;

    public async Task<StartupState> ReadAsync()
    {
        StartupTask task = await StartupTask.GetAsync(TaskId);
        return ConvertState(task.State);
    }

    public async Task<StartupState> SetEnabledAsync(bool enabled)
    {
        StartupTask task = await StartupTask.GetAsync(TaskId);
        if (enabled && task.State == StartupTaskState.Disabled)
        {
            // The caller awaits on the WPF UI context. Respect a refusal or a user/policy
            // override instead of falling back to registry-based startup registration.
            return ConvertState(await task.RequestEnableAsync());
        }

        if (!enabled && task.State == StartupTaskState.Enabled)
            task.Disable();

        return ConvertState(task.State);
    }

    private static StartupState ConvertState(StartupTaskState state) => state switch
    {
        StartupTaskState.Disabled => StartupState.Disabled,
        StartupTaskState.Enabled => StartupState.Enabled,
        StartupTaskState.DisabledByUser => StartupState.DisabledByUser,
        StartupTaskState.DisabledByPolicy => StartupState.DisabledByPolicy,
        StartupTaskState.EnabledByPolicy => StartupState.EnabledByPolicy,
        _ => StartupState.Unavailable,
    };
}
