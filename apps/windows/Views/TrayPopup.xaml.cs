using System;
using System.Windows;
using System.Windows.Controls;
using Amber.ViewModels;

namespace Amber.Views;

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
        if (sender is not Slider slider || !slider.IsLoaded)
            return;

        int value = (int)Math.Round(e.NewValue);
        if (DataContext is MainViewModel vm && value != vm.Brightness)
            vm.Brightness = value;
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
