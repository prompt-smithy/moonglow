using System;
using System.Windows;
using System.Windows.Controls;
using DisplayController.Infrastructure;
using DisplayController.ViewModels;

namespace DisplayController.Views;

public partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();
        Icon = IconFactory.CreateImageSource();
    }

    private void Close_Click(object sender, RoutedEventArgs e) => Close();

    // Forward only genuine user changes; ignore binding/layout echoes so saved values aren't clobbered.
    private void DayColorSlider_ValueChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
    {
        if (sender is not Slider slider || !slider.IsLoaded)
            return;

        int value = (int)Math.Round(e.NewValue);
        if (DataContext is MainViewModel vm && value != vm.DayColorTemp)
            vm.DayColorTemp = value;
    }

    private void NightColorSlider_ValueChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
    {
        if (sender is not Slider slider || !slider.IsLoaded)
            return;

        int value = (int)Math.Round(e.NewValue);
        if (DataContext is MainViewModel vm && value != vm.NightColorTemp)
            vm.NightColorTemp = value;
    }
}
