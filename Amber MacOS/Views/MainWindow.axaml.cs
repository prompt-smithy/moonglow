using Avalonia.Controls;

namespace Amber.MacOS.Views;

public partial class MainWindow : Window
{
    private bool _allowClose;

    public MainWindow()
    {
        InitializeComponent();
        Closing += (_, args) =>
        {
            if (_allowClose)
                return;

            args.Cancel = true;
            Hide();
        };
    }

    public void AllowClose() => _allowClose = true;
}