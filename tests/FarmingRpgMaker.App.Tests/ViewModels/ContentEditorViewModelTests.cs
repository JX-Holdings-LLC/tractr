using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using FarmingRpgMaker.App.ViewModels.EditMode;

namespace FarmingRpgMaker.App.Tests.ViewModels;

/// <summary>The Content tab's list, selection and draft rules (#87), driven without the editor or its generated form.</summary>
public sealed class ContentEditorViewModelTests
{
    /// <summary>Stands in for the generated form: holds the entry, an edit typed into it, or an invalid field.</summary>
    private sealed class FakeDraft(object entity) : IContentDraft
    {
        private readonly object _built = entity;

        public object Entity { get; set; } = entity;

        public string? Invalid { get; set; }

        public bool IsDirty => Invalid is not null || !Equals(Entity, _built);

        public object Commit() => Invalid is { } error ? throw new FormatException(error) : Entity;

        public object? Preview() => Invalid is null ? Entity : null;
    }

    /// <summary>A view model whose forms are fake drafts, as the view would build them.</summary>
    private static (ContentEditorViewModel ViewModel, Func<FakeDraft?> Draft) Create(TestWorkspace test)
    {
        FakeDraft? draft = null;
        var viewModel = new ContentEditorViewModel(test.Workspace);
        viewModel.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName != nameof(ContentEditorViewModel.FormEntity)) return;
            draft = viewModel.FormEntity is { } entity ? new FakeDraft(entity) : null;
            if (draft is not null) viewModel.AttachDraft(draft);
        };
        return (viewModel, () => draft);
    }

    [Fact]
    public void AnEntrySavesAsOneUndoStep()
    {
        using var test = new TestWorkspace();
        var (viewModel, draft) = Create(test);
        Assert.Equal("NPCs", viewModel.SelectedCategory.Name);
        Assert.Equal(test.Project.Npcs.Select(npc => npc.Id), viewModel.Rows.Select(row => row.Id));
        Assert.All(viewModel.Rows, row => Assert.True(row.HasArtSlot));
        Assert.Equal("Select an entry to edit it.", viewModel.Message);
        Assert.False(viewModel.SaveCommand.CanExecute(null));

        var npc = test.Project.Npcs[0];
        viewModel.SelectRow(npc.Id);
        Assert.Equal(npc, viewModel.FormEntity);
        Assert.Equal($"Editing npcs · {npc.Id}", viewModel.Message);
        Assert.True(viewModel.SaveCommand.CanExecute(null));
        Assert.False(viewModel.ShowDuplicate);
        Assert.False(viewModel.HasUnsavedChanges);

        draft()!.Entity = npc.WithName("Draft Name");
        Assert.True(viewModel.HasUnsavedChanges);
        var before = test.Project;
        viewModel.SaveCommand.Execute(null);
        Assert.Equal("Draft Name", test.Project.Npcs[0].Name);
        Assert.False(viewModel.HasUnsavedChanges);
        test.Workspace.Undo();
        Assert.Same(before, test.Project);

        // Nothing typed: nothing to undo.
        viewModel.SaveCommand.Execute(null);
        Assert.Equal("No changes were made.", viewModel.Message);
    }

    [Fact]
    public void AnotherEntryAsksFirstAndAnInvalidFieldIsNotSaved()
    {
        using var test = new TestWorkspace();
        var (viewModel, draft) = Create(test);
        var npc = test.Project.Npcs[0];
        var other = test.Project.Npcs[1];
        viewModel.SelectEntry("NPCs", npc.Id);
        draft()!.Invalid = "Heart level must be a number.";
        Assert.True(viewModel.HasUnsavedChanges);

        // Undo elsewhere keeps the draft; the entry list catches up.
        test.Workspace.Apply(Edits.UpsertNpc(other.WithName("Elsewhere")));
        Assert.Equal(npc.Id, viewModel.SelectedId);
        Assert.True(viewModel.HasUnsavedChanges);

        viewModel.SelectRow(other.Id);
        Assert.True(viewModel.Draft.IsAsking);
        Assert.Contains(ContentEditorViewModel.LabelOf(npc), viewModel.Draft.Text, StringComparison.Ordinal);
        viewModel.Draft.SaveAndContinue();
        Assert.Equal("Could not save: Heart level must be a number.", viewModel.Message);
        Assert.True(viewModel.Draft.IsAsking);
        Assert.Equal(npc.Id, viewModel.SelectedId);

        viewModel.Draft.DiscardAndContinue();
        Assert.Equal(other.Id, viewModel.SelectedId);
        Assert.False(viewModel.HasUnsavedChanges);
    }

    [Fact]
    public void AnEntryChangedElsewhereKeepsTheDraftAndSaysSo()
    {
        using var test = new TestWorkspace();
        var (viewModel, draft) = Create(test);
        var npc = test.Project.Npcs[0];
        viewModel.SelectRow(npc.Id);
        draft()!.Entity = npc.WithName("Mine");
        test.Workspace.Apply(Edits.UpsertNpc(npc.WithName("Theirs")));
        Assert.StartsWith("Theirs  ·  ", viewModel.Message, StringComparison.Ordinal);
        Assert.Contains("changed elsewhere while you edited it", viewModel.Message, StringComparison.Ordinal);
        Assert.True(viewModel.SaveDraft());
        Assert.Equal("Mine", test.Project.Npcs[0].Name);
    }

    [Fact]
    public void BuiltinsAreCustomizedAndComeBackWhenDeleted()
    {
        using var test = new TestWorkspace(ProjectTemplates.Blank);
        var (viewModel, _) = Create(test);
        viewModel.SelectCategory("Crops");
        var builtin = viewModel.Rows.First(row => ContentReadouts.IsBuiltinCrop(row.Id));
        viewModel.SelectRow(builtin.Id);
        Assert.Null(viewModel.FormEntity);
        Assert.True(viewModel.ShowCustomize);
        Assert.Equal("This is a built-in crop. Customize it to change it.", viewModel.Message);
        Assert.Equal("CropSummary", viewModel.Summary?.Name);
        Assert.Null(viewModel.ProfitName);

        viewModel.CustomizeCommand.Execute(null);
        Assert.StartsWith("Customized ", viewModel.Message, StringComparison.Ordinal);
        Assert.NotNull(viewModel.FormEntity);
        Assert.Equal("CropProfit", viewModel.ProfitName);
        Assert.NotEqual("", viewModel.ProfitText);
        Assert.Equal("Remove your changes and use the built-in crop again", viewModel.DeleteTip);

        viewModel.DeleteCommand.Execute(null);
        Assert.Equal($"Removed your changes: {builtin.Id} is the built-in crop again.", viewModel.Message);
        Assert.Equal(builtin.Id, viewModel.SelectedId);
        Assert.Null(viewModel.FormEntity);
    }

    [Fact]
    public void NewDuplicateFilterAndInventory()
    {
        using var test = new TestWorkspace();
        var (viewModel, _) = Create(test);
        viewModel.SelectCategory("Items");
        Assert.True(viewModel.ShowAddToInventory);
        var count = test.Project.Items.Length;
        viewModel.AddCommand.Execute(null);
        Assert.Equal(count + 1, test.Project.Items.Length);
        var created = test.Project.Items[^1];
        Assert.Equal(created.Id, viewModel.SelectedId);
        viewModel.AddToInventoryCommand.Execute(null);
        Assert.Contains(test.Project.Player.Inventory, slot => slot.Item.Id == created.Id);

        viewModel.Filter = "no entry is called this";
        Assert.Equal([created.Id], viewModel.Rows.Select(row => row.Id));
        // The selected entry always shows; once none is, the filter hides it too.
        viewModel.SelectRow(null);
        viewModel.Filter = "no entry is called that either";
        Assert.Empty(viewModel.Rows);
        viewModel.Filter = "";

        viewModel.SelectCategory("Actions");
        Assert.True(viewModel.ShowDuplicate);
        var action = test.Project.Actions[0];
        var actions = test.Project.Actions.Length;
        viewModel.SelectRow(action.Id);
        viewModel.DuplicateCommand.Execute(null);
        Assert.StartsWith("Duplicated as ", viewModel.Message, StringComparison.Ordinal);
        Assert.NotEqual(action.Id, viewModel.SelectedId);
        viewModel.DeleteCommand.Execute(null);
        Assert.Null(viewModel.SelectedId);
        Assert.Equal(actions, test.Project.Actions.Length);
    }

    [Fact]
    public void SearchRanksNamesThatStartWithTheQueryFirst()
    {
        using var test = new TestWorkspace();
        var npc = test.Project.Npcs[0];
        var results = ContentEditorViewModel.Search(test.Project, npc.Name);
        Assert.Equal(("NPCs", npc.Id), (results[0].Category, results[0].Id));
        Assert.Empty(ContentEditorViewModel.Search(test.Project, "  "));
        Assert.Equal(0, ContentEditorViewModel.Rank("Wheat seeds", "wheat"));
        Assert.Equal(1, ContentEditorViewModel.Rank("Wheat seeds", "seed"));
        Assert.Null(ContentEditorViewModel.Rank("Wheat seeds", "corn"));
    }
}
