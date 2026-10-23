using System.Text.Json.Nodes;
using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using Tractr.App.ViewModels.EditMode;

namespace Tractr.App.Tests.ViewModels;

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
    public void BuiltinTerrainsAreListedAndCustomizedLikeNodeTypes()
    {
        using var test = new TestWorkspace();
        var (viewModel, draft) = Create(test);
        viewModel.SelectCategory("Terrains");
        var grass = viewModel.Rows.Single(row => row.Id == TileTypes.Grass);
        Assert.Equal("Built-in · background", grass.Note);
        Assert.True(grass.HasArtSlot);
        viewModel.SelectRow(TileTypes.Grass);
        Assert.Null(viewModel.FormEntity);
        Assert.True(viewModel.ShowCustomize);
        Assert.Equal("This is a built-in terrain. Customize it to change it.", viewModel.Message);

        viewModel.CustomizeCommand.Execute(null);
        Assert.Contains(test.Project.Terrains.OrEmpty(), terrain => terrain.Id == TileTypes.Grass);
        Assert.StartsWith("Replaces built-in", viewModel.Rows.First().Note, StringComparison.Ordinal);
        Assert.Equal("Remove your changes and use the built-in terrain again", viewModel.DeleteTip);
        var renamed = (JsonObject)RecordJson.ToNode(viewModel.FormEntity!, typeof(TerrainDefinition))!;
        renamed["name"] = "Meadow";
        draft()!.Entity = RecordJson.FromNode<TerrainDefinition>(renamed);
        viewModel.SaveCommand.Execute(null);
        Assert.Equal("Meadow", test.Project.Terrains.OrEmpty().Single().Name);

        viewModel.DeleteCommand.Execute(null);
        Assert.Equal($"Removed your changes: {TileTypes.Grass} is the built-in terrain again.", viewModel.Message);
        Assert.Empty(test.Project.Terrains.OrEmpty());

        viewModel.AddCommand.Execute(null);
        var created = Assert.Single(test.Project.Terrains.OrEmpty());
        Assert.Equal(created.Id, viewModel.SelectedId);
        Assert.StartsWith("Custom · background", viewModel.Rows.First(row => row.Id == created.Id).Note, StringComparison.Ordinal);
    }

    [Theory]
    [InlineData("Tools", "tool")]
    [InlineData("Item kinds", "item kind")]
    [InlineData("Crop qualities", "crop quality")]
    [InlineData("Crop mutations", "crop mutation")]
    public void VocabularyCategoriesListBuiltinsAndAddTheirOwn(string category, string kind)
    {
        using var test = new TestWorkspace();
        var (viewModel, _) = Create(test);
        viewModel.SelectCategory(category);
        Assert.NotEmpty(viewModel.Rows);
        Assert.All(viewModel.Rows, row => Assert.StartsWith("Built-in", row.Note, StringComparison.Ordinal));
        viewModel.SelectRow(viewModel.Rows[0].Id);
        Assert.Equal($"This is a built-in {kind}. Customize it to change it.", viewModel.Message);

        var builtins = viewModel.Rows.Count;
        viewModel.AddCommand.Execute(null);
        Assert.Equal(builtins + 1, viewModel.Rows.Count);
        Assert.NotNull(viewModel.FormEntity);
        Assert.StartsWith("Custom", viewModel.Rows[0].Note, StringComparison.Ordinal);
        Assert.Equal(viewModel.Rows[0].Id, viewModel.SelectedId);
        viewModel.DeleteCommand.Execute(null);
        Assert.Equal(builtins, viewModel.Rows.Count);
    }

    [Fact]
    public void CutscenesAreAddedAndDuplicated()
    {
        using var test = new TestWorkspace();
        var (viewModel, _) = Create(test);
        viewModel.SelectCategory("Cutscenes");
        Assert.Empty(viewModel.Rows);
        Assert.True(viewModel.ShowDuplicate);
        viewModel.AddCommand.Execute(null);
        var cutscene = Assert.Single(test.Project.Cutscenes.OrEmpty());
        Assert.Equal(cutscene, viewModel.FormEntity);
        Assert.Equal(["fade", "say", "fade"], cutscene.Steps.Select(step => step.Type));
        viewModel.DuplicateCommand.Execute(null);
        Assert.StartsWith("Duplicated as ", viewModel.Message, StringComparison.Ordinal);
        Assert.Equal(2, test.Project.Cutscenes.OrEmpty().Length);
        Assert.Equal(test.Project.Cutscenes.OrEmpty()[1].Id, viewModel.SelectedId);
        Assert.Contains(ContentEditorViewModel.Search(test.Project, cutscene.Name), result => result.Category == "Cutscenes" && result.Id == cutscene.Id);
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
