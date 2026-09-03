using System;
using System.Windows;
using System.Windows.Interop;
using DisplayController.Infrastructure;
using static DisplayController.Native.NativeMethods;

namespace DisplayController.Views;

public partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();
        HeaderLogo.Source = LogoFactory.CreateIcon();
    }

    protected override void OnSourceInitialized(EventArgs e)
    {
        base.OnSourceInitialized(e);
        RemoveTitleBarIcon();
    }

    // Strips the caption icon so the title bar shows only the window title.
    private void RemoveTitleBarIcon()
    {
        IntPtr hwnd = new WindowInteropHelper(this).Handle;
        if (hwnd == IntPtr.Zero)
            return;

        int exStyle = GetWindowLong(hwnd, GWL_EXSTYLE);
        SetWindowLong(hwnd, GWL_EXSTYLE, exStyle | WS_EX_DLGMODALFRAME);
        SendMessage(hwnd, WM_SETICON, new IntPtr(1), IntPtr.Zero); // ICON_BIG
        SendMessage(hwnd, WM_SETICON, IntPtr.Zero, IntPtr.Zero);   // ICON_SMALL
        SetWindowPos(hwnd, IntPtr.Zero, 0, 0, 0, 0,
            (uint)(SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED));
    }

    private void Close_Click(object sender, RoutedEventArgs e) => Close();
}
