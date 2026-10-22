using System;
using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;

namespace Moonglow.Views;

/// <summary>12-hour / minute / AM-PM selectors for a canonical "HH:mm" time.</summary>
public partial class TimePicker : UserControl
{
    private bool _syncing;

    public static readonly DependencyProperty TimeProperty = DependencyProperty.Register(
        nameof(Time), typeof(string), typeof(TimePicker),
        new FrameworkPropertyMetadata("00:00", FrameworkPropertyMetadataOptions.BindsTwoWayByDefault, OnTimeChanged));

    public string Time
    {
        get => (string)GetValue(TimeProperty);
        set => SetValue(TimeProperty, value);
    }

    public TimePicker()
    {
        InitializeComponent();

        for (int m = 0; m < 60; m++)
            MinuteCombo.Items.Add(m.ToString("00", CultureInfo.InvariantCulture));
        AmPmCombo.Items.Add("AM");
        AmPmCombo.Items.Add("PM");

        RebuildHours();
        SyncFromTime();
    }

    private static void OnTimeChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
        => ((TimePicker)d).SyncFromTime();

    private void RebuildHours()
    {
        _syncing = true;
        HourCombo.Items.Clear();
        for (int h = 1; h <= 12; h++)
            HourCombo.Items.Add(h.ToString(CultureInfo.InvariantCulture));
        AmPmCombo.Visibility = Visibility.Visible;
        _syncing = false;
    }

    private void SyncFromTime()
    {
        if (HourCombo is null)
            return;

        if (!TimeSpan.TryParse(Time, CultureInfo.InvariantCulture, out var ts))
            ts = TimeSpan.Zero;

        _syncing = true;
        MinuteCombo.SelectedItem = ts.Minutes.ToString("00", CultureInfo.InvariantCulture);
        int hour12 = ts.Hours % 12;
        if (hour12 == 0)
            hour12 = 12;
        HourCombo.SelectedItem = hour12.ToString(CultureInfo.InvariantCulture);
        AmPmCombo.SelectedItem = ts.Hours >= 12 ? "PM" : "AM";
        _syncing = false;
    }

    private void Part_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (!_syncing)
            Compose();
    }

    private void Compose()
    {
        if (HourCombo.SelectedItem is not string hourText || MinuteCombo.SelectedItem is not string minuteText)
            return;
        if (!int.TryParse(hourText, NumberStyles.Integer, CultureInfo.InvariantCulture, out int hour) ||
            !int.TryParse(minuteText, NumberStyles.Integer, CultureInfo.InvariantCulture, out int minute))
            return;

        int hour24 = hour % 12;
        if ((AmPmCombo.SelectedItem as string) == "PM")
            hour24 += 12;

        Time = $"{hour24:00}:{minute:00}";
    }

    // Scroll a focused (clicked) box to step its value; hovering alone lets the wheel scroll the page.
    private void Combo_PreviewMouseWheel(object sender, MouseWheelEventArgs e)
    {
        if (sender is not ComboBox combo || combo.IsDropDownOpen || combo.Items.Count == 0)
            return;

        if (!combo.IsKeyboardFocusWithin)
            return;

        int index = combo.SelectedIndex + (e.Delta > 0 ? -1 : 1);
        if (index < 0)
            index = combo.Items.Count - 1;
        else if (index >= combo.Items.Count)
            index = 0;

        combo.SelectedIndex = index;
        e.Handled = true;
    }
}
