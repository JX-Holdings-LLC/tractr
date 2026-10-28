using Avalonia.Controls;
using Avalonia.Data;
using Avalonia.Data.Converters;
using Avalonia.Interactivity;

namespace Tractr.App.Services;

/// <summary>Mirror the editor menu in the macOS menu bar, sharing commands and language state.</summary>
internal static class NativeMenus
{
    private static readonly FuncValueConverter<object?, string> Header = new(value =>
        (value?.ToString() ?? "").Replace("_", "", StringComparison.Ordinal));

    public static NativeMenu Create(IEnumerable<object?> items)
    {
        var result = new NativeMenu();
        foreach (var item in items)
        {
            if (item is Separator)
            {
                result.Items.Add(new NativeMenuItemSeparator());
            }
            else if (item is MenuItem source)
            {
                var native = new NativeMenuItem();
                native.Bind(NativeMenuItem.HeaderProperty, new Binding(nameof(MenuItem.Header)) { Source = source, Converter = Header });
                native.Bind(NativeMenuItem.CommandProperty, new Binding(nameof(MenuItem.Command)) { Source = source });
                native.Bind(NativeMenuItem.CommandParameterProperty, new Binding(nameof(MenuItem.CommandParameter)) { Source = source });
                native.Bind(NativeMenuItem.GestureProperty, new Binding(nameof(MenuItem.InputGesture)) { Source = source });
                native.Bind(NativeMenuItem.IsCheckedProperty, new Binding(nameof(MenuItem.IsChecked)) { Source = source });
                native.Bind(NativeMenuItem.ToggleTypeProperty, new Binding(nameof(MenuItem.ToggleType)) { Source = source });
                native.Bind(NativeMenuItem.IsEnabledProperty, new Binding(nameof(MenuItem.IsEnabled)) { Source = source });
                if (source.Items.Count > 0)
                {
                    native.Menu = Create(source.Items);
                }

                // Language choices use Click handlers; commands execute via the shared binding.
                native.Click += (_, _) =>
                {
                    if (source.Command is null)
                    {
                        source.RaiseEvent(new RoutedEventArgs(MenuItem.ClickEvent));
                    }
                };
                result.Items.Add(native);
            }
        }

        return result;
    }
}
