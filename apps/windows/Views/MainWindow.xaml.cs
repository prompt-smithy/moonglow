using System.Windows;
using System.Windows.Input;
using System.Windows.Threading;
using Moonglow.Infrastructure;

namespace Moonglow.Views;

public partial class MainWindow : Window
{
    private readonly DispatcherTimer _inactivityTimer;

    public MainWindow()
    {
        InitializeComponent();
        Icon = IconFactory.CreateImageSource();

        _inactivityTimer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(10) };
        _inactivityTimer.Tick += (_, _) => Close();
        Loaded += (_, _) => ResetInactivityTimer();
        Closed += (_, _) => _inactivityTimer.Stop();
    }

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
