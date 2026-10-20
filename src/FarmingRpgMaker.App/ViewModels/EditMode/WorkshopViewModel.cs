using System.Globalization;
using FarmEngine.Authoring;
using FarmingRpgMaker.App.Mvvm;
using FarmingRpgMaker.App.Projects;

namespace FarmingRpgMaker.App.ViewModels.EditMode;

/// <summary>A character the Workshop's patterns can use (an empty id: none chosen).</summary>
public sealed record WorkshopCharacter(string Id, string Name);

/// <summary>
/// The Workshop tab: the creator pattern form (pattern, name, text, scene tile, delivery day,
/// friendship, character) and what creating one says. The F# patterns build the content; it
/// is applied as one undo step.
/// </summary>
public sealed class WorkshopViewModel : ObservableObject
{
    /// <summary>"Build your game" shortcuts: web editor tab keys and their labels (web CreatorWorkshop).</summary>
    public static IReadOnlyList<(string Tab, string Label)> QuickLinks { get; } =
    [
        ("assets", "Import art"), ("scenes", "Create scenes"), ("crops", "Crops & seeds"), ("wildlife", "Animals & fish"),
        ("items", "Inventory items"), ("craft", "Recipes & stations"), ("npcs", "Characters & dialogue"), ("quests", "Quests"),
        ("events", "Story events"), ("actions", "Magic & minigames"), ("problems", "Check project"),
    ];

    /// <summary>The first character choice: none.</summary>
    public static WorkshopCharacter NoCharacter { get; } = new("", "Choose a character");

    private readonly ProjectWorkspace _workspace;
    private readonly Func<string?> _mapScene;
    private readonly Action<string, string, Action<int, int>> _pickOnMap;
    private readonly Action<string> _openEditor;
    private PatternInfo? _pattern;
    private string _name = "";
    private string _text = "";
    private string _x = "1";
    private string _y = "1";
    private string _day = "2";
    private string _friendship = "500";
    private bool _rememberPromise = true;
    private IReadOnlyList<WorkshopCharacter> _characters = [NoCharacter];
    private string _characterId = "";
    private string _sceneText = "";
    private string _message = "";

    /// <param name="workspace">The open project.</param>
    /// <param name="mapScene">The scene the Map tab shows: the patterns put their tile there (#47).</param>
    /// <param name="pickOnMap">Lets the creator click the pattern's tile on the map (scene id, prompt, what to do with the tile).</param>
    /// <param name="openEditor">Opens an editor tab by its web tab key.</param>
    public WorkshopViewModel(ProjectWorkspace workspace, Func<string?> mapScene, Action<string, string, Action<int, int>> pickOnMap, Action<string> openEditor)
    {
        _workspace = workspace ?? throw new ArgumentNullException(nameof(workspace));
        _mapScene = mapScene ?? throw new ArgumentNullException(nameof(mapScene));
        _pickOnMap = pickOnMap ?? throw new ArgumentNullException(nameof(pickOnMap));
        _openEditor = openEditor ?? throw new ArgumentNullException(nameof(openEditor));
        _pattern = Patterns.All.FirstOrDefault();
        CreateCommand = new RelayCommand(Create);
        PickTileCommand = new RelayCommand(PickTile);
        OpenEditorCommand = new RelayCommand(OpenEditor);
        _workspace.ProjectChanged += OnProjectChanged;
        Refresh();
    }

    /// <summary>The patterns the form offers (F#).</summary>
    public static IReadOnlyList<PatternInfo> AllPatterns => Patterns.All;

    /// <summary>The chosen pattern; choosing one shows its help and clears the last message.</summary>
    public PatternInfo? SelectedPattern
    {
        get => _pattern;
        set
        {
            if (!SetProperty(ref _pattern, value)) return;
            OnPropertyChanged(nameof(Help));
            Message = "";
        }
    }

    /// <summary>What the chosen pattern makes.</summary>
    public string Help => _pattern?.Help ?? "";

    public string Name
    {
        get => _name;
        set => SetProperty(ref _name, value ?? "");
    }

    /// <summary>Story text or description.</summary>
    public string Text
    {
        get => _text;
        set => SetProperty(ref _text, value ?? "");
    }

    /// <summary>The scene tile's X, as typed.</summary>
    public string X
    {
        get => _x;
        set => SetProperty(ref _x, value ?? "");
    }

    /// <summary>The scene tile's Y, as typed.</summary>
    public string Y
    {
        get => _y;
        set => SetProperty(ref _y, value ?? "");
    }

    /// <summary>The delivery day, as typed.</summary>
    public string Day
    {
        get => _day;
        set => SetProperty(ref _day, value ?? "");
    }

    /// <summary>The friendship points, as typed.</summary>
    public string Friendship
    {
        get => _friendship;
        set => SetProperty(ref _friendship, value ?? "");
    }

    /// <summary>"Remember the player's promise" (the pattern's consequences).</summary>
    public bool RememberPromise
    {
        get => _rememberPromise;
        set => SetProperty(ref _rememberPromise, value);
    }

    /// <summary><see cref="NoCharacter"/>, then the project's characters.</summary>
    public IReadOnlyList<WorkshopCharacter> Characters
    {
        get => _characters;
        private set => SetProperty(ref _characters, value);
    }

    /// <summary>The chosen character's id ("" for none).</summary>
    public string CharacterId
    {
        get => _characterId;
        set => SetProperty(ref _characterId, value ?? "");
    }

    /// <summary>"In the scene the Map tab shows: Farm." (empty without a scene).</summary>
    public string SceneText
    {
        get => _sceneText;
        private set => SetProperty(ref _sceneText, value);
    }

    /// <summary>What the last action said (created, why not, the tile picked).</summary>
    public string Message
    {
        get => _message;
        private set => SetProperty(ref _message, value);
    }

    public RelayCommand CreateCommand { get; }

    /// <summary>Lets the creator click the pattern's tile on the map.</summary>
    public RelayCommand PickTileCommand { get; }

    /// <summary>Opens the editor of what the chosen pattern makes.</summary>
    public RelayCommand OpenEditorCommand { get; }

    /// <summary>Stops following the project (the editor that built this tab was replaced).</summary>
    public void Retire() => _workspace.ProjectChanged -= OnProjectChanged;

    /// <summary>The scene the patterns use: the Map tab's, else the player's.</summary>
    public string? SceneId() =>
        _mapScene() is { } id && _workspace.Current?.Scenes.Any(s => s.Id == id) == true ? id : _workspace.Current?.Player.SceneId;

    /// <summary>Catches up with the project: the scene line and the characters (a removed one is unchosen).</summary>
    public void Refresh()
    {
        var sceneId = SceneId();
        var sceneName = _workspace.Current?.Scenes.FirstOrDefault(s => s.Id == sceneId)?.Name;
        SceneText = sceneName is null ? "" : $"In the scene the Map tab shows: {sceneName}.";
        List<WorkshopCharacter> characters = [NoCharacter, .. (_workspace.Current?.Npcs ?? []).Select(npc => new WorkshopCharacter(npc.Id, npc.Name))];
        if (!characters.SequenceEqual(_characters)) Characters = characters;
        if (!characters.Any(character => character.Id == _characterId)) CharacterId = "";
    }

    /// <summary>A Workshop "Build your game" shortcut (a web editor tab key).</summary>
    public void OpenLink(string tabKey) => _openEditor(tabKey);

    private void PickTile()
    {
        if (SceneId() is not { } sceneId) return;
        _pickOnMap(sceneId, "Click the tile for the pattern.", (x, y) =>
        {
            X = x.ToString(CultureInfo.InvariantCulture);
            Y = y.ToString(CultureInfo.InvariantCulture);
            Message = $"Tile ({x}, {y}) chosen.";
        });
    }

    private void Create()
    {
        if (_workspace.Current is not { } project || _pattern is not { } pattern) return;
        try
        {
            var options = Patterns.Options(_name, _text, Parse(_x, "Scene tile X"), Parse(_y, "Scene tile Y"), Parse(_day, "Delivery day"),
                _characterId, Parse(_friendship, "Friendship"), _rememberPromise, SceneId() ?? "");
            var result = Patterns.Build(pattern.Kind, options, project);
            if (!result.Ok || result.Edit is null)
            {
                Message = result.Error ?? "This pattern could not be created.";
                return;
            }

            _workspace.Apply(result.Edit);
            Message = Patterns.SuccessMessage(pattern.Kind, options.Name);
        }
        catch (FormatException error)
        {
            Message = error.Message;
        }
    }

    private void OpenEditor()
    {
        if (_pattern is { } pattern) _openEditor(pattern.Tab);
    }

    /// <summary>A whole number of 0 or more; the error names the field (<paramref name="label"/>).</summary>
    private static int Parse(string text, string label) =>
        int.TryParse(text, NumberStyles.Integer, CultureInfo.InvariantCulture, out var value) && value >= 0
            ? value
            : throw new FormatException($"{label} must be a whole number of 0 or more.");

    private void OnProjectChanged(object? sender, ProjectChangedEventArgs e) => Refresh();
}
