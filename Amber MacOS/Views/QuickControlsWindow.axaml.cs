using Avalonia;
using Avalonia.Controls;

namespace Amber.MacOS.Views;

public partial class QuickControlsWindow : Window
{
    private bool _allowClose;

    public QuickControlsWindow()
    {
        InitializeComponent();
        Deactivated += (_, _) => Hide();
        Closing += (_, args) =>
        {
            if (_allowClose)
                return;

            args.Cancel = true;
            Hide();
        };
    }

    public void ShowNearMenuBar()
    {
        var screen = Screens.Primary;
        if (screen is not null)
        {
            int width = (int)Math.Ceiling(Width * screen.Scaling);
            Position = new PixelPoint(
                screen.WorkingArea.Right - width - 12,
                screen.WorkingArea.Y + 8);
        }

        Show();
        Activate();
    }

    public void AllowClose() => _allowClose = true;
}