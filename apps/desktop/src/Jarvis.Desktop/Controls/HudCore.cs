using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Threading;

namespace Jarvis.Desktop.Controls;

public class HudCore : Control
{
    public static readonly StyledProperty<double> LevelProperty =
        AvaloniaProperty.Register<HudCore, double>(nameof(Level));

    public static readonly StyledProperty<string> PhaseProperty =
        AvaloniaProperty.Register<HudCore, string>(nameof(Phase), "idle");

    private readonly DispatcherTimer _timer;
    private double _angle;

    static HudCore()
    {
        AffectsRender<HudCore>(LevelProperty, PhaseProperty);
    }

    public HudCore()
    {
        _timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(32) };
        _timer.Tick += (_, _) =>
        {
            _angle = (_angle + Speed()) % 360;
            InvalidateVisual();
        };
    }

    public double Level
    {
        get => GetValue(LevelProperty);
        set => SetValue(LevelProperty, value);
    }

    public string Phase
    {
        get => GetValue(PhaseProperty);
        set => SetValue(PhaseProperty, value);
    }

    protected override void OnAttachedToVisualTree(VisualTreeAttachmentEventArgs e)
    {
        base.OnAttachedToVisualTree(e);
        _timer.Start();
    }

    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        _timer.Stop();
        base.OnDetachedFromVisualTree(e);
    }

    public override void Render(DrawingContext context)
    {
        base.Render(context);
        var width = Bounds.Width;
        var height = Bounds.Height;
        if (width < 8 || height < 8)
            return;

        var center = new Point(width / 2, height / 2);
        var radius = Math.Min(width, height) / 2 - 4;
        var level = Math.Clamp(Level, 0, 1);
        var pulse = Phase == "thinking"
            ? 0.35 + 0.25 * (0.5 + 0.5 * Math.Sin(_angle / 18.0))
            : level;

        using (context.PushTransform(Matrix.CreateTranslation(center.X, center.Y)))
        using (context.PushTransform(Matrix.CreateRotation(_angle * 0.35 * Math.PI / 180)))
        using (context.PushTransform(Matrix.CreateTranslation(-center.X, -center.Y)))
            DrawRing(context, center, radius * 0.94, Parse("#3EE0FF"), 1.1, 2, 7);

        using (context.PushTransform(Matrix.CreateTranslation(center.X, center.Y)))
        using (context.PushTransform(Matrix.CreateRotation(-_angle * 0.8 * Math.PI / 180)))
        using (context.PushTransform(Matrix.CreateTranslation(-center.X, -center.Y)))
            DrawRing(context, center, radius * 0.78, Parse("#8B7CFF"), 1.3, 0, 0);

        DrawArc(context, center, radius * 0.64, _angle, 36 + pulse * 150, Parse("#3EE0FF"), 2.4);
        DrawArc(context, center, radius * 0.64, _angle + 180, 24 + pulse * 110, Parse("#D16BFF"), 1.8);
        DrawArc(context, center, radius * 0.50, -_angle * 1.4, 50 + pulse * 80, Parse("#8B7CFF"), 1.4);

        var core = radius * (0.16 + pulse * 0.05);
        context.DrawEllipse(
            new RadialGradientBrush
            {
                GradientStops =
                {
                    new GradientStop(Parse("#883EE0FF"), 0),
                    new GradientStop(Parse("#003EE0FF"), 1),
                },
            },
            null,
            center,
            core,
            core);
        context.DrawEllipse(null, new Pen(new SolidColorBrush(Parse("#CC3EE0FF")), 1.4), center, core * 0.72, core * 0.72);
    }

    private double Speed() => Phase switch
    {
        "listening" => 2.4,
        "thinking" => 4.2,
        "speaking" => 1.8,
        _ => 0.45,
    };

    private static void DrawRing(DrawingContext context, Point center, double radius, Color color, double thickness, double dash, double gap)
    {
        var pen = new Pen(new SolidColorBrush(color), thickness);
        if (dash > 0)
            pen.DashStyle = new DashStyle([dash, gap], 0);
        context.DrawEllipse(null, pen, center, radius, radius);
    }

    private static void DrawArc(DrawingContext context, Point center, double radius, double startDeg, double sweepDeg, Color color, double thickness)
    {
        var start = startDeg * Math.PI / 180;
        var sweep = Math.Clamp(sweepDeg, 1, 340) * Math.PI / 180;
        var begin = new Point(center.X + radius * Math.Cos(start), center.Y + radius * Math.Sin(start));
        var end = new Point(center.X + radius * Math.Cos(start + sweep), center.Y + radius * Math.Sin(start + sweep));
        var geometry = new StreamGeometry();
        using (var figure = geometry.Open())
        {
            figure.BeginFigure(begin, false);
            figure.ArcTo(end, new Size(radius, radius), 0, sweep > Math.PI, SweepDirection.Clockwise);
        }

        context.DrawGeometry(null, new Pen(new SolidColorBrush(color), thickness)
        {
            LineCap = PenLineCap.Round,
        }, geometry);
    }

    private static Color Parse(string hex) => Color.Parse(hex);
}
