using System.Windows.Interop;
using Moonglow.Core;
using Moonglow.Native;

namespace Moonglow.Services;

/// <summary>System-wide hotkeys for the four shortcut actions.</summary>
/// <remarks>
/// The hook lives on a message-only window so shortcuts fire while the settings
/// window is closed. Registration maps slot order to hotkey ids; a combo the OS
/// rejects (taken by another app, or reserved like Win+L) is skipped, leaving
/// that action on its buttons and sliders.
/// </remarks>
internal sealed class HotkeyService : IDisposable
{
    private static readonly IntPtr MessageOnlyParent = new(-3);

    private readonly HwndSource _source;
    private readonly Action<HotkeySlot> _pressed;
    private readonly Dictionary<int, HotkeySlot> _ids = new();
    private bool _disposed;

    public HotkeyService(Action<HotkeySlot> pressed)
    {
        _pressed = pressed;
        _source = new HwndSource(new HwndSourceParameters("Moonglow Hotkeys")
        {
            ParentWindow = MessageOnlyParent,
            Width = 0,
            Height = 0,
        });
        _source.AddHook(WndProc);
    }

    /// <summary>Re-registers every configured shortcut; call after shortcut edits.</summary>
    public void Refresh(AppSettings settings)
    {
        if (_disposed)
            throw new ObjectDisposedException(nameof(HotkeyService));

        foreach (int id in _ids.Keys)
            NativeMethods.UnregisterHotKey(_source.Handle, id);
        _ids.Clear();

        Register(settings.IncreaseBrightnessShortcut, HotkeySlot.IncreaseBrightness);
        Register(settings.DecreaseBrightnessShortcut, HotkeySlot.DecreaseBrightness);
        Register(settings.IncreaseTemperatureShortcut, HotkeySlot.IncreaseTemperature);
        Register(settings.DecreaseTemperatureShortcut, HotkeySlot.DecreaseTemperature);
    }

    private void Register(string? gesture, HotkeySlot slot)
    {
        if (!Hotkey.TryParse(gesture, out Hotkey? hotkey) || hotkey is null)
            return;

        int id = (int)slot;
        if (NativeMethods.RegisterHotKey(_source.Handle, id, hotkey.Modifiers, hotkey.VirtualKey))
            _ids[id] = slot;
    }

    private IntPtr WndProc(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
    {
        if (msg == NativeMethods.WM_HOTKEY && _ids.TryGetValue(wParam.ToInt32(), out HotkeySlot slot))
        {
            handled = true;
            _pressed(slot);
        }

        return IntPtr.Zero;
    }

    public void Dispose()
    {
        if (_disposed)
            return;
        _disposed = true;

        foreach (int id in _ids.Keys)
            NativeMethods.UnregisterHotKey(_source.Handle, id);
        _ids.Clear();
        _source.Dispose();
    }
}
