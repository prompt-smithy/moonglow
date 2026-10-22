using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using Moonglow.Infrastructure;

namespace Moonglow.Views;

public partial class TemperatureRangeSlider : UserControl
{
    private const int Minimum = 900;
    private const int Maximum = 6600;
    private const int Step = 100;
    private const double ThumbSize = 16;
    /// <summary>Must match the track <c>Border</c> height in TemperatureRangeSlider.xaml.</summary>
    private const double TrackThickness = 6;

    public static readonly DependencyProperty NightValueProperty = DependencyProperty.Register(
        nameof(NightValue), typeof(int), typeof(TemperatureRangeSlider),
        new FrameworkPropertyMetadata(1200, FrameworkPropertyMetadataOptions.BindsTwoWayByDefault, OnRangeValueChanged));

    public static readonly DependencyProperty DayValueProperty = DependencyProperty.Register(
        nameof(DayValue), typeof(int), typeof(TemperatureRangeSlider),
        new FrameworkPropertyMetadata(6500, FrameworkPropertyMetadataOptions.BindsTwoWayByDefault, OnRangeValueChanged));

    public int NightValue
    {
        get => (int)GetValue(NightValueProperty);
        set => SetValue(NightValueProperty, value);
    }

    public int DayValue
    {
        get => (int)GetValue(DayValueProperty);
        set => SetValue(DayValueProperty, value);
    }

    public TemperatureRangeSlider()
    {
        InitializeComponent();
        Loaded += (_, _) => UpdateThumbPositions();
    }

    private static void OnRangeValueChanged(DependencyObject dependencyObject, DependencyPropertyChangedEventArgs e)
        => ((TemperatureRangeSlider)dependencyObject).UpdateThumbPositions();

    private void Root_SizeChanged(object sender, SizeChangedEventArgs e) => UpdateThumbPositions();

    private void UpdateThumbPositions()
    {
        if (!IsLoaded || SliderCanvas.ActualWidth <= ThumbSize)
            return;

        double trackStart = ThumbSize / 2;
        double trackWidth = SliderCanvas.ActualWidth - ThumbSize;
        double nightPosition = ValueToPosition(NightValue, trackStart, trackWidth);
        double dayPosition = ValueToPosition(DayValue, trackStart, trackWidth);
        double trackTop = (SliderCanvas.ActualHeight - TrackThickness) / 2;
        double thumbTop = (SliderCanvas.ActualHeight - ThumbSize) / 2;

        Canvas.SetLeft(BaseTrack, trackStart);
        Canvas.SetTop(BaseTrack, trackTop);
        BaseTrack.Width = trackWidth;

        Canvas.SetLeft(SelectedTrack, nightPosition);
        Canvas.SetTop(SelectedTrack, trackTop);
        SelectedTrack.Width = Math.Max(0, dayPosition - nightPosition);

        Canvas.SetLeft(NightThumb, nightPosition - ThumbSize / 2);
        Canvas.SetTop(NightThumb, thumbTop);
        Canvas.SetLeft(DayThumb, dayPosition - ThumbSize / 2);
        Canvas.SetTop(DayThumb, thumbTop);
    }

    private static double ValueToPosition(int value, double start, double width)
    {
        double progress = (MathUtil.Clamp(value, Minimum, Maximum) - Minimum) / (double)(Maximum - Minimum);
        return start + progress * width;
    }

    private void NightThumb_DragDelta(object sender, DragDeltaEventArgs e)
        => MoveThumb(NightThumb, e.HorizontalChange, true);

    private void DayThumb_DragDelta(object sender, DragDeltaEventArgs e)
        => MoveThumb(DayThumb, e.HorizontalChange, false);

    private void MoveThumb(Thumb thumb, double horizontalChange, bool isNight)
    {
        double current = Canvas.GetLeft(thumb) + ThumbSize / 2;
        double position = current + horizontalChange;
        int value = PositionToValue(position);

        if (isNight)
            SetCurrentValue(NightValueProperty, Math.Min(value, DayValue));
        else
            SetCurrentValue(DayValueProperty, Math.Max(value, NightValue));
    }

    private int PositionToValue(double position)
    {
        double trackStart = ThumbSize / 2;
        double trackWidth = SliderCanvas.ActualWidth - ThumbSize;
        double progress = MathUtil.Clamp((position - trackStart) / trackWidth, 0, 1);
        return MathUtil.Clamp(Minimum + (int)Math.Round((Maximum - Minimum) * progress / Step) * Step, Minimum, Maximum);
    }

    private void SliderCanvas_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
    {
        if (e.OriginalSource is FrameworkElement element && (element == NightThumb || element == DayThumb))
            return;

        Point click = e.GetPosition(SliderCanvas);
        double nightPosition = Canvas.GetLeft(NightThumb) + ThumbSize / 2;
        double dayPosition = Canvas.GetLeft(DayThumb) + ThumbSize / 2;
        bool isNight = Math.Abs(click.X - nightPosition) < Math.Abs(click.X - dayPosition);
        int value = PositionToValue(click.X);

        if (isNight)
            SetCurrentValue(NightValueProperty, Math.Min(value, DayValue));
        else
            SetCurrentValue(DayValueProperty, Math.Max(value, NightValue));

        e.Handled = true;
    }

    private void NightThumb_KeyDown(object sender, KeyEventArgs e) => AdjustWithKeyboard(true, e);

    private void DayThumb_KeyDown(object sender, KeyEventArgs e) => AdjustWithKeyboard(false, e);

    private void AdjustWithKeyboard(bool isNight, KeyEventArgs e)
    {
        int delta = e.Key switch
        {
            Key.Left or Key.Down => -Step,
            Key.Right or Key.Up => Step,
            _ => 0
        };

        if (delta == 0)
            return;

        if (isNight)
            SetCurrentValue(NightValueProperty, Math.Min(MathUtil.Clamp(NightValue + delta, Minimum, Maximum), DayValue));
        else
            SetCurrentValue(DayValueProperty, Math.Max(MathUtil.Clamp(DayValue + delta, Minimum, Maximum), NightValue));

        e.Handled = true;
    }
}