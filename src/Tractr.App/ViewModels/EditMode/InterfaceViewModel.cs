using System.Collections.ObjectModel;
using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using Tractr.App.Mvvm;
using Tractr.App.Projects;

namespace Tractr.App.ViewModels.EditMode;

/// <summary>A game panel in the list.</summary>
public sealed record InterfacePanelRow(string Id, string Title);

/// <summary>What an entry's value is edited with: nothing (live counters), text, or a picker of the project's items or actions.</summary>
public enum PanelValueEditor
{
    None,
    Text,
    Picker,
}

/// <summary>The screen reader names of an entry's controls ("Entry 2 label", "Remove entry 2").</summary>
public sealed record PanelEntryNames(string Label, string Kind, string Value, string Remove);

/// <summary>
/// One entry of the panel draft: its label, its kind and the value that kind needs. A new kind
/// needs a different value, so changing the kind clears it (the web does too).
/// </summary>
public sealed class PanelEntryDraft : ObservableObject
{
    private string _label;
    private string _kind;
    private string _value;

    public PanelEntryDraft(GamePanelEntry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);
        _label = entry.Label;
        // A kind the editor doesn't know shows (and saves) as the first one.
        _kind = GamePanelEntryKinds.All.Contains(entry.Kind) ? entry.Kind : GamePanelEntryKinds.All[0];
        _value = entry.Value;
    }

    public string Label
    {
        get => _label;
        set => SetProperty(ref _label, value ?? "");
    }

    /// <summary>One of <see cref="GamePanelEntryKinds.All"/>; setting another clears <see cref="Value"/>.</summary>
    public string Kind
    {
        get => _kind;
        set
        {
            value ??= GamePanelEntryKinds.Text;
            if (value == _kind) return;
            // The value is cleared before anyone hears of the new kind, so its editor starts empty.
            _kind = value;
            _value = "";
            OnPropertiesChanged(nameof(Value), nameof(Kind), nameof(Editor));
        }
    }

    /// <summary>The text, flag name, item or action id, as chosen.</summary>
    public string Value
    {
        get => _value;
        set => SetProperty(ref _value, value ?? "");
    }

    /// <summary>What <see cref="Kind"/> edits its value with.</summary>
    public PanelValueEditor Editor => InterfaceViewModel.EditorFor(_kind);

    /// <summary>The entry as it would be saved (a kind without a value saves none).</summary>
    public GamePanelEntry ToEntry() => GamePanelEntry.Default.WithLabel(_label).WithKind(_kind).WithValue(Editor == PanelValueEditor.None ? "" : _value);
}

/// <summary>
/// The Interface tab: the creator's in-game panels and the draft of the selected one (title,
/// story flag, entries). The draft stays as typed until it is saved or dropped (#87), and
/// choosing another panel asks first (<see cref="Draft"/>). Saving is one F# undo step.
/// </summary>
public sealed class InterfaceViewModel : ObservableObject
{
    /// <summary>The most entries a panel may have.</summary>
    public const int MaxEntries = 40;

    private readonly ProjectWorkspace _workspace;
    private readonly Func<bool> _isShown;
    private IReadOnlyList<InterfacePanelRow> _panels = [];
    private string? _selectedId;
    private string _title = "";
    private string _visibleFlag = "";
    private string _message = "Select or add a panel.";
    private bool _canEdit;

    /// <summary>The panel the draft was filled from (null: none selected).</summary>
    private GamePanel? _shown;

    /// <param name="workspace">The open project.</param>
    /// <param name="isShown">Whether the tab is on show: project changes refresh it only then (opening it refreshes too).</param>
    public InterfaceViewModel(ProjectWorkspace workspace, Func<bool>? isShown = null)
    {
        _workspace = workspace ?? throw new ArgumentNullException(nameof(workspace));
        _isShown = isShown ?? (() => true);
        Draft = new DraftPrompt(SaveDraft, LoadSelected);
        AddPanelCommand = new RelayCommand(() => Navigate(AddNow));
        RemovePanelCommand = new RelayCommand(Delete, () => _canEdit);
        AddEntryCommand = new RelayCommand(AddEntry);
        SaveCommand = new RelayCommand(Save, () => _canEdit);
        _workspace.ProjectChanged += OnProjectChanged;
        Refresh();
    }

    /// <summary>The project's panels.</summary>
    public IReadOnlyList<InterfacePanelRow> Panels
    {
        get => _panels;
        private set => SetProperty(ref _panels, value);
    }

    /// <summary>The selected panel's id (null: none). <see cref="SelectPanel"/> changes it.</summary>
    public string? SelectedId
    {
        get => _selectedId;
        private set => SetProperty(ref _selectedId, value);
    }

    public string Title
    {
        get => _title;
        set => SetProperty(ref _title, value ?? "");
    }

    /// <summary>The story flag after which the panel shows (empty: always).</summary>
    public string VisibleFlag
    {
        get => _visibleFlag;
        set => SetProperty(ref _visibleFlag, value ?? "");
    }

    /// <summary>The draft's entries, in order.</summary>
    public ObservableCollection<PanelEntryDraft> Entries { get; } = [];

    /// <summary>The status line.</summary>
    public string Message
    {
        get => _message;
        private set => SetProperty(ref _message, value);
    }

    /// <summary>The question asked before unsaved fields would be replaced.</summary>
    public DraftPrompt Draft { get; }

    public RelayCommand AddPanelCommand { get; }

    public RelayCommand RemovePanelCommand { get; }

    public RelayCommand AddEntryCommand { get; }

    public RelayCommand SaveCommand { get; }

    /// <summary>True while the panel's fields hold edits that are not saved (#87).</summary>
    public bool HasUnsavedChanges => _shown is { } shown && !Equals(FromFields(shown), shown);

    /// <summary>The kinds an entry can have, in the order the picker lists them.</summary>
    public static IReadOnlyList<string> Kinds { get; } = [.. GamePanelEntryKinds.All];

    /// <summary>What an entry of <paramref name="kind"/> edits its value with.</summary>
    public static PanelValueEditor EditorFor(string kind) => kind switch
    {
        GamePanelEntryKinds.Item or GamePanelEntryKinds.Action => PanelValueEditor.Picker,
        GamePanelEntryKinds.Text or GamePanelEntryKinds.Flag => PanelValueEditor.Text,
        _ => PanelValueEditor.None,
    };

    /// <summary>The hint in an entry's value field or picker.</summary>
    public static string Placeholder(string kind) => kind switch
    {
        GamePanelEntryKinds.Item => "Choose item",
        GamePanelEntryKinds.Action => "Choose action",
        GamePanelEntryKinds.Flag => "Flag name",
        _ => "Text",
    };

    /// <summary>Screen reader names that follow the rows' order; <paramref name="index"/> counts from 0.</summary>
    public static PanelEntryNames NamesFor(int index, string kind)
    {
        var number = index + 1;
        var what = kind == GamePanelEntryKinds.Flag ? "flag name" : kind;
        return new PanelEntryNames($"Entry {number} label", $"Entry {number} kind", $"Entry {number} {what}", $"Remove entry {number}");
    }

    /// <summary>
    /// The choices of an item or action picker: the project's items or actions, and an unknown
    /// <paramref name="value"/> as "(missing: id)" so it stays visible and saved.
    /// </summary>
    public IReadOnlyList<PickerOption> ValueOptions(string kind, string value)
    {
        if (_workspace.Current is not { } project) return [];
        var reference = kind == GamePanelEntryKinds.Action ? "action" : "item";
        var field = ContentForms.Field("GamePanelEntry", "Value", "Value") ?? throw new InvalidOperationException("GamePanelEntry.Value is not declared.");
        return ContentForms.Entries(field, ContentForms.Options(reference, project), value);
    }

    /// <summary>Stops following the project (the editor that built this tab was replaced).</summary>
    public void Retire() => _workspace.ProjectChanged -= OnProjectChanged;

    /// <summary>Saves the panel's unsaved fields, if any. False when they could not be saved; the message says why.</summary>
    public bool SaveDraft()
    {
        if (!HasUnsavedChanges) return true;
        Save();
        return !HasUnsavedChanges;
    }

    /// <summary>Chooses another panel; unsaved fields are not dropped by that (#87): <see cref="Draft"/> asks first.</summary>
    public void SelectPanel(string? id)
    {
        if (id == _selectedId) return;
        Navigate(() =>
        {
            SelectedId = id;
            LoadSelected();
        });
    }

    /// <summary>Catches up with the project; a panel with unsaved fields stays as typed while it still exists.</summary>
    public void Refresh()
    {
        Panels = [.. (_workspace.Current?.GamePanels.OrEmpty() ?? []).Select(panel => new InterfacePanelRow(panel.Id, panel.Title))];
        if (!_panels.Any(panel => panel.Id == _selectedId)) SelectedId = null;
        // Tab switches and undo elsewhere keep the draft.
        if (_shown is { } shown && shown.Id == _selectedId && Selected() is not null && HasUnsavedChanges) return;
        LoadSelected();
    }

    /// <summary>Removes an entry from the draft.</summary>
    public void RemoveEntry(PanelEntryDraft entry) => Entries.Remove(entry);

    /// <summary>The panel as the fields describe it (title and flag trimmed).</summary>
    private GamePanel FromFields(GamePanel panel) =>
        panel.WithTitle(_title.Trim())
            .WithVisibleFlag(string.IsNullOrWhiteSpace(_visibleFlag) ? null : _visibleFlag.Trim())
            .WithEntries(Entries.Select(entry => entry.ToEntry()).ToList());

    private GamePanel? Selected() => _workspace.Current?.GamePanels.OrEmpty().FirstOrDefault(panel => panel.Id == _selectedId);

    private void Navigate(Action next)
    {
        if (!HasUnsavedChanges)
        {
            Draft.Hide();
            next();
            return;
        }

        Draft.Ask($"the panel \"{_shown!.Title}\"", next);
    }

    /// <summary>Fills the draft from the selected panel (dropping what was typed).</summary>
    private void LoadSelected()
    {
        Draft.Hide();
        var panel = Selected();
        _shown = panel;
        Title = panel?.Title ?? "";
        VisibleFlag = panel?.VisibleFlag.OrNull() ?? "";
        Entries.Clear();
        foreach (var entry in panel?.Entries ?? []) Entries.Add(new PanelEntryDraft(entry));
        _canEdit = panel is not null;
        SaveCommand.NotifyCanExecuteChanged();
        RemovePanelCommand.NotifyCanExecuteChanged();
        Message = panel is null ? "Select or add a panel." : $"Editing {panel.Id}";
    }

    private void AddEntry()
    {
        if (Selected() is null) return;
        if (Entries.Count >= MaxEntries)
        {
            Message = $"A panel may have at most {MaxEntries} entries.";
            return;
        }

        Entries.Add(new PanelEntryDraft(GamePanelEntry.Default.WithKind(GamePanelEntryKinds.Text)));
    }

    private void AddNow()
    {
        if (_workspace.Current is not { } project) return;
        var panel = Defaults.NewGamePanel(project);
        SelectedId = panel.Id;
        _workspace.Apply(Edits.SetGamePanels([.. project.GamePanels.OrEmpty(), panel]));
    }

    private void Delete()
    {
        if (_workspace.Current is not { } project || _selectedId is not { } id) return;
        SelectedId = null;
        _workspace.Apply(Edits.SetGamePanels(project.GamePanels.OrEmpty().Where(panel => panel.Id != id)));
    }

    private void Save()
    {
        if (_workspace.Current is not { } project || Selected() is not { } panel) return;
        if (_title.Trim().Length == 0)
        {
            Message = "Give the panel a title.";
            return;
        }

        var updated = FromFields(panel);
        _workspace.Apply(Edits.SetGamePanels(project.GamePanels.OrEmpty().Select(existing => existing.Id == panel.Id ? updated : existing)));
        LoadSelected();
        Message = _workspace.SavedText("Panel");
    }

    private void OnProjectChanged(object? sender, ProjectChangedEventArgs e)
    {
        if (e.Kind == ProjectChangeKind.Opened)
        {
            Draft.Hide();
            _shown = null;
            SelectedId = null;
        }

        if (_isShown()) Refresh();
    }
}
