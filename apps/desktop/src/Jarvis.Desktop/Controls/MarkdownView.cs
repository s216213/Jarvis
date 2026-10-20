using System.Diagnostics;
using System.Text;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Documents;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Markdig;
using Markdig.Extensions.Tables;
using Markdig.Extensions.TaskLists;
using Markdig.Helpers;
using Markdig.Syntax;
using Markdig.Syntax.Inlines;
using MdInline = Markdig.Syntax.Inlines.Inline;

namespace Jarvis.Desktop.Controls;

public sealed class MarkdownView : ContentControl
{
    public static readonly StyledProperty<string?> SourceProperty =
        AvaloniaProperty.Register<MarkdownView, string?>(nameof(Source));

    private static readonly MarkdownPipeline Pipeline = new MarkdownPipelineBuilder()
        .UseAdvancedExtensions()
        .DisableHtml()
        .Build();

    private static readonly Cursor Hand = new(StandardCursorType.Hand);

    public MarkdownView()
    {
        HorizontalAlignment = HorizontalAlignment.Stretch;
    }

    public string? Source
    {
        get => GetValue(SourceProperty);
        set => SetValue(SourceProperty, value);
    }

    static MarkdownView()
    {
        SourceProperty.Changed.AddClassHandler<MarkdownView>((view, _) => view.Rebuild());
    }

    private void Rebuild()
    {
        var root = new StackPanel { Spacing = 8 };
        var source = Source;
        if (!string.IsNullOrEmpty(source))
        {
            var document = Markdown.Parse(source, Pipeline);
            foreach (var block in document)
                AddBlock(root, block);
        }

        Content = root;
    }

    private void AddBlock(StackPanel parent, Block block)
    {
        switch (block)
        {
            case HeadingBlock heading:
                var title = Paragraph(heading.Inline);
                title.Classes.Add("md-h");
                title.Classes.Add($"md-h{Math.Clamp(heading.Level, 1, 4)}");
                parent.Children.Add(title);
                break;
            case ParagraphBlock paragraph:
                parent.Children.Add(Paragraph(paragraph.Inline));
                break;
            case ListBlock list:
                parent.Children.Add(RenderList(list));
                break;
            case FencedCodeBlock or CodeBlock:
                parent.Children.Add(RenderCode((LeafBlock)block));
                break;
            case QuoteBlock quote:
                var quoteBody = new StackPanel { Spacing = 6 };
                foreach (var child in quote)
                    AddBlock(quoteBody, child);
                parent.Children.Add(new Border { Classes = { "md-quote" }, Child = quoteBody });
                break;
            case ThematicBreakBlock:
                parent.Children.Add(new Border { Classes = { "md-hr" } });
                break;
            case Table table:
                parent.Children.Add(RenderTable(table));
                break;
            case LeafBlock leaf when leaf.Inline is not null:
                parent.Children.Add(Paragraph(leaf.Inline));
                break;
            case ContainerBlock container:
                foreach (var child in container)
                    AddBlock(parent, child);
                break;
        }
    }

    private Control RenderList(ListBlock list)
    {
        var panel = new StackPanel { Spacing = 4 };
        var index = 1;
        if (list.IsOrdered && int.TryParse(list.OrderedStart, out var start))
            index = start;

        foreach (var item in list.OfType<ListItemBlock>())
        {
            var row = new Grid { ColumnSpacing = 8 };
            row.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Auto));
            row.ColumnDefinitions.Add(new ColumnDefinition(new GridLength(1, GridUnitType.Star)));

            var marker = list.IsOrdered ? $"{index}." : "•";
            if (item.FirstOrDefault() is ParagraphBlock paragraph &&
                paragraph.Inline?.FirstChild is TaskList task)
                marker = task.Checked ? "☑" : "☐";
            index++;

            var bullet = new TextBlock
            {
                Text = marker,
                Classes = { "md-marker" },
                VerticalAlignment = VerticalAlignment.Top,
            };
            Grid.SetColumn(bullet, 0);

            var body = new StackPanel { Spacing = 4 };
            foreach (var child in item)
                AddBlock(body, child);
            Grid.SetColumn(body, 1);

            row.Children.Add(bullet);
            row.Children.Add(body);
            panel.Children.Add(row);
        }

        return panel;
    }

    private Control RenderCode(LeafBlock code)
    {
        var text = new TextBlock
        {
            Text = LinesToString(code.Lines),
            Classes = { "md-code" },
        };
        var scroller = new ScrollViewer
        {
            HorizontalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Auto,
            VerticalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Disabled,
            Content = text,
        };
        return new Border { Classes = { "md-pre" }, Child = scroller };
    }

    private Control RenderTable(Table table)
    {
        var rows = table.OfType<TableRow>().ToList();
        var columnCount = rows.Count == 0 ? 0 : rows.Max(row => row.Count);
        var grid = new Grid();
        for (var column = 0; column < columnCount; column++)
            grid.ColumnDefinitions.Add(new ColumnDefinition(new GridLength(1, GridUnitType.Star)));
        for (var row = 0; row < rows.Count; row++)
            grid.RowDefinitions.Add(new RowDefinition(GridLength.Auto));

        for (var rowIndex = 0; rowIndex < rows.Count; rowIndex++)
        {
            var row = rows[rowIndex];
            var columnIndex = 0;
            foreach (var cell in row.OfType<TableCell>())
            {
                var body = new StackPanel { Spacing = 4 };
                foreach (var child in cell)
                    AddBlock(body, child);
                var frame = new Border
                {
                    Classes = { row.IsHeader ? "md-th" : "md-td" },
                    Child = body,
                };
                Grid.SetRow(frame, rowIndex);
                Grid.SetColumn(frame, columnIndex);
                if (cell.ColumnSpan > 1)
                    Grid.SetColumnSpan(frame, cell.ColumnSpan);
                grid.Children.Add(frame);
                columnIndex += Math.Max(1, cell.ColumnSpan);
            }
        }

        return new ScrollViewer
        {
            HorizontalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Auto,
            VerticalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Disabled,
            Content = grid,
        };
    }

    private TextBlock Paragraph(ContainerInline? inline)
    {
        var text = new TextBlock { Classes = { "md-p" } };
        if (inline is not null)
            AppendInlines(text.Inlines!, inline);
        return text;
    }

    private void AppendInlines(InlineCollection target, ContainerInline container)
    {
        foreach (var inline in container)
            AppendInline(target, inline);
    }

    private void AppendInline(InlineCollection target, MdInline inline)
    {
        switch (inline)
        {
            case LiteralInline literal:
                target.Add(new Run(literal.Content.ToString()));
                break;
            case LineBreakInline:
                target.Add(new LineBreak());
                break;
            case TaskList:
                break;
            case CodeInline code:
                target.Add(new InlineUIContainer(InlineCode(code.Content)));
                break;
            case EmphasisInline emphasis:
                var span = new Span();
                if (emphasis.DelimiterChar == '~')
                    span.TextDecorations = TextDecorations.Strikethrough;
                else
                {
                    if (emphasis.DelimiterCount >= 2)
                        span.FontWeight = FontWeight.SemiBold;
                    if (emphasis.DelimiterCount % 2 == 1)
                        span.FontStyle = FontStyle.Italic;
                }

                AppendInlines(span.Inlines, emphasis);
                target.Add(span);
                break;
            case LinkInline link:
                AppendLink(target, link);
                break;
            case AutolinkInline autolink:
                AppendSafeLink(target, autolink.Url, autolink.Url);
                break;
            case HtmlEntityInline entity:
                target.Add(new Run(entity.Transcoded.ToString()));
                break;
            case ContainerInline nested:
                AppendInlines(target, nested);
                break;
        }
    }

    private void AppendLink(InlineCollection target, LinkInline link)
    {
        if (link.IsImage)
        {
            if (link.FirstChild is not null)
                AppendInlines(target, link);
            return;
        }

        var label = new TextBlock { Classes = { "md-link" } };
        if (link.FirstChild is not null)
            AppendInlines(label.Inlines!, link);
        else
            label.Inlines!.Add(new Run(link.Url ?? ""));

        if (!TrySafeUrl(link.Url, out var uri))
        {
            label.Classes.Remove("md-link");
            label.Classes.Add("md-p");
            target.Add(new InlineUIContainer(label));
            return;
        }

        label.Cursor = Hand;
        label.PointerPressed += (_, eventArgs) =>
        {
            if (!eventArgs.GetCurrentPoint(label).Properties.IsLeftButtonPressed)
                return;
            eventArgs.Handled = true;
            Open(uri);
        };
        target.Add(new InlineUIContainer(label));
    }

    private static void AppendSafeLink(InlineCollection target, string? url, string label)
    {
        if (!TrySafeUrl(url, out var uri))
        {
            target.Add(new Run(label));
            return;
        }

        var text = new TextBlock
        {
            Text = label,
            Classes = { "md-link" },
            Cursor = Hand,
        };
        text.PointerPressed += (_, eventArgs) =>
        {
            if (!eventArgs.GetCurrentPoint(text).Properties.IsLeftButtonPressed)
                return;
            eventArgs.Handled = true;
            Open(uri);
        };
        target.Add(new InlineUIContainer(text));
    }

    private static Border InlineCode(string content) =>
        new()
        {
            Classes = { "md-icode" },
            VerticalAlignment = VerticalAlignment.Center,
            Child = new TextBlock
            {
                Text = content,
                Classes = { "md-code" },
            },
        };

    private static string LinesToString(StringLineGroup lines)
    {
        var builder = new StringBuilder();
        for (var i = 0; i < lines.Count; i++)
        {
            if (i > 0)
                builder.Append('\n');
            builder.Append(lines.Lines[i].Slice.ToString());
        }

        return builder.ToString();
    }

    private static bool TrySafeUrl(string? url, out Uri uri)
    {
        if (Uri.TryCreate(url, UriKind.Absolute, out var parsed) &&
            parsed.Scheme is "http" or "https" or "mailto")
        {
            uri = parsed;
            return true;
        }

        uri = null!;
        return false;
    }

    private static void Open(Uri uri)
    {
        try
        {
            Process.Start(new ProcessStartInfo(uri.AbsoluteUri) { UseShellExecute = true });
        }
        catch
        {
            // Brak skojarzonej aplikacji nie powinien psuć czatu.
        }
    }
}
