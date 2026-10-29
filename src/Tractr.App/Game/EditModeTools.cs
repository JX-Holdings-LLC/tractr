using System.Globalization;
using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Media;
using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using Tractr.App.Localization;
using Tractr.App.ViewModels.EditMode;

namespace Tractr.App.Game;

public enum MapTool
{
    Inspect, Brush, Rectangle, Fill, Pick, Select, Paste, Erase, Block, Unblock, Door, PlayerStart,
    PlaceNpc, PlaceNode, PlaceItem, PlaceMachine, PlaceAnimal, Remove,
}

/// <summary>The creator-facing map tools. All mutations go through the F# document.</summary>
public sealed partial class EditModeView
{
    private readonly WrapPanel _tools = new() { Name = "MapTools" };
    private readonly WrapPanel _placeTools = new() { Name = "PlaceTools" };
    private readonly ComboBox _placeChoice = new() { Name = "PlaceChoice", MinWidth = 200, IsEnabled = false, PlaceholderText = "Choose a Place tool" };
    private readonly StackPanel _sceneAnimals = new() { Name = "SceneAnimals", Spacing = 6 };
    private readonly ComboBox _newSceneTile = new() { Name = "NewSceneTile", MinWidth = 110 };
    private readonly ComboBox _layerSelector = new() { Name = "LayerSelector", MinWidth = 140 };
    private readonly TextBox _sceneName = new() { Name = "SceneName", PlaceholderText = "Scene name" };
    private readonly TextBox _sceneWidth = new() { Name = "SceneWidth", Width = 58, PlaceholderText = "Width" };
    private readonly TextBox _sceneHeight = new() { Name = "SceneHeight", Width = 58, PlaceholderText = "Height" };
    private readonly Button _deleteScene = new() { Name = "DeleteSceneButton", Content = "Delete scene" };
    private readonly Button _startScene = new() { Name = "SetStartSceneButton", Content = "Set as start" };
    private readonly Button _copy = new() { Name = "CopySelectionButton", Content = "Copy selection" };
    private readonly Button _paste = new() { Name = "PasteToolButton", Content = "Paste on map" };
    private readonly Button _fillScene = new() { Name = "FillSceneButton", Content = "Fill scene" };
    private readonly ComboBox _doorDestination = new() { Name = "DoorDestination", MinWidth = 140 };
    private readonly TextBox _doorX = new() { Name = "DoorX", Width = 58, Text = "0" };
    private readonly TextBox _doorY = new() { Name = "DoorY", Width = 58, Text = "0" };
    private readonly CheckBox _doorReturn = new() { Name = "DoorReturn", Content = "Return door" };
    private readonly CheckBox _sceneIndoor = new() { Name = "SceneIndoor", Content = "Indoor (no weather)" };
    private bool _showingSceneIndoor;
    private readonly CheckBox _sceneDark = new() { Name = "SceneDark", Content = EditorStrings.Get("scene.dark") };
    private readonly ComboBox _sceneMusic = new() { Name = "SceneMusic", MinWidth = 160 };
    private readonly TextBox _sceneTemperature = new() { Name = "SceneTemperature", Width = 70, PlaceholderText = EditorStrings.Get("scene.seasons") };
    private readonly TextBox _sceneTags = new() { Name = "SceneTags", PlaceholderText = EditorStrings.Get("scene.tagsPlaceholder") };
    private readonly TextBox _sceneFoliage = new() { Name = "SceneFoliageTint", Width = 90, PlaceholderText = "#rrggbb" };
    private readonly TextBox _sceneDaylight = new() { Name = "SceneDaylight", AcceptsReturn = true, MinHeight = 54, PlaceholderText = "360 #ffd8a0" };
    private readonly CheckBox _sceneAllSeasons = new() { Name = "SceneAllSeasons", Content = EditorStrings.Get("scene.allSeasons") };
    private readonly TextBox _sceneMonsters = new() { Name = "SceneMonsters", AcceptsReturn = true, MinHeight = 40, PlaceholderText = EditorStrings.Get("scene.monstersPlaceholder") };
    private readonly TextBox _sceneLayers = new() { Name = "SceneLayers", AcceptsReturn = true, MinHeight = 40, PlaceholderText = EditorStrings.Get("scene.layersPlaceholder") };
    /// <summary>The surroundings' text fields as last shown, so a project change refreshes only the untouched ones.</summary>
    private (string Temperature, string Tags, string Foliage, string Daylight, string Monsters, string Layers) _shownSurroundings;
    private readonly Button _removeDoor = new() { Name = "RemoveDoorButton", Content = "Remove door" };
    private readonly StackPanel _transitionList = new() { Name = "TransitionList", Spacing = 6 };
    private readonly TextBlock _transitionCount = Ui.Text("", "muted", "small");
    private readonly Button _clearTransitions = new() { Name = "ClearTransitionsButton", Content = "Clear all" };
    private readonly TextBlock _doorFrom = Ui.Wrapped("Choose Door, then click a departure tile.", "muted", "small");
    private readonly TextBlock _editorMessage = Ui.Wrapped("", "muted", "small");
    private readonly TextBlock _sceneSizeInfo = Ui.Wrapped("", "muted", "small");
    private TileLayer _selectedLayer { get => ViewModel.Layer; set => ViewModel.Layer = value; }
    private MapTool _tool { get => ViewModel.Tool; set => ViewModel.Tool = value; }
    private (int X, int Y)? _gestureStart { get => ViewModel.GestureStart; set => ViewModel.GestureStart = value; }
    private (int X0, int Y0, int X1, int Y1)? _selection { get => ViewModel.Selection; set => ViewModel.Selection = value; }
    private List<List<Tile>>? _copiedTiles { get => ViewModel.CopiedTiles; set => ViewModel.CopiedTiles = value; }
    private (int X, int Y)? _doorAt { get => ViewModel.DoorAt; set => ViewModel.DoorAt = value; }
    private string? _sceneControlsSceneId;
    private string? _shownSceneName;
    private string? _shownSceneWidth;
    private string? _shownSceneHeight;
    /// <summary>The last choice of each Place tool, by placement kind.</summary>
    private Dictionary<string, string> _placeChoices => ViewModel.PlaceChoices;
    private string? _placeKind { get => ViewModel.PlaceKind; set => ViewModel.PlaceKind = value; }
    private bool _updatingPlaceChoice;
    private List<string> _shownTransitions = [];
    private List<string> _shownAnimals = [];

    public MapTool Tool
    {
        get => _tool;
        set
        {
            _tool = value;
            if (_gestureStart is not null)
            {
                // Another tool drops a rectangle's first corner (marked from the keyboard).
                _gestureStart = null;
                _hover.IsVisible = false;
            }
            foreach (var button in _tools.Children.Concat(_placeTools.Children).OfType<ToggleButton>())
            {
                button.IsChecked = Equals(button.Tag, value);
            }
            if (_workspace.Current is { } project) RefreshPlaceChoice(project);
            _canvas.Cursor = new Avalonia.Input.Cursor(value == MapTool.Inspect ? Avalonia.Input.StandardCursorType.Arrow : Avalonia.Input.StandardCursorType.Hand);
        }
    }

    public TileLayer Layer
    {
        get => _selectedLayer;
        set
        {
            _selectedLayer = value;
            SyncLayerSelector();
        }
    }

    public (int X0, int Y0, int X1, int Y1)? Selection => _selection;

    /// <summary>What the Place tool puts down (an id from <see cref="MapPlacement.Choices"/>), or null.</summary>
    public string? PlaceChoice
    {
        get => (_placeChoice.SelectedItem as ComboBoxItem)?.Tag as string;
        set => _placeChoice.SelectedItem = _placeChoice.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, value));
    }

    /// <summary>The placement kind of a Place tool (see <see cref="MapPlacement"/>), or null.</summary>
    private static string? PlacementKind(MapTool tool) => tool switch
    {
        MapTool.PlaceNpc => "npc",
        MapTool.PlaceNode => "nodeType",
        MapTool.PlaceItem => "item",
        MapTool.PlaceMachine => "machineType",
        MapTool.PlaceAnimal => "animalSpecies",
        _ => null,
    };

    private void BuildEditorPanels(StackPanel side)
    {
        void ToolButton(MapTool tool, string label, Panel? panel = null)
        {
            var button = new ToggleButton { Name = $"Tool_{tool}", Tag = tool, Content = label, Margin = new Thickness(0, 0, 6, 6) };
            button.Classes.Add("tool");
            if (panel is not null) AutomationProperties.SetName(button, tool == MapTool.Remove ? "Remove from tile" : $"Place {label}");
            if (ToolKeys.FirstOrDefault(pair => pair.Value == tool) is { Key: var key } shortcut && shortcut.Value == tool)
            {
                ToolTip.SetTip(button, $"{label} (press {key} on the map)");
                AutomationProperties.SetAcceleratorKey(button, key.ToString());
            }
            button.Click += (_, _) => Tool = tool;
            (panel ?? _tools).Children.Add(button);
        }

        ToolButton(MapTool.Inspect, "Inspect");
        ToolButton(MapTool.Brush, "Brush");
        ToolButton(MapTool.Rectangle, "Rectangle");
        ToolButton(MapTool.Fill, "Fill area");
        ToolButton(MapTool.Pick, "Pick");
        ToolButton(MapTool.Select, "Select");
        ToolButton(MapTool.Erase, "Erase layer");
        ToolButton(MapTool.Block, "Block");
        ToolButton(MapTool.Unblock, "Unblock");
        ToolButton(MapTool.Door, "Door");
        ToolButton(MapTool.PlayerStart, "Player start");
        ToolButton(MapTool.PlaceNpc, "NPC", _placeTools);
        ToolButton(MapTool.PlaceNode, "Node", _placeTools);
        ToolButton(MapTool.PlaceItem, "Item", _placeTools);
        ToolButton(MapTool.PlaceMachine, "Machine", _placeTools);
        ToolButton(MapTool.PlaceAnimal, "Animal", _placeTools);
        ToolButton(MapTool.Remove, "Remove", _placeTools);
        Tool = MapTool.Inspect;
        side.Children.Add(Ui.Text("MAP TOOLS", "section"));
        side.Children.Add(_tools);
        side.Children.Add(Ui.Wrapped("Drag with Brush, Erase, Block or Unblock. Drag a rectangle to paint or select an area. Pick takes a tile's type as the brush.", "muted", "small"));

        foreach (var (label, layer) in new[]
                 {
                     ("Background", Edits.LayerFor(TileTypes.Grass)),
                     ("Overlay", Edits.LayerFor(TileTypes.Path)),
                     ("Object", Edits.LayerFor(TileTypes.Wall)),
                 })
        {
            _layerSelector.Items.Add(new ComboBoxItem { Content = label, Tag = layer });
        }
        _layerSelector.SelectionChanged += (_, _) =>
        {
            if (_layerSelector.SelectedItem is ComboBoxItem { Tag: TileLayer layer })
            {
                _selectedLayer = layer;
            }
        };
        SyncLayerSelector();
        AutomationProperties.SetName(_layerSelector, "Layer");
        side.Children.Add(Ui.HStack(8, Ui.Text("Layer", "muted", "small"), _layerSelector));

        _copy.Click += (_, _) => CopySelection();
        _paste.Click += (_, _) => Tool = MapTool.Paste;
        side.Children.Add(Ui.HStack(8, _copy, _paste));

        side.Children.Add(Ui.Text("PLACE", "section"));
        side.Children.Add(_placeTools);
        _placeChoice.SelectionChanged += (_, _) =>
        {
            if (!_updatingPlaceChoice && _placeKind is not null && PlaceChoice is { } id) _placeChoices[_placeKind] = id;
        };
        AutomationProperties.SetName(_placeChoice, "What to place");
        side.Children.Add(_placeChoice);
        side.Children.Add(Ui.Wrapped("Choose what to place, then click a tile. Remove clears a tile's node, item, machine, crop and animals.", "muted", "small"));
        side.Children.Add(Ui.Text("Animals in this scene", "muted", "small"));
        side.Children.Add(_sceneAnimals);

        side.Children.Add(Ui.Text("SCENE", "section"));
        AutomationProperties.SetName(_sceneName, "Scene name");
        side.Children.Add(_sceneName);
        AutomationProperties.SetName(_sceneWidth, "Scene width");
        AutomationProperties.SetName(_sceneHeight, "Scene height");
        side.Children.Add(Ui.HStack(8, Ui.Text("Size", "muted", "small"), _sceneWidth, Ui.Text("×"), _sceneHeight));
        // Web SceneManager's size calculator, live as the size is typed.
        _sceneSizeInfo.Name = "SceneSizeInfo";
        _sceneWidth.TextChanged += (_, _) => RefreshSceneSizeInfo();
        _sceneHeight.TextChanged += (_, _) => RefreshSceneSizeInfo();
        side.Children.Add(_sceneSizeInfo);
        // Rain does not water an indoor scene's soil and storms do not reach its crops.
        ToolTip.SetTip(_sceneIndoor, "Greenhouses and interiors: rain does not water the soil and storms do not damage crops here.");
        _sceneIndoor.IsCheckedChanged += (_, _) =>
        {
            if (!_showingSceneIndoor) ApplyToScene(id => Edits.SetSceneIndoor(id, _sceneIndoor.IsChecked == true));
        };
        side.Children.Add(_sceneIndoor);
        BuildSurroundings(side);
        AutomationProperties.SetName(_newSceneTile, "New scene tile");
        side.Children.Add(Ui.HStack(8, Ui.Text("New scene tile", "muted", "small"), _newSceneTile));
        side.Children.Add(WrapButtons(
            NamedButton("AddSceneButton", "Add", AddScene),
            NamedButton("RenameSceneButton", "Rename", RenameScene),
            NamedButton("ResizeSceneButton", "Resize", ResizeScene)));
        side.Children.Add(WrapButtons(
            NamedButton("DuplicateSceneButton", "Duplicate", DuplicateScene), _startScene, _deleteScene));
        _startScene.Click += (_, _) => ApplyToScene(id => Edits.SetStartScene(id));
        _deleteScene.Click += (_, _) => DeleteScene();
        _fillScene.Click += (_, _) =>
        {
            if (_brush is not null) ApplyToScene(id => Edits.FillScene(id, _brush));
        };
        side.Children.Add(WrapButtons(_fillScene,
            NamedButton("ClearCropsItemsButton", "Clear crops/items", () => ApplyToScene(Edits.ClearCropsAndItems)),
            NamedButton("ResetSoilButton", "Reset soil", () => ApplyToScene(Edits.ResetSoil))));

        side.Children.Add(Ui.Text("TRANSITIONS", "section"));
        _transitionCount.Name = "TransitionCount";
        _clearTransitions.Classes.Add("tool");
        _clearTransitions.Click += (_, _) => ApplyToScene(Edits.ClearTransitions);
        side.Children.Add(Ui.HStack(8, _transitionCount, _clearTransitions));
        side.Children.Add(_transitionList);
        _doorFrom.Name = "DoorFrom";
        side.Children.Add(_doorFrom);
        AutomationProperties.SetName(_doorDestination, "To scene");
        AutomationProperties.SetName(_doorX, "At X (arrival column)");
        AutomationProperties.SetName(_doorY, "At Y (arrival row)");
        side.Children.Add(Ui.HStack(8, Ui.Text("To", "muted", "small"), _doorDestination));
        var pickArrival = NamedButton("PickDoorArrivalButton", "Pick on map", PickDoorArrival);
        AutomationProperties.SetName(pickArrival, "Pick the arrival tile on the map");
        ToolTip.SetTip(pickArrival, "Click the arrival tile in the destination scene");
        side.Children.Add(Ui.HStack(8, Ui.Text("At", "muted", "small"), _doorX, Ui.Text(","), _doorY, pickArrival));
        side.Children.Add(_doorReturn);
        side.Children.Add(Ui.HStack(8, NamedButton("SaveDoorButton", "Save door", SaveDoor), _removeDoor));
        _removeDoor.Click += (_, _) => RemoveDoor();
        // Tool results ("Door saved", "Corner marked") are announced politely.
        _editorMessage.Name = "MapEditorMessage";
        AutomationProperties.SetLiveSetting(_editorMessage, AutomationLiveSetting.Polite);
        side.Children.Add(_editorMessage);
    }

    /// <summary>The tool's button label ("Fill area"), for messages.</summary>
    private string ToolLabel(MapTool tool) =>
        _tools.Children.Concat(_placeTools.Children).OfType<ToggleButton>().FirstOrDefault(button => Equals(button.Tag, tool))?.Content as string ?? tool.ToString();

    private static Button NamedButton(string name, string label, Action action)
    {
        var button = Ui.Button(label, action, "tool");
        button.Name = name;
        return button;
    }

    private static WrapPanel WrapButtons(params Button[] buttons)
    {
        var panel = new WrapPanel();
        foreach (var button in buttons)
        {
            button.Margin = new Thickness(0, 0, 6, 6);
            panel.Children.Add(button);
        }
        return panel;
    }

    /// <summary>
    /// The Layer list: the three fixed layers, then the scene's extra layers (M8). A painted-on
    /// extra layer the scene no longer has gives way to the background.
    /// </summary>
    private void RefreshLayerSelector(Scene scene)
    {
        var extra = scene.Layers.OrEmpty().Select(layer => Edits.DecorLayer(layer.Id)).ToList();
        var shown = _layerSelector.Items.OfType<ComboBoxItem>().Skip(3).Select(item => item.Tag as TileLayer).ToList();
        if (!shown.SequenceEqual(extra))
        {
            while (_layerSelector.Items.Count > 3) _layerSelector.Items.RemoveAt(3);
            foreach (var (layer, definition) in extra.Zip(scene.Layers.OrEmpty()))
            {
                var name = string.IsNullOrWhiteSpace(definition.Name) ? definition.Id : definition.Name;
                _layerSelector.Items.Add(new ComboBoxItem { Content = EditorStrings.Format("map.extraLayer", name), Tag = layer });
            }
        }

        if (_selectedLayer.IsDecor && !extra.Contains(_selectedLayer)) _selectedLayer = Edits.LayerFor(TileTypes.Grass);
        SyncLayerSelector();
    }

    private void SyncLayerSelector()
    {
        _layerSelector.SelectedItem = _layerSelector.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, _selectedLayer));
    }

    private void RefreshEditorPanels(GameProject project, Scene? scene)
    {
        _copy.IsEnabled = _selection is not null;
        _paste.IsEnabled = _copiedTiles is not null;
        _fillScene.IsEnabled = _brush is not null;
        _deleteScene.IsEnabled = scene is not null && project.Scenes.Length > 1 && scene.Id != project.StartSceneId;
        _startScene.IsEnabled = scene is not null && scene.Id != project.StartSceneId;
        RefreshPlaceChoice(project);
        if (scene is null) return;
        RefreshSceneAnimals(project, scene);
        RefreshTransitionList(project, scene);

        var sceneChanged = _sceneControlsSceneId != scene.Id;
        if (sceneChanged)
        {
            _sceneControlsSceneId = scene.Id;
            _sceneName.Text = scene.Name;
            _sceneWidth.Text = ((int)scene.Width).ToString(CultureInfo.InvariantCulture);
            _sceneHeight.Text = ((int)scene.Height).ToString(CultureInfo.InvariantCulture);
            _doorAt = null;
            _doorFrom.Text = "Choose Door, then click a departure tile.";
        }
        else
        {
            if (_sceneName.Text == _shownSceneName) _sceneName.Text = scene.Name;
            if (_sceneWidth.Text == _shownSceneWidth) _sceneWidth.Text = ((int)scene.Width).ToString(CultureInfo.InvariantCulture);
            if (_sceneHeight.Text == _shownSceneHeight) _sceneHeight.Text = ((int)scene.Height).ToString(CultureInfo.InvariantCulture);
        }
        _shownSceneName = scene.Name;
        _shownSceneWidth = ((int)scene.Width).ToString(CultureInfo.InvariantCulture);
        _shownSceneHeight = ((int)scene.Height).ToString(CultureInfo.InvariantCulture);
        _removeDoor.IsEnabled = _doorAt is { } at && scene.Transitions.Any(t => t.FromX == at.X && t.FromY == at.Y);
        _showingSceneIndoor = true;
        _sceneIndoor.IsChecked = scene.Indoor.OrNullable() == true;
        _sceneDark.IsChecked = scene.Dark.OrNullable() == true;
        _sceneAllSeasons.IsChecked = scene.AllSeasons.OrNullable() == true;
        RefreshSurroundings(project, scene, sceneChanged);
        RefreshLayerSelector(scene);
        _showingSceneIndoor = false;
        RefreshNewSceneTiles(project);
        RefreshSceneSizeInfo();

        var previous = (_doorDestination.SelectedItem as ComboBoxItem)?.Tag as string;
        var ids = _doorDestination.Items.OfType<ComboBoxItem>().Select(item => item.Tag as string);
        if (!ids.SequenceEqual(project.Scenes.Select(s => s.Id)))
        {
            _doorDestination.Items.Clear();
            foreach (var destination in project.Scenes)
            {
                _doorDestination.Items.Add(new ComboBoxItem { Content = destination.Name, Tag = destination.Id });
            }
            _doorDestination.SelectedItem = _doorDestination.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, previous))
                ?? _doorDestination.Items.OfType<ComboBoxItem>().FirstOrDefault(item => !Equals(item.Tag, scene.Id));
        }
        else
        {
            foreach (var item in _doorDestination.Items.OfType<ComboBoxItem>())
            {
                if (project.Scenes.FirstOrDefault(s => s.Id == item.Tag as string) is { } destination)
                    item.Content = destination.Name;
            }
        }
    }

    /// <summary>
    /// The scene's surroundings (M1, M2) under its Indoor box: Dark and the music apply at once;
    /// the temperature, tags and the scene's own day/night and foliage tints with Save surroundings.
    /// </summary>
    private void BuildSurroundings(StackPanel side)
    {
        ToolTip.SetTip(_sceneDark, EditorStrings.Get("scene.darkTip"));
        _sceneDark.IsCheckedChanged += (_, _) =>
        {
            if (!_showingSceneIndoor) ApplySurroundings(dark: _sceneDark.IsChecked == true);
        };
        side.Children.Add(_sceneDark);
        // A greenhouse (M5): crops are planted and grow here in every season.
        ToolTip.SetTip(_sceneAllSeasons, EditorStrings.Get("scene.allSeasonsTip"));
        _sceneAllSeasons.IsCheckedChanged += (_, _) =>
        {
            if (!_showingSceneIndoor) ApplyToScene(id => Edits.SetSceneAllSeasons(id, _sceneAllSeasons.IsChecked == true));
        };
        side.Children.Add(_sceneAllSeasons);
        _sceneMusic.SelectionChanged += (_, _) =>
        {
            if (!_showingSceneIndoor && _sceneMusic.SelectedItem is ComboBoxItem { Tag: string id }) ApplySurroundings(music: id);
        };
        AutomationProperties.SetName(_sceneMusic, EditorStrings.Get("scene.musicName"));
        AutomationProperties.SetName(_sceneTemperature, EditorStrings.Get("scene.temperatureName"));
        AutomationProperties.SetName(_sceneTags, EditorStrings.Get("scene.tagsName"));
        AutomationProperties.SetName(_sceneFoliage, EditorStrings.Get("scene.foliageName"));
        AutomationProperties.SetName(_sceneDaylight, EditorStrings.Get("scene.daylightName"));
        ToolTip.SetTip(_sceneTemperature, EditorStrings.Get("scene.temperatureTip"));
        ToolTip.SetTip(_sceneTags, EditorStrings.Get("scene.tagsTip"));
        ToolTip.SetTip(_sceneDaylight, EditorStrings.Get("scene.daylightTip"));
        side.Children.Add(Ui.HStack(8, Ui.Text(EditorStrings.Get("common.music"), "muted", "small"), _sceneMusic));
        side.Children.Add(Ui.HStack(8, Ui.Text(EditorStrings.Get("scene.temperature"), "muted", "small"), _sceneTemperature, Ui.Text(EditorStrings.Get("common.foliageTint"), "muted", "small"), _sceneFoliage));
        side.Children.Add(Ui.VStack(3, Ui.Text(EditorStrings.Get("scene.tags"), "muted", "small"), _sceneTags));
        side.Children.Add(Ui.VStack(3, Ui.Text(EditorStrings.Get("scene.daylight"), "muted", "small"), _sceneDaylight));
        // The monsters the scene has each morning (M7), saved with the surroundings.
        AutomationProperties.SetName(_sceneMonsters, EditorStrings.Get("scene.monstersName"));
        ToolTip.SetTip(_sceneMonsters, EditorStrings.Get("scene.monstersTip"));
        side.Children.Add(Ui.VStack(3, Ui.Text(EditorStrings.Get("scene.monsters"), "muted", "small"), _sceneMonsters));
        // Its extra layers (M8), painted from the Layer list.
        AutomationProperties.SetName(_sceneLayers, EditorStrings.Get("scene.layersName"));
        ToolTip.SetTip(_sceneLayers, EditorStrings.Get("scene.layersTip"));
        side.Children.Add(Ui.VStack(3, Ui.Text(EditorStrings.Get("scene.layers"), "muted", "small"), _sceneLayers));
        side.Children.Add(NamedButton("SceneForageSpawnsButton", EditorStrings.Get("scene.forage"), EditForageSpawns));
        side.Children.Add(WrapButtons(NamedButton("SaveSurroundingsButton", EditorStrings.Get("scene.saveSurroundings"), () => ApplySurroundings(fields: true))));
    }

    /// <summary>
    /// Shows the scene's surroundings. Text fields keep what was typed into them across project
    /// changes, unless another scene is shown.
    /// </summary>
    private void RefreshSurroundings(GameProject project, Scene scene, bool sceneChanged)
    {
        var look = scene.Atmosphere.OrNull();
        var stored = (
            Temperature: scene.Temperature.OrNullable() is { } warmth ? DisplayFormat.Number(warmth) : "",
            Tags: string.Join(", ", scene.Tags.OrEmpty()),
            Foliage: look?.FoliageTint.OrNull() ?? "",
            Daylight: look is null ? "" : SettingsForm.DaylightText(look.Daylight),
            Monsters: SettingsForm.MonsterSpawnsText(scene.Monsters.OrEmpty()),
            Layers: SettingsForm.SceneLayersText(scene.Layers.OrEmpty()));
        if (sceneChanged || _sceneTemperature.Text == _shownSurroundings.Temperature) _sceneTemperature.Text = stored.Temperature;
        if (sceneChanged || _sceneTags.Text == _shownSurroundings.Tags) _sceneTags.Text = stored.Tags;
        if (sceneChanged || _sceneFoliage.Text == _shownSurroundings.Foliage) _sceneFoliage.Text = stored.Foliage;
        if (sceneChanged || _sceneDaylight.Text == _shownSurroundings.Daylight) _sceneDaylight.Text = stored.Daylight;
        if (sceneChanged || _sceneMonsters.Text == _shownSurroundings.Monsters) _sceneMonsters.Text = stored.Monsters;
        if (sceneChanged || _sceneLayers.Text == _shownSurroundings.Layers) _sceneLayers.Text = stored.Layers;
        _shownSurroundings = stored;

        var music = scene.Music.OrNull() ?? "";
        var field = ContentForms.Field(nameof(Scene), nameof(Scene.Music), "Music", project)!;
        var entries = ContentForms.Entries(field, ContentForms.Options(field.Reference, project), music);
        var shown = _sceneMusic.Items.OfType<ComboBoxItem>().Select(item => (item.Tag as string, item.Content as string));
        if (!shown.SequenceEqual(entries.Select(entry => ((string?)entry.Id, (string?)entry.Label))))
        {
            _sceneMusic.Items.Clear();
            foreach (var entry in entries) _sceneMusic.Items.Add(new ComboBoxItem { Content = entry.Label, Tag = entry.Id });
        }

        _sceneMusic.SelectedItem = _sceneMusic.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, music));
    }

    /// <summary>
    /// Saves the scene's surroundings: <paramref name="dark"/> or <paramref name="music"/> when
    /// given, the text fields when <paramref name="fields"/>, the rest as stored. One undo step.
    /// </summary>
    private void ApplySurroundings(bool? dark = null, string? music = null, bool fields = false)
    {
        if (CurrentScene() is not { } scene) return;
        try
        {
            var temperature = scene.Temperature.OrNullable();
            IEnumerable<string> tags = scene.Tags.OrEmpty();
            var atmosphere = scene.Atmosphere.OrNull();
            IEnumerable<MonsterSpawn> monsters = scene.Monsters.OrEmpty();
            IEnumerable<SceneLayer> layers = scene.Layers.OrEmpty();
            if (fields)
            {
                var warmth = (_sceneTemperature.Text ?? "").Trim();
                temperature = warmth.Length == 0 ? null
                    : double.TryParse(warmth, NumberStyles.Float, CultureInfo.InvariantCulture, out var value) && double.IsFinite(value) ? value
                    : throw new FormatException(EditorStrings.Get("scene.temperatureNumber"));
                tags = (_sceneTags.Text ?? "").Split(',');
                atmosphere = SettingsForm.Look("Scene", _sceneDaylight.Text, _sceneFoliage.Text);
                monsters = SettingsForm.MonsterSpawns(_sceneMonsters.Text);
                layers = SettingsForm.SceneLayers(scene.Layers.OrEmpty(), _sceneLayers.Text);
            }

            var saved = ViewModel.Apply(Edits.Batch(EditorStrings.Get("scene.saveSurroundings"),
            [
                Edits.SetSceneEnvironment(scene.Id, dark ?? scene.Dark.OrNullable() == true, temperature, tags, music ?? scene.Music.OrNull(), atmosphere),
                Edits.SetSceneMonsters(scene.Id, monsters),
                Edits.SetSceneLayers(scene.Id, layers),
            ]));
            if (fields) ViewModel.Message = saved ? EditorStrings.Format("scene.savedSurroundings", scene.Name) : EditorStrings.Get("common.noChanges");
        }
        catch (FormatException error)
        {
            ViewModel.Message = EditorStrings.Format("scene.couldNotSaveSurroundings", error.Message);
        }
    }

    /// <summary>The tile a new scene is filled with: one of the game's terrains (grass at first).</summary>
    private void RefreshNewSceneTiles(GameProject project)
    {
        var terrains = ContentReadouts.Terrains(project);
        if (_newSceneTile.Items.OfType<ComboBoxItem>().Select(item => item.Tag as string).SequenceEqual(terrains.Select(terrain => terrain.Id))) return;
        var previous = (_newSceneTile.SelectedItem as ComboBoxItem)?.Tag as string ?? TileTypes.Grass;
        _newSceneTile.Items.Clear();
        foreach (var terrain in terrains)
        {
            _newSceneTile.Items.Add(new ComboBoxItem { Content = string.IsNullOrWhiteSpace(terrain.Name) ? terrain.Id : terrain.Name, Tag = terrain.Id });
        }
        _newSceneTile.SelectedItem = _newSceneTile.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, previous))
            ?? _newSceneTile.Items.OfType<ComboBoxItem>().FirstOrDefault();
    }

    /// <summary>
    /// Fills the Place picker with what the current Place tool offers (from F#), keeping the
    /// creator's last choice for that tool.
    /// </summary>
    private void RefreshPlaceChoice(GameProject project)
    {
        var kind = PlacementKind(Tool);
        IReadOnlyList<PickerOption> options = kind is null ? [] : MapPlacement.Choices(kind, project);
        var shown = _placeChoice.Items.OfType<ComboBoxItem>().Select(item => ((string?)item.Tag, item.Content as string));
        if (kind == _placeKind && shown.SequenceEqual(options.Select(option => ((string?)option.Id, (string?)option.Label)))) return;

        _updatingPlaceChoice = true;
        try
        {
            _placeKind = kind;
            _placeChoice.Items.Clear();
            foreach (var option in options)
            {
                _placeChoice.Items.Add(new ComboBoxItem { Content = option.Label, Tag = option.Id });
            }
            _placeChoice.IsEnabled = kind is not null;
            _placeChoice.PlaceholderText = kind is null ? "Choose a Place tool" : options.Count == 0 ? "Nothing to place yet" : "Choose what to place";
            PlaceChoice = kind is not null && _placeChoices.TryGetValue(kind, out var previous) ? previous : null;
            if (PlaceChoice is null && _placeChoice.ItemCount > 0) _placeChoice.SelectedIndex = 0;
        }
        finally
        {
            _updatingPlaceChoice = false;
        }
    }

    /// <summary>The animals placed in this scene, each with a button that removes it.</summary>
    private void RefreshSceneAnimals(GameProject project, Scene scene)
    {
        var animals = project.Animals.Where(a => a.SceneId == scene.Id).ToList();
        var labels = animals.Select(a => $"{a.Id}|{a.Name} ({Ui.Num(Math.Floor(a.X))}, {Ui.Num(Math.Floor(a.Y))})").ToList();
        if (labels.SequenceEqual(_shownAnimals)) return;
        _shownAnimals = labels;
        _sceneAnimals.Children.Clear();
        if (animals.Count == 0)
        {
            _sceneAnimals.Children.Add(Ui.Wrapped("None yet. Choose Animal and click a tile.", "muted", "small"));
            return;
        }
        foreach (var animal in animals)
        {
            var id = animal.Id;
            var remove = NamedButton($"RemoveAnimal_{id}", "Remove", () => ViewModel.Apply(Edits.RemoveAnimal(id)));
            AutomationProperties.SetName(remove, $"Remove {animal.Name}");
            _sceneAnimals.Children.Add(Ui.Row(Ui.Wrapped($"{animal.Name} ({Ui.Num(Math.Floor(animal.X))}, {Ui.Num(Math.Floor(animal.Y))})", "small"), remove));
        }
    }

    /// <summary>
    /// The doors leaving this scene (web TransitionEditor list); a click edits one, Duplicate
    /// starts a new door with the same destination.
    /// </summary>
    private void RefreshTransitionList(GameProject project, Scene scene)
    {
        string SceneName(string id) => project.Scenes.FirstOrDefault(s => s.Id == id)?.Name ?? $"(missing: {id})";
        var labels = scene.Transitions
            .Select(t => $"({Ui.Num(t.FromX)}, {Ui.Num(t.FromY)}) → {SceneName(t.ToSceneId)} ({Ui.Num(t.ToX)}, {Ui.Num(t.ToY)})")
            .ToList();
        _transitionCount.Text = $"{labels.Count} transition(s) from this scene";
        _clearTransitions.IsEnabled = labels.Count > 0;
        if (labels.SequenceEqual(_shownTransitions)) return;
        _shownTransitions = labels;
        _transitionList.Children.Clear();
        for (var index = 0; index < labels.Count; index++)
        {
            var transition = scene.Transitions[index];
            var (x, y) = ((int)transition.FromX, (int)transition.FromY);
            var button = NamedButton($"Transition_{index}", labels[index], () =>
            {
                Tool = MapTool.Door;
                ChooseDoor(x, y);
            });
            button.HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Stretch;
            var duplicate = NamedButton($"DuplicateTransition_{index}", "Duplicate", () => DuplicateTransition(transition));
            AutomationProperties.SetName(duplicate, $"Duplicate transition {labels[index]}");
            var row = new Grid { ColumnDefinitions = new ColumnDefinitions("*,Auto") };
            row.Children.Add(button);
            duplicate.Margin = new Thickness(6, 0, 0, 0);
            Grid.SetColumn(duplicate, 1);
            row.Children.Add(duplicate);
            _transitionList.Children.Add(row);
        }
    }

    private bool BeginToolAt((int X, int Y) tile)
    {
        var scene = CurrentScene();
        if (scene is null) return false;
        // A form's "Pick on map" takes the tile before any tool (#47).
        if (CompletePick(tile)) return true;
        switch (Tool)
        {
            case MapTool.Inspect:
                return OpenMarkerAt(tile);
            case MapTool.Pick:
                PickAt(tile.X, tile.Y);
                return true;
            case MapTool.PlaceNpc:
            case MapTool.PlaceNode:
            case MapTool.PlaceItem:
            case MapTool.PlaceMachine:
            case MapTool.PlaceAnimal:
                PlaceAt(tile.X, tile.Y);
                return true;
            case MapTool.Remove:
                ViewModel.Apply(Edits.ClearTile(scene.Id, tile.X, tile.Y));
                return true;
            case MapTool.Brush when _brush is not null:
            case MapTool.Erase:
            case MapTool.Block:
            case MapTool.Unblock:
                _strokeId = $"map-{++_strokeCount}";
                _lastPainted = tile;
                ApplyStrokeTile(tile.X, tile.Y);
                return true;
            case MapTool.Rectangle when _brush is not null:
            case MapTool.Select:
                _gestureStart = tile;
                _lastPainted = tile;
                return true;
            case MapTool.Fill when _brush is not null:
                ViewModel.Apply(Edits.FloodFill(scene.Id, Layer, tile.X, tile.Y, _brush));
                return true;
            case MapTool.Paste:
                PasteAt(tile.X, tile.Y);
                return true;
            case MapTool.Door:
                ChooseDoor(tile.X, tile.Y);
                return true;
            case MapTool.PlayerStart:
                ViewModel.Apply(Edits.SetPlayerStart(scene.Id, tile.X, tile.Y));
                return true;
            default:
                return false;
        }
    }

    private void ApplyStrokeTile(int x, int y) => ViewModel.ApplyStrokeTile(x, y);

    private void UpdateGesturePreview((int X, int Y) start, (int X, int Y) end)
    {
        _lastPainted = end;
        var first = _canvas.TileRect(Math.Min(start.X, end.X), Math.Min(start.Y, end.Y));
        var last = _canvas.TileRect(Math.Max(start.X, end.X), Math.Max(start.Y, end.Y));
        _hover.Margin = new Thickness(first.X, first.Y, 0, 0);
        _hover.Width = last.Right - first.Left;
        _hover.Height = last.Bottom - first.Top;
        _hover.IsVisible = true;
    }

    private void CompleteGesture((int X, int Y) end)
    {
        ViewModel.CompleteGesture(end);
        _copy.IsEnabled = ViewModel.Selection is not null;
        _hover.IsVisible = false;
    }

    private void ClearSelection()
    {
        ViewModel.ClearSelection();
        _copy.IsEnabled = false;
    }

    public void CopySelection()
    {
        ViewModel.CopySelection();
        _copy.IsEnabled = ViewModel.Selection is not null;
        _paste.IsEnabled = ViewModel.CopiedTiles is not null;
    }

    public void PasteAt(int x, int y) => ViewModel.PasteAt(x, y);

    /// <summary>Eyedropper: the tile's type (and that layer's art) becomes the brush.</summary>
    public void PickAt(int x, int y)
    {
        if (ViewModel.PickAt(x, y)) SetBrush(ViewModel.Brush, ViewModel.BrushVisual);
    }

    /// <summary>The current Place tool puts its chosen NPC, node, item, machine or animal on the tile.</summary>
    public void PlaceAt(int x, int y)
    {
        var choice = _placeChoice.SelectedItem as ComboBoxItem;
        ViewModel.PlaceAt(x, y, choice?.Tag as string, choice?.Content as string);
    }

    private void ApplyToScene(Func<string, Edit> createEdit)
    {
        if (CurrentScene() is { } scene) ViewModel.Apply(createEdit(scene.Id));
    }

    /// <summary>
    /// Tile count and aspect ratio of the typed size, and what a resize to it would cut off: the
    /// tiles, and each thing standing on or pointing at them that moves inside (#44).
    /// </summary>
    private void RefreshSceneSizeInfo() => _sceneSizeInfo.Text = ViewModel.SceneSizeInfo();

    private void AddScene()
    {
        var tile = (_newSceneTile.SelectedItem as ComboBoxItem)?.Tag as string ?? TileTypes.Grass;
        if (ViewModel.AddScene(tile) is { } id) SelectScene(id);
    }

    private void RenameScene() => ViewModel.RenameScene();

    private void ResizeScene()
    {
        if (ViewModel.ResizeScene())
        {
            ClearSelection();
            FitToView();
        }
    }

    private void DuplicateScene()
    {
        if (ViewModel.DuplicateScene() is { } id) SelectScene(id);
    }

    private void DeleteScene() => ViewModel.DeleteScene();

    private void ChooseDoor(int x, int y)
    {
        var scene = CurrentScene();
        if (scene is null) return;
        _doorAt = (x, y);
        _doorFrom.Text = $"From {scene.Name} ({x}, {y})";
        if (scene.Transitions.FirstOrDefault(t => t.FromX == x && t.FromY == y) is { } existing)
        {
            _doorDestination.SelectedItem = _doorDestination.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, existing.ToSceneId));
            _doorX.Text = ((int)existing.ToX).ToString(CultureInfo.InvariantCulture);
            _doorY.Text = ((int)existing.ToY).ToString(CultureInfo.InvariantCulture);
            _doorReturn.IsChecked = false;
            _removeDoor.IsEnabled = true;
        }
        else
        {
            _removeDoor.IsEnabled = false;
        }
    }

    /// <summary>
    /// Web TransitionEditor "Duplicate": the door form gets the same destination and arrival tile,
    /// departing from the next free tile (one to the right, else onward to the right and below).
    /// Save door creates it.
    /// </summary>
    private void DuplicateTransition(SceneTransition transition)
    {
        if (CurrentScene() is not { } scene) return;
        var from = FreeDoorTile(scene, (int)transition.FromX + 1, (int)transition.FromY);
        Tool = MapTool.Door;
        ChooseDoor(from.X, from.Y);
        SetCursor(from);
        _doorDestination.SelectedItem = _doorDestination.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, transition.ToSceneId));
        _doorX.Text = ((int)transition.ToX).ToString(CultureInfo.InvariantCulture);
        _doorY.Text = ((int)transition.ToY).ToString(CultureInfo.InvariantCulture);
        _doorReturn.IsChecked = false;
        ViewModel.Message = $"Copied the door to ({from.X}, {from.Y}). Adjust the departure tile (click the map), then choose Save door.";
    }

    /// <summary>The first tile without a door from (x, y), clamped into the scene, scanning right then down.</summary>
    private static (int X, int Y) FreeDoorTile(Scene scene, int x, int y) => MapEditorViewModel.FreeDoorTile(scene, x, y);

    private void SaveDoor() => ViewModel.SaveDoor();

    private void RemoveDoor() => ViewModel.RemoveDoor();
}
