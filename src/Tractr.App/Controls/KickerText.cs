using Avalonia.Media.TextFormatting;

namespace Tractr.App.Controls;

/// <summary>
/// A <see cref="Avalonia.Controls.TextBlock"/> that draws its text in capitals: Tractr's label voice
/// (small tracked Oswald caps for kickers, section labels and headings). <c>Text</c> keeps the
/// original casing, so screen readers, tests and bindings see the real string.
/// </summary>
public class KickerText : Avalonia.Controls.TextBlock
{
    protected override Type StyleKeyOverride => typeof(Avalonia.Controls.TextBlock);

    protected override TextLayout CreateTextLayout(string? text) => base.CreateTextLayout(text?.ToUpperInvariant());
}
