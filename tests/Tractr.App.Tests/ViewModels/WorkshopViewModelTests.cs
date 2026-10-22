using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using Tractr.App.ViewModels.EditMode;

namespace Tractr.App.Tests.ViewModels;

/// <summary>The Workshop tab's pattern form, driven without the editor.</summary>
public sealed class WorkshopViewModelTests
{
    private sealed class Calls
    {
        public List<string> Opened { get; } = [];

        public List<(string Scene, string Prompt, Action<int, int> Done)> Picks { get; } = [];
    }

    private static WorkshopViewModel Create(TestWorkspace test, Calls calls, string? mapScene = null) =>
        new(test.Workspace, () => mapScene, (scene, prompt, done) => calls.Picks.Add((scene, prompt, done)), calls.Opened.Add);

    [Fact]
    public void CreatesAPatternAsOneUndoStep()
    {
        using var test = new TestWorkspace();
        var viewModel = Create(test, new Calls());
        Assert.Equal(Patterns.All[0], viewModel.SelectedPattern);
        Assert.Equal(Patterns.All[0].Help, viewModel.Help);
        var before = test.Project.Npcs.Length;

        viewModel.Name = "Luna";
        viewModel.X = "4";
        viewModel.Y = "4";
        viewModel.CreateCommand.Execute(null);
        Assert.Equal(before + 1, test.Project.Npcs.Length);
        Assert.Contains(test.Project.Npcs, npc => npc.Name == "Luna");
        Assert.Equal(Patterns.SuccessMessage(Patterns.All[0].Kind, "Luna"), viewModel.Message);
        Assert.Contains(viewModel.Characters, character => character.Name == "Luna");

        test.Workspace.Undo();
        Assert.Equal(before, test.Project.Npcs.Length);
    }

    [Fact]
    public void NumbersMustBeWholeAndTheErrorNamesTheField()
    {
        using var test = new TestWorkspace();
        var viewModel = Create(test, new Calls());
        var before = test.Project;
        viewModel.Y = "-1";
        viewModel.CreateCommand.Execute(null);
        Assert.Equal("Scene tile Y must be a whole number of 0 or more.", viewModel.Message);
        viewModel.Y = "1";
        viewModel.Friendship = "lots";
        viewModel.CreateCommand.Execute(null);
        Assert.Equal("Friendship must be a whole number of 0 or more.", viewModel.Message);
        Assert.Same(before, test.Project);

        // Another pattern clears the message.
        viewModel.SelectedPattern = Patterns.All[1];
        Assert.Equal("", viewModel.Message);
        Assert.Equal(Patterns.All[1].Help, viewModel.Help);
    }

    [Fact]
    public void PickOnMapFillsTheTileInTheMapTabsScene()
    {
        using var test = new TestWorkspace();
        var calls = new Calls();
        var scene = test.Project.Scenes[^1];
        var viewModel = Create(test, calls, scene.Id);
        Assert.Equal($"In the scene the Map tab shows: {scene.Name}.", viewModel.SceneText);

        viewModel.PickTileCommand.Execute(null);
        var (pickedScene, prompt, done) = Assert.Single(calls.Picks);
        Assert.Equal(scene.Id, pickedScene);
        Assert.Equal("Click the tile for the pattern.", prompt);
        done(7, 3);
        Assert.Equal(("7", "3"), (viewModel.X, viewModel.Y));
        Assert.Equal("Tile (7, 3) chosen.", viewModel.Message);

        // A scene that is gone falls back to the player's.
        var fallback = Create(test, calls, "gone");
        Assert.Equal(test.Project.Player.SceneId, fallback.SceneId());
    }

    [Fact]
    public void ARemovedCharacterIsUnchosenAndLinksOpenTheirEditor()
    {
        using var test = new TestWorkspace();
        var calls = new Calls();
        var viewModel = Create(test, calls);
        Assert.Equal(WorkshopViewModel.NoCharacter, viewModel.Characters[0]);
        var npc = test.Project.Npcs[0];
        viewModel.CharacterId = npc.Id;
        test.Workspace.Apply(Edits.RemoveNpc(npc.Id));
        Assert.Equal("", viewModel.CharacterId);
        Assert.DoesNotContain(viewModel.Characters, character => character.Id == npc.Id);

        viewModel.OpenEditorCommand.Execute(null);
        viewModel.OpenLink("craft");
        Assert.Equal([Patterns.All[0].Tab, "craft"], calls.Opened);
        Assert.Equal(11, WorkshopViewModel.QuickLinks.Count);
    }
}
