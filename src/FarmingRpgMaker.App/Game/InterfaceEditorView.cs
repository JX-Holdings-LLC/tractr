using System.Collections.Specialized;
using System.ComponentModel;
using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Data;
using FarmEngine.Authoring;
using FarmingRpgMaker.App.Projects;
using FarmingRpgMaker.App.ViewModels.EditMode;

namespace FarmingRpgMaker.App.Game;

/// <summary>The Interface tab: shows and collects the panel draft of its <see cref="InterfaceViewModel"/>.</summary>
public sealed class InterfaceEditorView : UserControl, IRetirable
{
    /// <summary>
    /// One entry's controls: its label, its kind, and the value control that kind needs in
    /// <paramref name="Value"/> (empty for live counters).
    /// </summary>
    private sealed record EntryRow(PanelEntryDraft Entry, TextBox Label, ComboBox Kind, ContentControl Value, Button Remove, Control Control);
    private readonly ListBox _panels = new() { Name = "InterfacePanels", MinHeight = 130 };
    private readonly StackPanel _entries = new() { Name = "InterfaceEntries", Spacing = 8 };
    private readonly List<EntryRow> _rows = [];
    private bool _refreshing;

    public InterfaceEditorView(ProjectWorkspace workspace)
    {
        ViewModel = new InterfaceViewModel(workspace, () => IsEffectivelyVisible);
        DataContext = ViewModel;
        Name = "InterfaceEditorView";
        var title = new TextBox { Name = "InterfaceTitle" };
        title.Bind(TextBox.TextProperty, new Binding(nameof(InterfaceViewModel.Title)) { Mode = BindingMode.TwoWay });
        var flag = new TextBox { Name = "InterfaceVisibleFlag", PlaceholderText = "Optional story flag" };
        flag.Bind(TextBox.TextProperty, new Binding(nameof(InterfaceViewModel.VisibleFlag)) { Mode = BindingMode.TwoWay });
        var message = Ui.Wrapped("", "muted", "small");
        message.Name = "InterfaceMessage";
        message.Bind(TextBlock.TextProperty, new Binding(nameof(InterfaceViewModel.Message)));
        Ui.Label((_panels, "Game panels"), (title, "Panel title"), (flag, "Show after story flag (optional)"));
        var draftBar = new DraftBar("Interface", ViewModel.Draft);
        _panels.SelectionChanged += (_, _) =>
        {
            if (_refreshing) return;
            var id = (_panels.SelectedItem as ListBoxItem)?.Tag as string;
            if (id == ViewModel.SelectedId) return;
            // The list shows the panel the fields are about until the view model moves on.
            ShowSelection();
            ViewModel.SelectPanel(id);
        };
        var left = new StackPanel { Spacing = 10, Margin = new Thickness(0, 0, 20, 0) };
        left.Children.Add(Ui.Text("GAME PANELS", "section"));
        left.Children.Add(Ui.Wrapped("Panels appear below the game and can show live counters, story status or action buttons.", "muted", "small"));
        left.Children.Add(_panels);
        var add = Ui.CommandButton("Add panel", ViewModel.AddPanelCommand, "accent");
        add.Name = "AddInterfacePanelButton";
        var delete = Ui.CommandButton("Remove panel", ViewModel.RemovePanelCommand, "tool");
        delete.Name = "RemoveInterfacePanelButton";
        left.Children.Add(Ui.HStack(8, add, delete));

        var right = new StackPanel { Spacing = 10 };
        right.Children.Add(Ui.Text("PANEL DETAILS", "section"));
        right.Children.Add(draftBar);
        right.Children.Add(Ui.Text("Title", "muted", "small"));
        right.Children.Add(title);
        right.Children.Add(Ui.Text("Show after story flag (optional)", "muted", "small"));
        right.Children.Add(flag);
        right.Children.Add(Ui.Text("ENTRIES", "section"));
        right.Children.Add(_entries);
        var addEntry = Ui.CommandButton("Add entry", ViewModel.AddEntryCommand, "tool");
        addEntry.Name = "AddInterfaceEntryButton";
        right.Children.Add(addEntry);
        right.Children.Add(Ui.Wrapped("Kinds: text shows what you type; money, energy and day show live values; item counts that item in the bag; flag shows Yes or No for a story flag; action adds a button that runs the action.", "muted", "small"));
        var save = Ui.CommandButton("Save panel", ViewModel.SaveCommand, "accent");
        save.Name = "SaveInterfacePanelButton";
        right.Children.Add(save);
        right.Children.Add(message);
        var grid = new Grid { ColumnDefinitions = new ColumnDefinitions("245,*"), Margin = new Thickness(20) };
        grid.Children.Add(left);
        var scroll = new ScrollViewer { Content = right };
        Grid.SetColumn(scroll, 1);
        grid.Children.Add(scroll);
        Content = grid;
        ShowPanels();
        ShowEntries();
        ViewModel.PropertyChanged += OnViewModelChanged;
        ViewModel.Entries.CollectionChanged += OnEntriesChanged;
    }

    public InterfaceViewModel ViewModel { get; }

    /// <summary>Stops following the project (the editor that built this view was replaced).</summary>
    public void Retire() => ViewModel.Retire();

    public void Refresh() => ViewModel.Refresh();

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(InterfaceViewModel.Panels)) ShowPanels();
        if (e.PropertyName == nameof(InterfaceViewModel.SelectedId)) ShowSelection();
    }

    private void ShowPanels()
    {
        _refreshing = true;
        try
        {
            _panels.SelectedItem = null;
            _panels.Items.Clear();
            foreach (var panel in ViewModel.Panels) _panels.Items.Add(new ListBoxItem { Content = panel.Title, Tag = panel.Id });
        }
        finally { _refreshing = false; }
        ShowSelection();
    }

    private void ShowSelection()
    {
        _refreshing = true;
        try
        {
            _panels.SelectedItem = _panels.Items.OfType<ListBoxItem>().FirstOrDefault(item => Equals(item.Tag, ViewModel.SelectedId));
        }
        finally { _refreshing = false; }
    }

    private void OnEntriesChanged(object? sender, NotifyCollectionChangedEventArgs e)
    {
        // Rows keep their controls while the draft gains or loses entries; a reload rebuilds them.
        if (e.Action == NotifyCollectionChangedAction.Add && e.NewItems is { } added)
        {
            foreach (PanelEntryDraft entry in added) AddRow(entry);
        }
        else if (e.Action == NotifyCollectionChangedAction.Remove && e.OldItems is { } removed)
        {
            foreach (PanelEntryDraft entry in removed)
            {
                if (_rows.FirstOrDefault(row => ReferenceEquals(row.Entry, entry)) is not { } row) continue;
                row.Entry.PropertyChanged -= OnEntryChanged;
                _rows.Remove(row);
                _entries.Children.Remove(row.Control);
            }
        }
        else
        {
            ShowEntries();
        }

        Renumber();
    }

    private void ShowEntries()
    {
        foreach (var row in _rows) row.Entry.PropertyChanged -= OnEntryChanged;
        _rows.Clear();
        _entries.Children.Clear();
        foreach (var entry in ViewModel.Entries) AddRow(entry);
        Renumber();
    }

    private void AddRow(PanelEntryDraft entry)
    {
        var label = new TextBox { Name = "InterfaceEntryLabel", PlaceholderText = "Label", MinWidth = 115 };
        label.Bind(TextBox.TextProperty, new Binding(nameof(PanelEntryDraft.Label)) { Source = entry, Mode = BindingMode.TwoWay });
        var kind = new ComboBox { Name = "InterfaceEntryKind", MinWidth = 85 };
        foreach (var value in InterfaceViewModel.Kinds) kind.Items.Add(new ComboBoxItem { Content = value, Tag = value });
        kind.SelectedItem = kind.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, entry.Kind));
        kind.SelectionChanged += (_, _) =>
        {
            if (kind.SelectedItem is ComboBoxItem { Tag: string tag }) entry.Kind = tag;
        };
        var valueHost = new ContentControl { Name = "InterfaceEntryValueHost" };
        ShowValue(valueHost, entry);
        var remove = Ui.Button("×", () => ViewModel.RemoveEntry(entry), "tool", "small");
        remove.Name = "InterfaceEntryRemove";
        ToolTip.SetTip(remove, "Remove this entry");
        var row = new EntryRow(entry, label, kind, valueHost, remove, Ui.HStack(8, label, kind, valueHost, remove));
        entry.PropertyChanged += OnEntryChanged;
        _rows.Add(row);
        _entries.Children.Add(row.Control);
    }

    private void OnEntryChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName != nameof(PanelEntryDraft.Kind) || _rows.FirstOrDefault(row => ReferenceEquals(row.Entry, sender)) is not { } row) return;
        ShowValue(row.Value, row.Entry);
        Renumber();
    }

    /// <summary>Puts the control the entry's kind edits its value with in the row; a kind without a value takes no room.</summary>
    private void ShowValue(ContentControl host, PanelEntryDraft entry)
    {
        Control? control = entry.Editor switch
        {
            PanelValueEditor.Picker => Picker(entry),
            PanelValueEditor.Text => Bound(new TextBox { Name = "InterfaceEntryValue", PlaceholderText = InterfaceViewModel.Placeholder(entry.Kind), MinWidth = 150 }, entry),
            _ => null,
        };
        host.Content = control;
        host.IsVisible = control is not null;
    }

    private static TextBox Bound(TextBox box, PanelEntryDraft entry)
    {
        box.Bind(TextBox.TextProperty, new Binding(nameof(PanelEntryDraft.Value)) { Source = entry, Mode = BindingMode.TwoWay });
        return box;
    }

    /// <summary>A picker of the project's items or actions; an unknown id shows as "(missing: id)".</summary>
    private ComboBox Picker(PanelEntryDraft entry)
    {
        var picker = new ComboBox { Name = "InterfaceEntryValue", MinWidth = 150, PlaceholderText = InterfaceViewModel.Placeholder(entry.Kind), MaxDropDownHeight = 320 };
        foreach (var option in ViewModel.ValueOptions(entry.Kind, entry.Value))
        {
            var item = new ComboBoxItem { Content = option.Label, Tag = option.Id };
            if (option.Missing) item.Classes.Add("missing");
            picker.Items.Add(item);
        }

        picker.SelectedItem = picker.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, entry.Value));
        picker.SelectionChanged += (_, _) => entry.Value = (picker.SelectedItem as ComboBoxItem)?.Tag as string ?? "";
        return picker;
    }

    /// <summary>Screen reader names that follow the rows' order ("Entry 2 item", "Remove entry 2").</summary>
    private void Renumber()
    {
        for (var i = 0; i < _rows.Count; i++)
        {
            var row = _rows[i];
            var names = InterfaceViewModel.NamesFor(i, row.Entry.Kind);
            AutomationProperties.SetName(row.Label, names.Label);
            AutomationProperties.SetName(row.Kind, names.Kind);
            AutomationProperties.SetName(row.Remove, names.Remove);
            if (row.Value.Content is Control value) AutomationProperties.SetName(value, names.Value);
        }
    }
}
