using System.Threading.Tasks;
using Moonglow.Core;

namespace Moonglow.Infrastructure;

internal enum StartupState
{
    Unavailable,
    Disabled,
    Enabled,
    DisabledByUser,
    DisabledByPolicy,
    EnabledByPolicy,
}

internal interface IStartupRegistration
{
    bool RestorePreferenceOnLaunch { get; }
    Task<StartupState> ReadAsync();
    Task<StartupState> SetEnabledAsync(bool enabled);
}

/// <summary>Keeps the startup toggle consistent with Windows, including user and policy overrides.</summary>
internal sealed class StartupController : ObservableObject
{
    private readonly IStartupRegistration _registration;
    private readonly AppSettings _settings;
    private readonly Action _save;
    private StartupState _state = StartupState.Unavailable;
    private bool _initialized;
    private bool _busy;
    private bool _changeFailed;

    public StartupController(IStartupRegistration registration, AppSettings settings, Action save)
    {
        _registration = registration;
        _settings = settings;
        _save = save;
    }

    // Preserve the last preference if Windows cannot be queried; do not claim that a failed
    // disable succeeded, or overwrite a valid preference with an invented disabled state.
    public bool Enabled => _state == StartupState.Unavailable ? _settings.LaunchAtLogin : IsEnabled(_state);

    public bool CanChange => _initialized && !_busy &&
        (_state == StartupState.Enabled || _state == StartupState.Disabled);

    public string Status
    {
        get
        {
            if (_busy || !_initialized)
                return "Checking startup settings...";
            if (_state == StartupState.DisabledByUser)
                return "Startup is disabled in Windows. Enable Moonglow in Settings > Apps > Startup or Task Manager.";
            if (_state == StartupState.DisabledByPolicy)
                return "Your administrator or Windows policy has disabled startup for Moonglow.";
            if (_state == StartupState.EnabledByPolicy)
                return "Your administrator or Windows policy requires Moonglow to open at sign-in.";
            if (_state == StartupState.Unavailable)
                return "Could not read the startup setting. Reopen Settings to try again.";
            if (_changeFailed)
                return "Windows did not accept the startup change. Check Windows startup settings, then try again.";
            return Enabled ? "Moonglow opens automatically when you sign in."
                : "Moonglow opens only when you launch it.";
        }
    }

    public Task InitializeAsync() => RefreshAsync(restorePreference: true);

    public Task RefreshAsync() => RefreshAsync(restorePreference: false);

    private async Task RefreshAsync(bool restorePreference)
    {
        if (_busy)
            return;

        _busy = true;
        _changeFailed = false;
        NotifyState();
        try
        {
            StartupState state = await _registration.ReadAsync();
            // Unpackaged builds retain their existing preference restoration. For MSIX,
            // Windows is authoritative: never re-enable a task disabled outside the app.
            if (restorePreference && _registration.RestorePreferenceOnLaunch &&
                _settings.LaunchAtLogin && state == StartupState.Disabled)
            {
                state = await _registration.SetEnabledAsync(true);
                _changeFailed = !IsEnabled(state);
            }
            AcceptState(state);
        }
        catch
        {
            _state = StartupState.Unavailable;
        }
        finally
        {
            _initialized = true;
            _busy = false;
            NotifyState();
        }
    }

    public async Task SetEnabledAsync(bool enabled)
    {
        if (!CanChange || enabled == Enabled)
        {
            NotifyState();
            return;
        }

        _busy = true;
        _changeFailed = false;
        NotifyState();
        try
        {
            StartupState state = await _registration.SetEnabledAsync(enabled);
            _changeFailed = state == StartupState.Unavailable || IsEnabled(state) != enabled;
            AcceptState(state);
        }
        catch
        {
            _state = StartupState.Unavailable;
        }
        finally
        {
            _busy = false;
            NotifyState();
        }
    }

    private void AcceptState(StartupState state)
    {
        _state = state;
        if (state == StartupState.Unavailable || _settings.LaunchAtLogin == IsEnabled(state))
            return;
        _settings.LaunchAtLogin = IsEnabled(state);
        _save();
    }

    private static bool IsEnabled(StartupState state)
        => state == StartupState.Enabled || state == StartupState.EnabledByPolicy;

    private void NotifyState()
    {
        OnPropertyChanged(nameof(Enabled));
        OnPropertyChanged(nameof(CanChange));
        OnPropertyChanged(nameof(Status));
    }
}
