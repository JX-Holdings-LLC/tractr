using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Controls.Documents;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.VisualTree;
using Tractr.App.Markdown;

namespace Tractr.App.Controls;

/// <summary>
/// Renders Markdown (see <see cref="MarkdownParser"/>) as selectable, wrapped text blocks.
/// Text blocks get the style classes <c>md-h1</c>…<c>md-h3</c>, <c>md-p</c>, <c>md-li</c>,
/// <c>md-quote</c> and <c>md-code</c> so the look is controlled from <c>App.axaml</c>.
/// Images come from <see cref="ImageSource"/>; one it has no bytes for is shown as a link.
/// </summary>
public sealed class MarkdownView : Border
{
    public static readonly StyledProperty<string?> MarkdownProperty =
        AvaloniaProperty.Register<MarkdownView, string?>(nameof(Markdown));

    private Func<string, Stream?>? imageSource;

    private static readonly FontFamily MonoFont = new("avares://Tractr/Assets/Fonts#Courier Prime, fonts:Inter#Inter");

    static MarkdownView()
    {
        MarkdownProperty.Changed.AddClassHandler<MarkdownView>((view, _) => view.Rebuild());
    }

    public MarkdownView()
    {
        Rebuild();
    }

    public string? Markdown
    {
        get => GetValue(MarkdownProperty);
        set => SetValue(MarkdownProperty, value);
    }

    /// <summary>Opens the bytes of an image the Markdown names by its URL, or returns null when there are none.</summary>
    public Func<string, Stream?>? ImageSource
    {
        get => imageSource;
        set
        {
            imageSource = value;
            Rebuild();
        }
    }

    /// <summary>The rendered blocks (for tests).</summary>
    public StackPanel Document { get; private set; } = new();

    /// <summary>A link in the text was clicked; the host decides how to open it.</summary>
    public event EventHandler<string>? LinkClicked;

    private readonly Dictionary<string, Control> anchors = new(StringComparer.Ordinal);

    /// <summary>
    /// Scrolls the heading a <c>#fragment</c> link names into view (GitHub's anchors: lower case, punctuation
    /// dropped, spaces as dashes). False when no heading has that anchor.
    /// </summary>
    public bool ScrollToAnchor(string fragment)
    {
        ArgumentNullException.ThrowIfNull(fragment);
        if (!anchors.TryGetValue(fragment.TrimStart('#'), out var heading))
        {
            return false;
        }

        // Put the heading at the top of the scroller holding the document, as a browser does.
        if (this.FindAncestorOfType<ScrollViewer>() is { Content: Visual content } scroller
            && heading.TranslatePoint(default, content) is { } top)
        {
            scroller.Offset = new Vector(scroller.Offset.X, Math.Max(0, top.Y - 8));
        }
        else
        {
            heading.BringIntoView();
        }

        return true;
    }

    /// <summary>The anchor GitHub gives a heading with this text.</summary>
    public static string Anchor(string heading)
    {
        ArgumentNullException.ThrowIfNull(heading);
        var slug = new System.Text.StringBuilder(heading.Length);
        foreach (var c in heading.Trim().ToLowerInvariant())
        {
            if (char.IsLetterOrDigit(c) || c is '-' or '_')
            {
                slug.Append(c);
            }
            else if (c == ' ')
            {
                slug.Append('-');
            }
        }

        return slug.ToString();
    }

    private void Rebuild()
    {
        var panel = new StackPanel { Spacing = 6 };
        anchors.Clear();
        var blocks = MarkdownParser.Parse(Markdown);
        for (var index = 0; index < blocks.Count; index++)
        {
            var block = blocks[index];
            var control = block switch
            {
                MarkdownHeading h => Text(h.Inlines, $"md-h{Math.Min(3, h.Level)}", topMargin: index == 0 ? 0 : 8),
                MarkdownParagraph p => Text(p.Inlines, "md-p"),
                MarkdownListItem li => ListItem(li),
                MarkdownQuote q => Quote(q),
                MarkdownCodeBlock code => CodeBlock(code),
                MarkdownRule => new Separator { Margin = new Thickness(0, 6) },
                MarkdownImage image => Image(image),
                MarkdownTable table => Table(table),
                _ => null,
            };
            if (control is not null)
            {
                panel.Children.Add(control);
            }

            if (block is MarkdownHeading heading && control is not null)
            {
                anchors.TryAdd(Anchor(string.Concat(heading.Inlines.Select(i => i.Text))), control);
            }
        }

        Document = panel;
        Child = panel;
    }

    private SelectableTextBlock Text(IReadOnlyList<MarkdownInline> inlines, string styleClass, double topMargin = 0)
    {
        var block = new SelectableTextBlock
        {
            TextWrapping = TextWrapping.Wrap,
            Classes = { styleClass },
            Margin = new Thickness(0, topMargin, 0, 0),
            Inlines = [],
        };
        foreach (var inline in inlines)
        {
            block.Inlines!.Add(ToInline(inline));
        }

        return block;
    }

    private Inline ToInline(MarkdownInline inline)
    {
        if (inline.LinkUrl is { } url)
        {
            var link = new HyperlinkButton
            {
                Content = inline.Text,
                Padding = new Thickness(0),
                Margin = new Thickness(0),
                MinHeight = 0,
                FontSize = 13.5,
                VerticalAlignment = VerticalAlignment.Center,
                Classes = { "md-link" },
            };
            ToolTip.SetTip(link, url);
            link.Click += (_, _) => LinkClicked?.Invoke(this, url);
            return new InlineUIContainer { Child = link, BaselineAlignment = BaselineAlignment.Baseline };
        }

        return ToRun(inline);
    }

    private static Run ToRun(MarkdownInline inline)
    {
        var run = new Run(inline.Text);
        if (inline.Bold)
        {
            run.FontWeight = FontWeight.SemiBold;
        }

        if (inline.Italic)
        {
            run.FontStyle = FontStyle.Italic;
        }

        if (inline.Code)
        {
            run.FontFamily = MonoFont;
            run.Background = Brush("FarmCodeBackgroundBrush", Color.FromRgb(0xEE, 0xE8, 0xDA));
        }

        return run;
    }

    private Control ListItem(MarkdownListItem item)
    {
        var grid = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("Auto,*"),
            Margin = new Thickness(8 + (item.Depth * 18), 0, 0, 0),
        };
        var marker = new TextBlock
        {
            Text = item.Marker,
            Classes = { "md-marker" },
            MinWidth = item.Marker == "•" ? 14 : 22,
            VerticalAlignment = VerticalAlignment.Top,
        };
        var text = Text(item.Inlines, "md-li");
        Grid.SetColumn(text, 1);
        grid.Children.Add(marker);
        grid.Children.Add(text);
        return grid;
    }

    private Control Table(MarkdownTable table)
    {
        var columns = table.Rows.Count == 0 ? 0 : table.Rows.Max(row => row.Count);
        var grid = new Grid { Classes = { "md-table" }, Margin = new Thickness(0, 2) };
        // Columns share the width by how much text they hold, so short label columns stay narrow.
        for (var c = 0; c < columns; c++)
        {
            var longest = table.Rows.Max(row => c < row.Count ? row[c].Sum(i => i.Text.Length) : 0);
            grid.ColumnDefinitions.Add(new ColumnDefinition(Math.Clamp(longest, 8, 60), GridUnitType.Star));
        }

        for (var r = 0; r < table.Rows.Count; r++)
        {
            grid.RowDefinitions.Add(new RowDefinition(GridLength.Auto));
            var header = r == 0 && table.HasHeader;
            for (var c = 0; c < table.Rows[r].Count; c++)
            {
                IReadOnlyList<MarkdownInline> inlines = header ? [.. table.Rows[r][c].Select(i => i with { Bold = true })] : table.Rows[r][c];
                var cell = new Border
                {
                    Classes = { header ? "md-table-header" : "md-table-cell" },
                    Padding = new Thickness(0, 3, 12, 3),
                    BorderBrush = Brush("FarmBorderBrush", Color.FromRgb(0xC9, 0xB4, 0x8A)),
                    BorderThickness = new Thickness(0, 0, 0, header ? 1 : 0),
                    Child = Text(inlines, "md-li"),
                };
                Grid.SetRow(cell, r);
                Grid.SetColumn(cell, c);
                grid.Children.Add(cell);
            }
        }

        return grid;
    }

    private Control Image(MarkdownImage image)
    {
        if (LoadImage(image.Url) is { } bitmap)
        {
            var control = new Image
            {
                Source = bitmap,
                Stretch = Stretch.Uniform,
                StretchDirection = StretchDirection.DownOnly,
                HorizontalAlignment = HorizontalAlignment.Left,
                Margin = new Thickness(0, 4),
                Classes = { "md-image" },
            };
            AutomationProperties.SetName(control, image.Alt);
            ToolTip.SetTip(control, image.Alt);
            return control;
        }

        var alt = image.Alt.Length > 0 ? image.Alt : image.Url;
        return Text([new MarkdownInline(alt, LinkUrl: image.Url)], "md-p");
    }

    private Bitmap? LoadImage(string url)
    {
        try
        {
            using var stream = imageSource?.Invoke(url);
            return stream is null ? null : new Bitmap(stream);
        }
        catch (Exception ex) when (ex is IOException or ArgumentException or InvalidOperationException or NotSupportedException)
        {
            return null;
        }
    }

    private Control Quote(MarkdownQuote quote) => new Border
    {
        Classes = { "md-quote" },
        Child = Text(quote.Inlines, "md-quote-text"),
    };

    private static Control CodeBlock(MarkdownCodeBlock code) => new Border
    {
        Classes = { "md-code" },
        Child = new SelectableTextBlock
        {
            Text = code.Code,
            FontFamily = MonoFont,
            TextWrapping = TextWrapping.Wrap,
            Classes = { "md-code-text" },
        },
    };

    private static IBrush Brush(string key, Color fallback) =>
        Application.Current?.TryGetResource(key, Application.Current.ActualThemeVariant, out var value) == true && value is IBrush brush
            ? brush
            : new SolidColorBrush(fallback);
}
