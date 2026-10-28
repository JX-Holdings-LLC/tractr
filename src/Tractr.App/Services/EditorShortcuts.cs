using Avalonia.Input;

namespace Tractr.App.Services;

/// <summary>Editor commands use Command on macOS and Control elsewhere.</summary>
public static class EditorShortcuts
{
    public static KeyModifiers CommandModifier => OperatingSystem.IsMacOS() ? KeyModifiers.Meta : KeyModifiers.Control;
    public static KeyGesture NewProject => new(Key.N, CommandModifier);
    public static KeyGesture OpenProject => new(Key.O, CommandModifier);
    public static KeyGesture ExportGame => new(Key.E, CommandModifier | KeyModifiers.Shift);
    public static KeyGesture Quit => OperatingSystem.IsMacOS() ? new(Key.Q, KeyModifiers.Meta) : new(Key.F4, KeyModifiers.Alt);

    public static string Display(string keys) => OperatingSystem.IsMacOS()
        ? keys.Replace("Ctrl+", "Cmd+", StringComparison.Ordinal).Replace("Alt+F4", "Cmd+Q", StringComparison.Ordinal)
        : keys;
}
