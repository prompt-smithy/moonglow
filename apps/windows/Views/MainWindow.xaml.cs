using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Threading;
using Moonglow.Core;
using Moonglow.ViewModels;


namespace Moonglow.Views;

public partial class MainWindow : Window
{
    private readonly DispatcherTimer _inactivityTimer;

    public MainWindow()
    {
        InitializeComponent();
        // No explicit window icon: the title bar and taskbar fall back to the
        // executable's icon, whose per-size frames stay crisper than a
        // runtime bitmap scaled down by the shell.

        _inactivityTimer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(10) };
        _inactivityTimer.Tick += (_, _) => Close();
        Loaded += (_, _) => ResetInactivityTimer();
        Closed += (_, _) => _inactivityTimer.Stop();
        AddHandler(Keyboard.PreviewKeyDownEvent, new KeyEventHandler(OnHotkeyCapture));
    }

    private void RecordHotkey_Click(object sender, RoutedEventArgs e)
    {
        if (sender is Button button
            && button.Tag is HotkeySlot slot
            && DataContext is MainViewModel viewModel)
        {
            if (viewModel.CapturingSlot == slot)
                viewModel.CancelCapture();
            else
                viewModel.BeginCapture(slot);
        }
    }

    private void ClearHotkey_Click(object sender, RoutedEventArgs e)
    {
        if (sender is Button button
            && button.Tag is HotkeySlot slot
            && DataContext is MainViewModel viewModel)
            viewModel.SetHotkey(slot, null);
    }

    private void ResetHotkeys_Click(object sender, RoutedEventArgs e)
    {
        if (DataContext is MainViewModel viewModel)
            viewModel.ResetHotkeys();
    }

    /// <summary>Captures the next key press while a shortcut row is recording.
    /// Escape cancels; Delete turns the shortcut off.</summary>
    private void OnHotkeyCapture(object sender, KeyEventArgs e)
    {
        if (DataContext is not MainViewModel viewModel || viewModel.CapturingSlot is not HotkeySlot slot)
            return;

        e.Handled = true;
        if (e.Key == Key.Escape)
        {
            viewModel.CancelCapture();
            return;
        }
        if (e.Key == Key.Delete || e.Key == Key.Back)
        {
            viewModel.SetHotkey(slot, null);
            return;
        }

        Key key = e.Key == Key.System ? e.SystemKey : e.Key;
        if (IsModifierKey(key))
            return;

        int modifiers = 0;
        if (Keyboard.Modifiers.HasFlag(ModifierKeys.Control))
            modifiers |= Hotkey.ModControl;
        if (Keyboard.Modifiers.HasFlag(ModifierKeys.Alt))
            modifiers |= Hotkey.ModAlt;
        if (Keyboard.Modifiers.HasFlag(ModifierKeys.Shift))
            modifiers |= Hotkey.ModShift;
        if (Keyboard.Modifiers.HasFlag(ModifierKeys.Windows))
            modifiers |= Hotkey.ModWin;

        if (modifiers == 0)
        {
            viewModel.SetHotkeyMessage("Add Ctrl, Alt, Shift, or Win so the shortcut does not fire while typing.");
            return;
        }

        if (!Hotkey.TryGetKeyName(KeyInterop.VirtualKeyFromKey(key), out string? keyName) || keyName is null)
        {
            viewModel.SetHotkeyMessage("That key cannot be used. Try an arrow, letter, digit, or function key.");
            return;
        }

        viewModel.SetHotkey(slot, Hotkey.FormatGesture(modifiers, keyName));
    }

    private static bool IsModifierKey(Key key)
        => key is Key.LeftCtrl or Key.RightCtrl
            or Key.LeftAlt or Key.RightAlt
            or Key.LeftShift or Key.RightShift
            or Key.LWin or Key.RWin;

    private void OnMouseActivity(object sender, MouseEventArgs e) => ResetInactivityTimer();

    private void OnMouseActivity(object sender, MouseButtonEventArgs e) => ResetInactivityTimer();

    private void OnMouseActivity(object sender, MouseWheelEventArgs e) => ResetInactivityTimer();

    private void OnKeyboardActivity(object sender, KeyEventArgs e) => ResetInactivityTimer();

    private void OnTouchActivity(object sender, TouchEventArgs e) => ResetInactivityTimer();

    private void ResetInactivityTimer()
    {
        _inactivityTimer.Stop();
        _inactivityTimer.Start();
    }

}
