using FarmEngine.Authoring;
using FarmEngine.Schemas;
using Microsoft.FSharp.Core;
using Tractr.App.Mvvm;
using Tractr.App.Projects;

namespace Tractr.App.ViewModels.EditMode;

/// <summary>
/// A dialogue script being edited (#225): an NPC's dialogues and lines, or the project's
/// cutscenes, as text. F# reads, checks and applies it; Apply is one undo step.
/// </summary>
public sealed class ScriptEditorViewModel : ObservableObject
{
    private readonly ProjectWorkspace _workspace;
    private string _text = "";
    private IReadOnlyList<string> _messages = [];

    /// <param name="workspace">The open project.</param>
    /// <param name="npcId">The NPC whose script this is; null for the project's cutscenes.</param>
    public ScriptEditorViewModel(ProjectWorkspace workspace, string? npcId)
    {
        _workspace = workspace;
        NpcId = npcId;
        Reload();
    }

    public string? NpcId { get; }

    /// <summary>The window's title.</summary>
    public string Title => NpcId is null ? "Cutscene script" : $"Dialogue script: {NpcId}";

    public string Text
    {
        get => _text;
        set => SetProperty(ref _text, value);
    }

    /// <summary>What the last check or apply found, by line ("Line 4: …").</summary>
    public IReadOnlyList<string> Messages
    {
        get => _messages;
        private set => SetProperty(ref _messages, value);
    }

    /// <summary>Puts the project's current dialogue back into the text.</summary>
    public void Reload()
    {
        if (_workspace.Current is not { } project) return;
        Text = NpcId is null ? AuthoringTools.CutsceneScript(project) : AuthoringTools.NpcScript(project, NpcId);
        Messages = NpcId is null ? [] : AuthoringTools.ScriptProblems(project, NpcId);
    }

    /// <summary>Checks the text without applying it; true when it has no mistakes.</summary>
    public bool Check()
    {
        if (_workspace.Current is not { } project) return false;
        var errors = AuthoringTools.CheckScript(project, Text);
        Messages = errors.Count == 0 ? ["The script reads without mistakes."] : errors;
        return errors.Count == 0;
    }

    /// <summary>Applies the text to the project (one undo step); false with its mistakes shown.</summary>
    public bool Apply()
    {
        if (_workspace.Current is not { } project) return false;
        var result = AuthoringTools.ApplyScript(project, Text);
        if (!result.Ok || result.Edit is not { } edit)
        {
            Messages = result.Errors;
            return false;
        }

        _workspace.Apply(edit);
        Reload();
        if (Messages.Count == 0) Messages = ["Applied."];
        return true;
    }
}

/// <summary>
/// A content table (#226, #227): rows of text cells for one category, inline edits, CSV export
/// and import with a diff, and bulk edits over the shown rows. Every change is one undo step.
/// </summary>
public sealed class TableEditorViewModel : ObservableObject
{
    private readonly ProjectWorkspace _workspace;
    private string _category = AuthoringTools.TableCategories[0];
    private string _filter = "";
    private string _tag = "";
    private IReadOnlyList<TableRow> _rows = [];
    private string _message = "";
    private TableDiff? _pendingImport;
    private BulkPreview? _pendingBulk;

    public TableEditorViewModel(ProjectWorkspace workspace)
    {
        _workspace = workspace;
        Refresh();
    }

    public static IReadOnlyList<string> Categories => AuthoringTools.TableCategories;

    public string Category
    {
        get => _category;
        set
        {
            if (SetProperty(ref _category, value))
            {
                ClearPending();
                OnPropertyChanged(nameof(Columns));
                Refresh();
            }
        }
    }

    public IReadOnlyList<TableColumn> Columns => AuthoringTools.TableColumns(_category);

    public string Filter
    {
        get => _filter;
        set
        {
            if (SetProperty(ref _filter, value)) Refresh();
        }
    }

    /// <summary>Only rows with this tag (empty: all).</summary>
    public string Tag
    {
        get => _tag;
        set
        {
            if (SetProperty(ref _tag, value)) Refresh();
        }
    }

    public IReadOnlyList<TableRow> Rows
    {
        get => _rows;
        private set => SetProperty(ref _rows, value);
    }

    public string Message
    {
        get => _message;
        private set => SetProperty(ref _message, value);
    }

    /// <summary>An import or bulk edit waiting for Apply.</summary>
    public bool HasPending => _pendingImport is not null || _pendingBulk is not null;

    public void Refresh()
    {
        if (_workspace.Current is not { } project) return;
        Rows = AuthoringTools.TableRows(project, _category, _filter, _tag.Length == 0 ? null : _tag);
    }

    /// <summary>Writes one cell; false (and the reason in <see cref="Message"/>) when it cannot be.</summary>
    public bool SetCell(string rowId, string columnKey, string text)
    {
        if (_workspace.Current is not { } project) return false;
        return Run(AuthoringTools.SetCell(project, _category, rowId, columnKey, text), "");
    }

    /// <summary>Pastes tab-separated rows at a cell, over the shown rows from that one on.</summary>
    public bool Paste(string rowId, string columnKey, string text)
    {
        if (_workspace.Current is not { } project) return false;
        var ids = Rows.Select(row => row.Id).SkipWhile(id => id != rowId).ToList();
        return Run(AuthoringTools.Paste(project, _category, ids, columnKey, text), "Pasted.");
    }

    public string ExportCsv() => _workspace.Current is { } project ? AuthoringTools.ExportCsv(project, _category) : "";

    /// <summary>Reads a CSV and describes what it would change; Apply makes the change.</summary>
    public void PreviewImport(string csv)
    {
        if (_workspace.Current is not { } project) return;
        ClearPending();
        var diff = AuthoringTools.ImportDiff(project, _category, csv);
        var lines = new List<string>();
        foreach (var invalid in diff.Invalid) lines.Add($"Row {invalid.Row}, {invalid.Column}: {invalid.Message}");
        foreach (var added in diff.Added) lines.Add($"New: {added.Id}");
        foreach (var changed in diff.Changed)
        {
            lines.Add($"Changed: {changed.Id}: " + string.Join("; ", changed.Changes.Select(c => $"{c.Column} {c.Before} → {c.After}")));
        }

        foreach (var removed in diff.Removed) lines.Add($"Not in the file (kept): {removed}");
        if (diff.Invalid.IsEmpty && diff.HasChanges) _pendingImport = diff;
        if (lines.Count == 0) lines.Add("The file changes nothing.");
        Message = string.Join("\n", lines);
        OnPropertyChanged(nameof(HasPending));
    }

    /// <summary>Shows what a bulk edit (<c>= 50</c>, <c>× 1.1</c>, <c>+ 1</c>, <c>+ summer</c>) of a column of the shown rows would do.</summary>
    public void PreviewBulk(string columnKey, string operation)
    {
        if (_workspace.Current is not { } project) return;
        ClearPending();
        var preview = AuthoringTools.BulkPreview(project, _category, Rows.Select(row => row.Id), columnKey, operation);
        if (!FSharpOption<BulkPreview>.get_IsSome(preview))
        {
            Message = "Write = VALUE, × FACTOR, + N, - N, + WORD or - WORD.";
            OnPropertyChanged(nameof(HasPending));
            return;
        }

        var value = preview!.Value;
        var lines = value.Invalid.Select(e => $"{e.Column}: {e.Message}")
            .Concat(value.Changes.Select(c => $"{c.Id}: " + string.Join("; ", c.Changes.Select(x => $"{x.Before} → {x.After}"))))
            .ToList();
        if (value.Invalid.IsEmpty && !value.Changes.IsEmpty) _pendingBulk = value;
        Message = lines.Count == 0 ? "The edit changes nothing." : string.Join("\n", lines);
        OnPropertyChanged(nameof(HasPending));
    }

    /// <summary>Adds a tag to the shown rows.</summary>
    public bool TagShown(string tag, bool remove)
    {
        if (_workspace.Current is not { } project || string.IsNullOrWhiteSpace(tag)) return false;
        return Run(AuthoringTools.Tag(project, _category, Rows.Select(row => row.Id), tag.Trim(), remove), remove ? "Tag removed." : "Tagged.");
    }

    /// <summary>Applies the previewed import or bulk edit (one undo step).</summary>
    public bool ApplyPending()
    {
        if (_workspace.Current is not { } project) return false;
        ToolResult? result = null;
        if (_pendingImport is { } diff) result = AuthoringTools.ApplyImport(project, diff, false);
        else if (_pendingBulk is { } bulk) result = AuthoringTools.BulkApply(project, _category, bulk);
        ClearPending();
        return result is not null && Run(result, "Applied.");
    }

    private void ClearPending()
    {
        _pendingImport = null;
        _pendingBulk = null;
        OnPropertyChanged(nameof(HasPending));
    }

    private bool Run(ToolResult result, string done)
    {
        if (!result.Ok || result.Edit is not { } edit)
        {
            Message = string.Join("\n", result.Errors);
            return false;
        }

        _workspace.Apply(edit);
        Message = done;
        Refresh();
        return true;
    }
}
