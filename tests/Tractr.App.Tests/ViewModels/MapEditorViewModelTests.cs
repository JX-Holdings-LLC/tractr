using FarmEngine.Authoring;
using FarmEngine.Schemas;
using Tractr.App.Game;
using Tractr.App.ViewModels.EditMode;

namespace Tractr.App.Tests.ViewModels;

public sealed class MapEditorViewModelTests
{
    [Fact]
    public void PickingReturnsToTheOriginalSceneAndUpdatesTheDraftBeforeShowingItsTab()
    {
        using var test = new TestWorkspace();
        var model = new MapEditorViewModel(test.Workspace) { SceneId = test.Project.Player.SceneId, SelectedTab = EditorTab.Content };
        var originalScene = model.SceneId;
        var destination = test.Project.Scenes.Last().Id;
        var delivered = false;
        model.NavigationRequested += (tab, _) =>
        {
            if (tab == EditorTab.Content) Assert.True(delivered);
        };
        model.BeginPick(destination, "Pick a tile.", (x, y) => delivered = (x, y) == (3, 4));
        Assert.Equal(EditorTab.Map, model.SelectedTab);
        Assert.Equal(destination, model.SceneId);
        Assert.True(model.CompletePick((3, 4)));
        Assert.Equal(EditorTab.Content, model.SelectedTab);
        Assert.Equal(originalScene, model.SceneId);
        Assert.Null(model.PendingPick);
        Assert.False(model.CompletePick((0, 0)));
    }

    [Fact]
    public void BrushStrokeAndClipboardWorkWithoutAView()
    {
        using var test = new TestWorkspace();
        var model = new MapEditorViewModel(test.Workspace) { SceneId = test.Project.Player.SceneId };
        model.SelectBrush("water", null);
        Assert.Equal(MapTool.Brush, model.Tool);
        model.StrokeId = "stroke";
        model.PaintTile(2, 2);
        model.PaintTile(3, 2);
        Assert.Equal("water", model.Scene!.Tiles[2][2].Background);
        Assert.Equal("water", model.Scene.Tiles[2][3].Background);
        model.Undo();
        Assert.NotEqual("water", model.Scene.Tiles[2][2].Background);
        model.Redo();
        model.Tool = MapTool.Select;
        model.GestureStart = (3, 2);
        model.CompleteGesture((2, 2));
        Assert.Equal((2, 2, 3, 2), model.Selection);
        model.CopySelection();
        model.PasteAt(4, 3);
        Assert.Equal("water", model.Scene.Tiles[3][4].Background);
        Assert.Equal("water", model.Scene.Tiles[3][5].Background);
    }

    [Fact]
    public void SceneAndDoorDraftsValidateBeforeChangingTheProject()
    {
        using var test = new TestWorkspace();
        var model = new MapEditorViewModel(test.Workspace) { SceneId = test.Project.Player.SceneId, SceneWidth = "invalid", SceneHeight = "4" };
        var before = test.Project;
        Assert.Null(model.AddScene(TileTypes.Grass));
        Assert.Same(before, test.Project);
        Assert.Contains("whole numbers", model.Message, StringComparison.Ordinal);
        model.SceneWidth = "4";
        model.SceneName = "Tiny room";
        var room = model.AddScene(TileTypes.Grass);
        Assert.NotNull(room);
        model.DoorAt = (2, 2);
        model.DoorDestinationId = room;
        model.DoorX = "99";
        model.DoorY = "1";
        model.SaveDoor();
        Assert.Contains("inside that scene", model.Message, StringComparison.Ordinal);
        model.DoorX = "1";
        model.ReturnDoor = true;
        model.SaveDoor();
        Assert.Contains(model.Scene!.Transitions, door => door.FromX == 2 && door.FromY == 2 && door.ToSceneId == room);
        Assert.Contains(test.Project.Scenes.Single(scene => scene.Id == room).Transitions, door => door.ToSceneId == model.SceneId);
        model.Undo();
        Assert.DoesNotContain(model.Scene.Transitions, door => door.ToSceneId == room);
    }
}
