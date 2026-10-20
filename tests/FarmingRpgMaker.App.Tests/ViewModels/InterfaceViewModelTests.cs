using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using FarmingRpgMaker.App.ViewModels.EditMode;

namespace FarmingRpgMaker.App.Tests.ViewModels;

/// <summary>The Interface tab's panel draft (#87), driven without the editor.</summary>
public sealed class InterfaceViewModelTests
{
    private static InterfaceViewModel WithPanel(TestWorkspace test)
    {
        var viewModel = new InterfaceViewModel(test.Workspace);
        Assert.Equal("Select or add a panel.", viewModel.Message);
        Assert.False(viewModel.SaveCommand.CanExecute(null));
        viewModel.AddPanelCommand.Execute(null);
        return viewModel;
    }

    [Fact]
    public void ADraftSavesAsOneUndoStep()
    {
        using var test = new TestWorkspace();
        var viewModel = WithPanel(test);
        var panel = test.Project.GamePanels.OrEmpty().Last();
        Assert.Equal(panel.Id, viewModel.SelectedId);
        Assert.Equal($"Editing {panel.Id}", viewModel.Message);
        Assert.True(viewModel.SaveCommand.CanExecute(null));
        Assert.False(viewModel.HasUnsavedChanges);

        viewModel.Title = "  Farm Journal ";
        viewModel.VisibleFlag = " ";
        viewModel.AddEntryCommand.Execute(null);
        viewModel.AddEntryCommand.Execute(null);
        viewModel.Entries[0].Label = "Mood";
        viewModel.Entries[0].Value = "Calm";
        viewModel.Entries[1].Kind = GamePanelEntryKinds.Money;
        Assert.True(viewModel.HasUnsavedChanges);
        var before = test.Project;

        Assert.True(viewModel.SaveDraft());
        var saved = test.Project.GamePanels.OrEmpty().Single(p => p.Id == panel.Id);
        Assert.Equal("Farm Journal", saved.Title);
        Assert.Null(saved.VisibleFlag.OrNull());
        Assert.Equal([("Mood", "text", "Calm"), ("", "money", "")], saved.Entries.Select(entry => (entry.Label, entry.Kind, entry.Value)));
        Assert.False(viewModel.HasUnsavedChanges);
        Assert.Equal(test.Workspace.SavedText("Panel"), viewModel.Message);
        test.Workspace.Undo();
        Assert.Same(before, test.Project);
    }

    [Fact]
    public void ATitleIsRequired()
    {
        using var test = new TestWorkspace();
        var viewModel = WithPanel(test);
        viewModel.Title = " ";
        Assert.False(viewModel.SaveDraft());
        Assert.Equal("Give the panel a title.", viewModel.Message);
        Assert.True(viewModel.HasUnsavedChanges);

        // Save and continue that can't save stays on the panel and keeps asking.
        var id = viewModel.SelectedId;
        viewModel.SelectPanel(null);
        viewModel.Draft.SaveAndContinue();
        Assert.True(viewModel.Draft.IsAsking);
        Assert.Equal(id, viewModel.SelectedId);
    }

    [Fact]
    public void AnotherPanelAsksFirstAndKeepEditingStays()
    {
        using var test = new TestWorkspace();
        var viewModel = WithPanel(test);
        var first = viewModel.SelectedId;
        viewModel.AddPanelCommand.Execute(null);
        var second = viewModel.SelectedId;
        Assert.NotEqual(first, second);

        viewModel.Title = "Draft panel";
        // Undo elsewhere and a refresh keep the draft.
        viewModel.Refresh();
        Assert.Equal("Draft panel", viewModel.Title);

        viewModel.SelectPanel(first);
        Assert.True(viewModel.Draft.IsAsking);
        Assert.Contains("unsaved changes to the panel", viewModel.Draft.Text, StringComparison.Ordinal);
        Assert.Equal(second, viewModel.SelectedId);
        viewModel.Draft.Hide();
        Assert.Equal("Draft panel", viewModel.Title);

        viewModel.SelectPanel(first);
        viewModel.Draft.DiscardAndContinue();
        Assert.Equal(first, viewModel.SelectedId);
        Assert.DoesNotContain(test.Project.GamePanels.OrEmpty(), panel => panel.Title == "Draft panel");

        viewModel.Title = "Saved on the way";
        viewModel.SelectPanel(second);
        viewModel.Draft.SaveAndContinue();
        Assert.Equal(second, viewModel.SelectedId);
        Assert.Contains(test.Project.GamePanels.OrEmpty(), panel => panel.Id == first && panel.Title == "Saved on the way");
        Assert.False(viewModel.Draft.IsAsking);
    }

    [Fact]
    public void ANewKindClearsTheValueAndPickersKeepUnknownIds()
    {
        using var test = new TestWorkspace();
        var viewModel = WithPanel(test);
        viewModel.AddEntryCommand.Execute(null);
        var entry = viewModel.Entries[0];
        Assert.Equal(PanelValueEditor.Text, entry.Editor);
        entry.Value = "Hello";
        entry.Kind = GamePanelEntryKinds.Item;
        Assert.Equal("", entry.Value);
        Assert.Equal(PanelValueEditor.Picker, entry.Editor);
        Assert.Equal(PanelValueEditor.None, InterfaceViewModel.EditorFor(GamePanelEntryKinds.Day));
        Assert.Equal(ContentForms.Options("item", test.Project).Select(o => o.Id), viewModel.ValueOptions(GamePanelEntryKinds.Item, "").Select(o => o.Id));
        var missing = Assert.Single(viewModel.ValueOptions(GamePanelEntryKinds.Item, "ghost-item"), option => option.Missing);
        Assert.Equal("(missing: ghost-item)", missing.Label);

        Assert.Equal(new PanelEntryNames("Entry 2 label", "Entry 2 kind", "Entry 2 flag name", "Remove entry 2"), InterfaceViewModel.NamesFor(1, GamePanelEntryKinds.Flag));
        Assert.Equal("Entry 1 item", InterfaceViewModel.NamesFor(0, GamePanelEntryKinds.Item).Value);
        Assert.Equal("Choose action", InterfaceViewModel.Placeholder(GamePanelEntryKinds.Action));

        viewModel.RemoveEntry(entry);
        Assert.Empty(viewModel.Entries);
    }

    [Fact]
    public void APanelHoldsAtMostFortyEntriesAndCanBeRemoved()
    {
        using var test = new TestWorkspace();
        var viewModel = WithPanel(test);
        for (var i = 0; i < InterfaceViewModel.MaxEntries + 1; i++) viewModel.AddEntryCommand.Execute(null);
        Assert.Equal(InterfaceViewModel.MaxEntries, viewModel.Entries.Count);
        Assert.Equal("A panel may have at most 40 entries.", viewModel.Message);

        var id = viewModel.SelectedId;
        viewModel.RemovePanelCommand.Execute(null);
        Assert.Null(viewModel.SelectedId);
        Assert.DoesNotContain(test.Project.GamePanels.OrEmpty(), panel => panel.Id == id);
        Assert.Empty(viewModel.Entries);
        Assert.False(viewModel.RemovePanelCommand.CanExecute(null));
    }

    [Fact]
    public void OpeningAnotherProjectDropsTheSelection()
    {
        using var test = new TestWorkspace();
        var viewModel = WithPanel(test);
        viewModel.Title = "Unsaved";
        test.Workspace.Open(test.Workspace.CreateProject(ProjectTemplates.Blank, "Other"));
        Assert.Null(viewModel.SelectedId);
        Assert.False(viewModel.HasUnsavedChanges);
        Assert.Equal("Select or add a panel.", viewModel.Message);
    }
}
