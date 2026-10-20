using System.Collections.ObjectModel;
using System.Collections.Specialized;
using Avalonia.Controls;

namespace FarmingRpgMaker.App.Game;

/// <summary>
/// Keeps a panel's rows in step with a view model's draft rows: a row keeps its controls while
/// others are added, removed or moved (so focus and screen reader position stay), and a reset
/// rebuilds them all.
/// </summary>
/// <typeparam name="T">The view model's row type.</typeparam>
internal sealed class DraftRows<T>
    where T : class
{
    private readonly Panel _panel;
    private readonly ObservableCollection<T> _items;
    private readonly Func<T, Control> _build;
    private readonly Action? _changed;
    private readonly List<(T Item, Control Control)> _rows = [];

    /// <param name="panel">Where the rows go (nothing else may be in it).</param>
    /// <param name="items">The view model's rows.</param>
    /// <param name="build">Builds one row's controls.</param>
    /// <param name="changed">Runs after the rows changed (renumbering, arrows).</param>
    public DraftRows(Panel panel, ObservableCollection<T> items, Func<T, Control> build, Action? changed = null)
    {
        _panel = panel;
        _items = items;
        _build = build;
        _changed = changed;
        items.CollectionChanged += OnChanged;
        Rebuild();
    }

    /// <summary>The rows on show, in order.</summary>
    public IReadOnlyList<(T Item, Control Control)> Rows => _rows;

    private void OnChanged(object? sender, NotifyCollectionChangedEventArgs e)
    {
        switch (e.Action)
        {
            case NotifyCollectionChangedAction.Add when e.NewItems is { } added:
                var index = e.NewStartingIndex;
                foreach (T item in added)
                {
                    var row = (item, _build(item));
                    _rows.Insert(index, row);
                    _panel.Children.Insert(index, row.Item2);
                    index++;
                }

                break;
            case NotifyCollectionChangedAction.Remove when e.OldItems is { Count: 1 }:
                _rows.RemoveAt(e.OldStartingIndex);
                _panel.Children.RemoveAt(e.OldStartingIndex);
                break;
            case NotifyCollectionChangedAction.Move when e.OldItems is { Count: 1 }:
                var moved = _rows[e.OldStartingIndex];
                _rows.RemoveAt(e.OldStartingIndex);
                _rows.Insert(e.NewStartingIndex, moved);
                _panel.Children.Remove(moved.Control);
                _panel.Children.Insert(e.NewStartingIndex, moved.Control);
                break;
            default:
                Rebuild();
                return;
        }

        _changed?.Invoke();
    }

    private void Rebuild()
    {
        _rows.Clear();
        _panel.Children.Clear();
        foreach (var item in _items)
        {
            var control = _build(item);
            _rows.Add((item, control));
            _panel.Children.Add(control);
        }

        _changed?.Invoke();
    }
}
