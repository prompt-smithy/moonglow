using System;
using System.Windows;
using System.Windows.Controls;
using Moonglow.ViewModels;

namespace Moonglow.Views;

public partial class TrayPopup : UserControl
{
    public TrayPopup()
    {
        InitializeComponent();
    }

    // Forward only genuine user adjustments. Before the popup is loaded the value comes from
    // binding/layout, and the OneWay binding's echo matches the current value, so both are
    // ignored and the saved brightness is never overwritten.
    private void BrightnessSlider_ValueChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
    {
        if (sender is not Slider slider || !slider.IsLoaded || !slider.IsEnabled)
            return;

        int value = (int)Math.Round(e.NewValue);
        if (DataContext is MainViewModel vm && value != vm.Brightness)
            vm.Brightness = value;
    }

    private void HardwareBrightnessSlider_ValueChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
    {
        if (sender is not Slider slider || !slider.IsLoaded || !slider.IsEnabled)
            return;
        int value = (int)Math.Round(e.NewValue);
        if (DataContext is MainViewModel vm && value != vm.HardwareBrightness)
            vm.HardwareBrightness = value;
    }

    private void SoftwareBrightnessSlider_ValueChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
    {
        if (sender is not Slider slider || !slider.IsLoaded || !slider.IsEnabled)
            return;
        int value = (int)Math.Round(e.NewValue);
        if (DataContext is MainViewModel vm && value != vm.SoftwareBrightness)
            vm.SoftwareBrightness = value;
    }

    private void BrightnessInfo_Click(object sender, RoutedEventArgs e)
    {
        if (DataContext is not MainViewModel vm)
            return;

        if (!vm.SoftwareBrightnessEnabled)
        {
            MessageBoxResult disabledResult = MessageBox.Show(
                "Software brightness is turned off in Settings. Enable Digital Shade to dim the image in software. A global Restore brightness to 100% shortcut is required before that control can be used.",
                "Software brightness is disabled",
                MessageBoxButton.YesNo,
                MessageBoxImage.Information);
            if (disabledResult == MessageBoxResult.Yes)
                vm.OpenSettingsCommand.Execute(null);
            return;
        }

        if (!vm.HasSoftwareBrightnessSupport)
        {
            MessageBox.Show(
                "The current Windows display backend does not provide software gamma brightness. Use the display's physical controls, or enable hardware brightness when every connected display supports it.",
                "Brightness control unavailable",
                MessageBoxButton.OK,
                MessageBoxImage.Information);
            return;
        }

        bool recoveryShortcutReady = vm.HasBrightnessRecoveryShortcut;
        string hardwareContext = !vm.HardwareBrightnessEnabled
            ? "Hardware Backlight is disabled in Settings."
            : vm.AllActiveDisplaysSupportHardwareBrightness
                ? "Hardware backlight control is available on every active display."
                : "At least one active display lacks hardware brightness, so Moonglow uses software dimming across all displays.";
        string message = recoveryShortcutReady
            ? $"{hardwareContext} Software dimming changes the image rather than the display backlight. Your global Restore brightness to 100% shortcut is registered and immediately brings the image back to full brightness.\n\nOpen keyboard shortcut settings?"
            : $"{hardwareContext} Software dimming can make the screen hard to read at low values. Set a global Restore brightness to 100% shortcut before using software dimming; that control stays disabled until Windows registers it.\n\nOpen keyboard shortcut settings now?";
        MessageBoxResult result = MessageBox.Show(
            message,
            recoveryShortcutReady ? "Software brightness is ready" : "Set a brightness recovery shortcut",
            MessageBoxButton.YesNo,
            MessageBoxImage.Information);
        if (result == MessageBoxResult.Yes)
            vm.OpenSettingsCommand.Execute(null);
    }

    // Same guard for color temperature: ignore binding/layout echoes, forward real user changes.
    private void ColorTempSlider_ValueChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
    {
        if (sender is not Slider slider || !slider.IsLoaded)
            return;

        int value = (int)Math.Round(e.NewValue);
        if (DataContext is MainViewModel vm && value != vm.ColorTemperature)
            vm.ColorTemperature = value;
    }
}
