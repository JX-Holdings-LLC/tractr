using System.Globalization;
using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using Tractr.App.Game;
using Tractr.App.Mvvm;
using Tractr.App.Projects;

namespace Tractr.App.ViewModels.EditMode;

/// <summary>
/// Map authoring state and commands. Pointer/keyboard events supply tile coordinates; F#
/// performs edits, and this model owns the brush, gesture, clipboard, selection and form
/// validation independently of the canvas or its controls.
/// </summary>
public sealed class MapEditorViewModel(ProjectWorkspace workspace) : ObservableObject
{
    private string _message = "";
    public string Message { get => _message; set => SetProperty(ref _message, value); }
    public string? SceneId { get; set; }
    public string? Brush { get; set; }
    public VisualRef? BrushVisual { get; set; }
    public TileLayer Layer { get; set; } = Edits.LayerFor(TileTypes.Grass);
    public MapTool Tool { get; set; } = MapTool.Inspect;
    public EditorTab SelectedTab { get; set; } = EditorTab.Map;
    public string? StrokeId { get; set; }
    public int StrokeCount { get; set; }
    public (int X, int Y)? LastPainted { get; set; }
    public (int X, int Y) CursorTile { get; set; }
    public (int X, int Y)? GestureStart { get; set; }
    public (int X0, int Y0, int X1, int Y1)? Selection { get; set; }
    public List<List<Tile>>? CopiedTiles { get; set; }
    public (int X, int Y)? DoorAt { get; set; }
    public Dictionary<string, string> PlaceChoices { get; } = new(StringComparer.Ordinal);
    public string? PlaceKind { get; set; }
    public string? PlaceChoice { get; set; }
    public (Action<int, int> Done, string? ReturnScene, EditorTab ReturnTab)? PendingPick { get; set; }
    public string SceneName { get; set; } = "";
    public string SceneWidth { get; set; } = "";
    public string SceneHeight { get; set; } = "";
    public string? DoorDestinationId { get; set; }
    public string DoorX { get; set; } = "0";
    public string DoorY { get; set; } = "0";
    public bool ReturnDoor { get; set; }
    public GameProject? Project => workspace.Current;
    public Scene? Scene => Project?.Scenes.FirstOrDefault(scene => scene.Id == SceneId)
        ?? Project?.Scenes.FirstOrDefault(scene => scene.Id == Project.Player.SceneId)
        ?? Project?.Scenes.FirstOrDefault();

    public event Action<EditorTab, string?>? NavigationRequested;

    public void Navigate(EditorTab tab, string? sceneId = null)
    {
        SelectedTab = tab;
        if (sceneId is not null && Project?.Scenes.Any(scene => scene.Id == sceneId) == true) SceneId = sceneId;
        NavigationRequested?.Invoke(tab, sceneId);
    }

    public void BeginPick(string sceneId, string prompt, Action<int, int> done)
    {
        ArgumentNullException.ThrowIfNull(done);
        PendingPick = (done, SceneId, SelectedTab);
        Navigate(EditorTab.Map, sceneId);
        Message = $"{prompt} Escape cancels.";
    }

    public bool CompletePick((int X, int Y) tile)
    {
        if (PendingPick is not { } pick) return false;
        PendingPick = null;
        Navigate(EditorTab.Map, pick.ReturnScene);
        // Collect the tile before reopening the form so its unsaved draft is retained.
        pick.Done(tile.X, tile.Y);
        Navigate(pick.ReturnTab);
        return true;
    }

    public void CancelPick()
    {
        if (PendingPick is not { } pick) return;
        PendingPick = null;
        Navigate(pick.ReturnTab, pick.ReturnScene);
        Message = "Pick cancelled.";
    }

    public bool Apply(Edit edit) => workspace.Apply(edit);
    public bool ApplyInStroke(string strokeId, Edit edit) => workspace.ApplyInStroke(strokeId, edit);
    public bool ApplyWithoutHistory(Edit edit) => workspace.ApplyWithoutHistory(edit);
    public bool Undo() => workspace.Undo();
    public bool Redo() => workspace.Redo();

    public void SelectBrush(string? brush, VisualRef? visual)
    {
        Brush = brush;
        BrushVisual = brush is null ? null : visual;
        if (brush is not null)
        {
            ApplyWithoutHistory(Edits.SelectBrush(brush, visual));
            Layer = Project is { } project ? Edits.LayerIn(project, brush) : Edits.LayerFor(brush);
        }
        Tool = brush is null ? MapTool.Inspect : MapTool.Brush;
    }

    public void PaintTile(int x, int y)
    {
        if (Brush is not { } brush || Scene is not { } scene || y < 0 || y >= scene.Tiles.Length || x < 0 || x >= scene.Tiles[y].Length) return;
        var edit = Edits.PaintTiles(scene.Id, Layer, new[] { (x, y) }, brush);
        if (StrokeId is { } strokeId) ApplyInStroke(strokeId, edit);
        else Apply(edit);
    }

    public void ApplyStrokeTile(int x, int y)
    {
        if (Scene is not { } scene) return;
        switch (Tool)
        {
            case MapTool.Brush: PaintTile(x, y); break;
            case MapTool.Erase: ApplyInStroke(StrokeId!, Edits.EraseLayer(scene.Id, Layer, new[] { (x, y) })); break;
            case MapTool.Block: ApplyInStroke(StrokeId!, Edits.SetCollision(scene.Id, new[] { (x, y) }, true)); break;
            case MapTool.Unblock: ApplyInStroke(StrokeId!, Edits.SetCollision(scene.Id, new[] { (x, y) }, false)); break;
        }
    }

    public void CompleteGesture((int X, int Y) end)
    {
        if (GestureStart is not { } start) return;
        GestureStart = null;
        if (Scene is not { } scene) return;
        var area = (X0: Math.Min(start.X, end.X), Y0: Math.Min(start.Y, end.Y), X1: Math.Max(start.X, end.X), Y1: Math.Max(start.Y, end.Y));
        if (Tool == MapTool.Rectangle && Brush is not null) Apply(Edits.FillRect(scene.Id, Layer, area.X0, area.Y0, area.X1, area.Y1, Brush));
        else if (Tool == MapTool.Select)
        {
            Selection = area;
            Message = $"Selected {area.X1 - area.X0 + 1} × {area.Y1 - area.Y0 + 1} tiles.";
        }
    }

    public void ClearSelection()
    {
        Selection = null;
        GestureStart = null;
    }

    public void CopySelection()
    {
        if (Scene is not { } scene || Selection is not { } area) return;
        if (area.Y0 < 0 || area.X0 < 0 || area.Y1 >= scene.Tiles.Length || scene.Tiles.Skip(area.Y0).Take(area.Y1 - area.Y0 + 1).Any(row => area.X1 >= row.Length))
        {
            ClearSelection();
            Message = "The selection no longer fits this scene; select an area again.";
            return;
        }
        CopiedTiles = [];
        for (var y = area.Y0; y <= area.Y1; y++) CopiedTiles.Add(scene.Tiles[y].Skip(area.X0).Take(area.X1 - area.X0 + 1).ToList());
        Message = $"Copied {CopiedTiles[0].Count} × {CopiedTiles.Count} tiles. Choose Paste on map.";
    }

    public void PasteAt(int x, int y)
    {
        if (Scene is { } scene && CopiedTiles is not null) Apply(Edits.PasteTiles(scene.Id, x, y, CopiedTiles));
    }

    public bool PickAt(int x, int y)
    {
        if (Project is not { } project || Scene is not { } scene || Edits.PickBrush(project, scene.Id, x, y) is not { } pick) return false;
        var picked = Documents.Preview(project, pick);
        SelectBrush(picked.SelectedTileType, picked.SelectedTileVisual.OrNull());
        Message = $"Picked {picked.SelectedTileType}.";
        return true;
    }

    public void PlaceAt(int x, int y, string? id, string? label)
    {
        if (Project is not { } project || Scene is not { } scene || PlacementKind(Tool) is not { } kind) return;
        if (id is null || MapPlacement.Place(project, kind, id, scene.Id, x, y) is not { } edit)
        {
            Message = "Choose what to place first.";
            return;
        }
        if (Apply(edit)) Message = $"Placed {label} at ({x}, {y}).";
    }

    public static string? PlacementKind(MapTool tool) => tool switch
    {
        MapTool.PlaceNpc => "npc",
        MapTool.PlaceNode => "nodeType",
        MapTool.PlaceItem => "item",
        MapTool.PlaceMachine => "machineType",
        MapTool.PlaceAnimal => "animalSpecies",
        _ => null,
    };

    public bool TypedSceneSize(out int width, out int height)
    {
        var validWidth = int.TryParse(SceneWidth, NumberStyles.None, CultureInfo.InvariantCulture, out width);
        var validHeight = int.TryParse(SceneHeight, NumberStyles.None, CultureInfo.InvariantCulture, out height);
        return validWidth && validHeight && width >= 1 && width <= Edits.MaxSceneSize && height >= 1 && height <= Edits.MaxSceneSize;
    }

    public string SceneSizeInfo()
    {
        if (!TypedSceneSize(out var width, out var height)) return $"Width and height must be whole numbers from 1 to {Edits.MaxSceneSize}.";
        var info = $"Total tiles: {width * height} · Aspect ratio: {((double)width / height).ToString("0.00", CultureInfo.InvariantCulture)}";
        if (Scene is { } scene && Project is { } project && (width < scene.Width || height < scene.Height))
        {
            info += $"\nResize removes the tiles outside {width}×{height}.";
            var impact = Edits.ResizeImpact(project, scene.Id, width, height);
            if (impact.Count > 0) info += " It also changes: " + string.Join("; ", impact) + ".";
        }
        return info;
    }

    private bool ValidSceneSize(out int width, out int height)
    {
        var valid = TypedSceneSize(out width, out height);
        if (!valid) Message = $"Scene width and height must be whole numbers from 1 to {Edits.MaxSceneSize}.";
        return valid;
    }

    public string? AddScene(string tile)
    {
        if (Project is not { } project || !ValidSceneSize(out var width, out var height)) return null;
        var scene = Defaults.NewScene(project, SceneName, width, height, tile);
        return Apply(Edits.AddScene(scene)) ? scene.Id : null;
    }

    public void RenameScene()
    {
        if (Scene is not { } scene) return;
        if (string.IsNullOrWhiteSpace(SceneName)) Message = "Enter a scene name first.";
        else Apply(Edits.RenameScene(scene.Id, SceneName.Trim()));
    }

    public bool ResizeScene()
    {
        if (Scene is not { } scene || Project is not { } project || !ValidSceneSize(out var width, out var height)) return false;
        var impact = Edits.ResizeImpact(project, scene.Id, width, height);
        var changed = Apply(Edits.ResizeScene(scene.Id, width, height));
        if (changed) Message = impact.Count == 0
            ? $"Resized {scene.Name} to {width}×{height}."
            : $"Resized {scene.Name} to {width}×{height}: {string.Join("; ", impact)}. Undo puts them back.";
        ClearSelection();
        return changed;
    }

    public string? DuplicateScene()
    {
        if (Project is not { } project || Scene is not { } scene) return null;
        var id = Defaults.NextId("scene", project.Scenes.Select(s => s.Id));
        return Apply(Edits.DuplicateScene(scene.Id, id)) ? id : null;
    }

    public void DeleteScene()
    {
        if (Scene is { } scene && !Apply(Edits.RemoveScene(scene.Id))) Message = "The start scene and the last scene cannot be deleted.";
    }

    public void RemoveDoor()
    {
        if (Scene is { } scene && DoorAt is { } from) Apply(Edits.RemoveTransition(scene.Id, from.X, from.Y));
    }

    public void SaveDoor()
    {
        if (Scene is not { } scene || Project is not { } project || DoorAt is not { } from)
        {
            Message = "Choose Door and click a departure tile first.";
            return;
        }
        if (DoorDestinationId is not { } destinationId
            || !int.TryParse(DoorX, NumberStyles.None, CultureInfo.InvariantCulture, out var x)
            || !int.TryParse(DoorY, NumberStyles.None, CultureInfo.InvariantCulture, out var y)
            || project.Scenes.FirstOrDefault(s => s.Id == destinationId) is not { } destination
            || x < 0 || y < 0 || x >= destination.Width || y >= destination.Height)
        {
            Message = "Choose a destination and coordinates inside that scene.";
            return;
        }
        var old = scene.Transitions.FirstOrDefault(t => t.FromX == from.X && t.FromY == from.Y);
        var transition = (old ?? SceneTransition.Default).WithFromX(from.X).WithFromY(from.Y).WithToSceneId(destinationId).WithToX(x).WithToY(y);
        Apply(ReturnDoor ? Edits.LinkScenes(scene.Id, transition) : Edits.SetTransition(scene.Id, transition));
        Message = workspace.SavedText("Door", $" to {destination.Name} ({x}, {y})");
    }

    public static (int X, int Y) FreeDoorTile(Scene scene, int x, int y)
    {
        var width = Math.Max(1, (int)scene.Width);
        var count = width * Math.Max(1, (int)scene.Height);
        var start = (Math.Clamp(y, 0, (count / width) - 1) * width) + Math.Clamp(x, 0, width - 1);
        for (var i = 0; i < count; i++)
        {
            var index = (start + i) % count;
            var tile = (X: index % width, Y: index / width);
            if (!scene.Transitions.Any(t => t.FromX == tile.X && t.FromY == tile.Y)) return tile;
        }
        return (start % width, start / width);
    }
}
