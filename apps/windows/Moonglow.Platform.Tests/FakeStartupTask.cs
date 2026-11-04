namespace Windows.ApplicationModel;

// The production adapter is source-linked into the tests, like the display worker. These
// fakes exercise its user/policy guards without registering a real app to start at sign-in.
internal enum StartupTaskState
{
    Disabled,
    DisabledByUser,
    Enabled,
    DisabledByPolicy,
    EnabledByPolicy,
}

internal sealed class StartupTask
{
    public static StartupTask Current { get; set; } = new();
    public static string? RequestedTaskId { get; private set; }
    public StartupTaskState State { get; set; }
    public StartupTaskState EnableResult { get; set; } = StartupTaskState.Enabled;
    public int EnableRequests { get; private set; }
    public int DisableRequests { get; private set; }

    public static Task<StartupTask> GetAsync(string taskId)
    {
        RequestedTaskId = taskId;
        return Task.FromResult(Current);
    }

    public Task<StartupTaskState> RequestEnableAsync()
    {
        EnableRequests++;
        State = EnableResult;
        return Task.FromResult(State);
    }

    public void Disable()
    {
        DisableRequests++;
        State = StartupTaskState.Disabled;
    }
}
