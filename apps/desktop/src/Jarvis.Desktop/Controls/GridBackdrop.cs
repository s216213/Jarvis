using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;

namespace Jarvis.Desktop.Controls;

public class GridBackdrop : Control
{
    private static readonly IPen GridPen = new Pen(new SolidColorBrush(Color.FromArgb(12, 130, 170, 255)), 1);

    public override void Render(DrawingContext context)
    {
        base.Render(context);
        var width = Bounds.Width;
        var height = Bounds.Height;
        for (var x = 0.0; x <= width; x += 56)
            context.DrawLine(GridPen, new Point(x, 0), new Point(x, height));
        for (var y = 0.0; y <= height; y += 56)
            context.DrawLine(GridPen, new Point(0, y), new Point(width, y));
    }
}
