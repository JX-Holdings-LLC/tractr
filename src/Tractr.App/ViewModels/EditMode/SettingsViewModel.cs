using System.Collections.ObjectModel;
using System.Globalization;
using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using Tractr.App.Mvvm;
using Tractr.App.Projects;

namespace Tractr.App.ViewModels.EditMode;

/// <summary>The parts of the Settings form that are saved (and reloaded) on their own.</summary>
public enum SettingsSection
{
    Project,
    Weather,
    Atmosphere,
    Mine,
    Export,
}

/// <summary>
/// The Settings tab: project identity, gameplay settings, the game's words and calendar (saved
/// as one F# undo step), the weather (odds, overlays, each type's look and music), the
/// atmosphere (day/night and foliage tints), the mine and export settings. Each section keeps what was typed into it
/// until it is saved or reverted (#70): saving another section, toggling the mine, undo or a tab
/// switch reload only the sections without unsaved fields. Season moves and removals change the
/// draft rows and are saved with the project settings. Errors name the field (#46): the labels
/// here are the ones the view shows.
/// </summary>
public sealed class SettingsViewModel : ObservableObject
{
    public const string NameLabel = "Name";
    public const string VersionLabel = "Version";
    public const string LocaleLabel = "Locale";
    public const string PlayerSpeedLabel = "Player speed (tiles/sec)";
    public const string MaxEnergyLabel = "Max energy";
    public const string CollapseFractionLabel = "Collapse energy fraction";
    public const string CollapsePenaltyLabel = "Collapse money penalty";
    public const string DayStartLabel = "Day start minute";
    public const string DayEndLabel = "Day end minute";
    public const string MinutesPerSecondLabel = "Game minutes per real second";
    public const string DayMinutesLabel = "Minutes per day";
    public const string MoneyFormatLabel = "Money format";
    public const string EnergyWordLabel = "Energy word";
    public const string MineSceneLabel = "Mine entrance scene";
    public const string MineXLabel = "Mine entrance X";
    public const string MineYLabel = "Mine entrance Y";
    public const string MineFloorsLabel = "Mine floors";
    public const string MineLadderLabel = "Ladder chance (0.02–1)";
    public const string WindowWidthLabel = "Window width";
    public const string WindowHeightLabel = "Window height";

    /// <summary>The fastest player speed the form accepts, in tiles a second.</summary>
    public const double MaxPlayerSpeed = 15;

    private readonly ProjectWorkspace _workspace;
    private readonly Func<bool> _isShown;
    private readonly Action<string, string, Action<int, int>>? _pickOnMap;

    /// <summary>Each section's fields as they were last loaded from the project.</summary>
    private readonly Dictionary<SettingsSection, string> _loaded = [];

    /// <summary>The saved seasons when the project section was loaded (a row removed since is removed on save).</summary>
    private List<string> _loadedSeasons = [];

    private string _name = "";
    private string _version = "";
    private string _locale = "";
    private string _playerSpeed = "";
    private string _maxEnergy = "";
    private string _collapseFraction = "";
    private string _collapsePenalty = "";
    private string _dayStart = "";
    private string _dayEnd = "";
    private string _minutesPerSecond = "";
    private string _dayMinutes = "";
    private string _moneyFormat = "";
    private string _energyWord = "";
    private string _atmosphereMessage = "";
    private bool _pauseInModals;
    private bool _energyEnabled;
    private bool _skillsEnabled;
    private bool _showCredit;
    private string _message = "";
    private IReadOnlyList<WeatherSeasonRow> _weather = [];
    private string _weatherMessage = "";
    private bool _mineEnabled;
    private IReadOnlyList<SceneChoice> _mineScenes = [];
    private string? _mineSceneId;
    private string _mineX = "";
    private string _mineY = "";
    private string _mineFloors = "";
    private string _mineLadder = "";
    private string _mineNote = "";
    private string _mineMessage = "";
    private string _exportTitle = "";
    private string _exportExecutable = "";
    private string _exportVersion = "";
    private string _exportAuthor = "";
    private string _exportCompany = "";
    private string _exportGameId = "";
    private IReadOnlyList<PickerOption> _iconOptions = [];
    private string? _exportIcon;
    private string _exportWidth = "";
    private string _exportHeight = "";
    private bool _exportFullscreen;
    private IReadOnlyList<PickerOption> _pixelScaleOptions = [];
    private string? _exportPixelScale;
    private string _exportCredits = "";
    private IReadOnlyList<Problem> _exportProblems = [];
    private string _exportMessage = "";

    /// <param name="workspace">The open project.</param>
    /// <param name="pickOnMap">Lets the creator click the mine entrance on the map (scene id, prompt, what to do with the tile); null: no Pick on map.</param>
    /// <param name="isShown">Whether the tab is on show: project changes refresh it only then (opening it refreshes too).</param>
    public SettingsViewModel(ProjectWorkspace workspace, Action<string, string, Action<int, int>>? pickOnMap = null, Func<bool>? isShown = null)
    {
        _workspace = workspace ?? throw new ArgumentNullException(nameof(workspace));
        _pickOnMap = pickOnMap;
        _isShown = isShown ?? (() => true);
        SaveCommand = new RelayCommand(Save);
        RevertCommand = new RelayCommand(() => Load(SettingsSection.Project));
        AddSkillLevelCommand = new RelayCommand(AddSkillLevel);
        AddSeasonCommand = new RelayCommand(AddSeason);
        AddFestivalCommand = new RelayCommand(AddFestival);
        SaveWeatherCommand = new RelayCommand(SaveWeather);
        AddOverlayCommand = new RelayCommand(AddOverlay);
        SaveAtmosphereCommand = new RelayCommand(SaveAtmosphere);
        RevertAtmosphereCommand = new RelayCommand(() => Load(SettingsSection.Atmosphere));
        SaveMineCommand = new RelayCommand(SaveMine);
        PickMineEntranceCommand = new RelayCommand(PickMineEntrance, () => _pickOnMap is not null);
        SaveExportCommand = new RelayCommand(SaveExport);
        Overlays.CollectionChanged += OnOverlaysChanged;
        _workspace.ProjectChanged += OnProjectChanged;
        Refresh();
    }

    // ---- Project section ----

    public string Name { get => _name; set => SetProperty(ref _name, value ?? ""); }

    public string Version { get => _version; set => SetProperty(ref _version, value ?? ""); }

    public string Locale { get => _locale; set => SetProperty(ref _locale, value ?? ""); }

    public string PlayerSpeed { get => _playerSpeed; set => SetProperty(ref _playerSpeed, value ?? ""); }

    public string MaxEnergy { get => _maxEnergy; set => SetProperty(ref _maxEnergy, value ?? ""); }

    public string CollapseFraction { get => _collapseFraction; set => SetProperty(ref _collapseFraction, value ?? ""); }

    public string CollapsePenalty { get => _collapsePenalty; set => SetProperty(ref _collapsePenalty, value ?? ""); }

    public string DayStart { get => _dayStart; set => SetProperty(ref _dayStart, value ?? ""); }

    public string DayEnd { get => _dayEnd; set => SetProperty(ref _dayEnd, value ?? ""); }

    public string MinutesPerSecond { get => _minutesPerSecond; set => SetProperty(ref _minutesPerSecond, value ?? ""); }

    /// <summary>Minutes in a game day (empty: the built-in 1440).</summary>
    public string DayMinutes { get => _dayMinutes; set => SetProperty(ref _dayMinutes, value ?? ""); }

    /// <summary>How money is written, {0} standing for the amount (empty: the built-in "$12").</summary>
    public string MoneyFormat { get => _moneyFormat; set => SetProperty(ref _moneyFormat, value ?? ""); }

    /// <summary>What energy is called (empty: "Energy").</summary>
    public string EnergyWord { get => _energyWord; set => SetProperty(ref _energyWord, value ?? ""); }

    /// <summary>The skills' names, one row per skill (empty: the built-in name).</summary>
    public ObservableCollection<SkillNameDraft> SkillNames { get; } = [];

    /// <summary>"Pause the clock in dialogue, shops, minigames and menus".</summary>
    public bool PauseInModals { get => _pauseInModals; set => SetProperty(ref _pauseInModals, value); }

    public bool EnergyEnabled { get => _energyEnabled; set => SetProperty(ref _energyEnabled, value); }

    public bool SkillsEnabled { get => _skillsEnabled; set => SetProperty(ref _skillsEnabled, value); }

    /// <summary>"Show creator credit".</summary>
    public bool ShowCredit { get => _showCredit; set => SetProperty(ref _showCredit, value); }

    /// <summary>The skill curve's rows: index <c>i</c> is level <c>i</c>.</summary>
    public ObservableCollection<SkillLevelDraft> SkillLevels { get; } = [];

    /// <summary>The calendar's seasons in year order.</summary>
    public ObservableCollection<SeasonDraft> Seasons { get; } = [];

    public ObservableCollection<FestivalDraft> Festivals { get; } = [];

    /// <summary>The project section's status line.</summary>
    public string Message { get => _message; private set => SetProperty(ref _message, value); }

    public RelayCommand SaveCommand { get; }

    /// <summary>Reloads the project section's fields from the project.</summary>
    public RelayCommand RevertCommand { get; }

    public RelayCommand AddSkillLevelCommand { get; }

    public RelayCommand AddSeasonCommand { get; }

    public RelayCommand AddFestivalCommand { get; }

    // ---- Weather section ----

    /// <summary>One row per season, one weight per weather type.</summary>
    public IReadOnlyList<WeatherSeasonRow> Weather { get => _weather; private set => SetProperty(ref _weather, value); }

    public string WeatherMessage { get => _weatherMessage; private set => SetProperty(ref _weatherMessage, value); }

    public RelayCommand SaveWeatherCommand { get; }

    /// <summary>The game's weather overlays (beside the built-in rain and snow).</summary>
    public ObservableCollection<OverlayDraft> Overlays { get; } = [];

    /// <summary>Each weather type's overlay and music.</summary>
    public ObservableCollection<WeatherLookDraft> WeatherLooks { get; } = [];

    /// <summary>The overlay styles a row can pick.</summary>
    public IReadOnlyList<string> OverlayStyles => SettingsForm.OverlayStyles;

    public RelayCommand AddOverlayCommand { get; }

    // ---- Atmosphere section ----

    /// <summary>The look of every season first, then each season's own.</summary>
    public ObservableCollection<LookDraft> Looks { get; } = [];

    public string AtmosphereMessage { get => _atmosphereMessage; private set => SetProperty(ref _atmosphereMessage, value); }

    public RelayCommand SaveAtmosphereCommand { get; }

    /// <summary>Reloads the atmosphere's fields from the project.</summary>
    public RelayCommand RevertAtmosphereCommand { get; }

    // ---- Choices ----

    /// <summary>The music a season, festival or weather type can play: none, the game's music assets and a missing <paramref name="current"/>.</summary>
    public IReadOnlyList<PickerOption> MusicChoices(string? current) => Choices(nameof(CalendarSeason), nameof(CalendarSeason.Music), current);

    /// <summary>
    /// The overlays a weather type can show: none, rain, snow, the game's overlays as the rows
    /// above have them (one added there is offered before the weather is saved) and a missing
    /// <paramref name="current"/>.
    /// </summary>
    public IReadOnlyList<PickerOption> OverlayChoices(string? current)
    {
        if (_workspace.Current is not { } project) return [];
        var drafted = Overlays.Select(row => row.Id.Trim()).Where(id => id.Length > 0).Distinct(StringComparer.Ordinal)
            .Select(id => WeatherOverlayDefinition.Default.WithId(id)).ToList();
        return Choices(nameof(WeatherTypeDefinition), nameof(WeatherTypeDefinition.Overlay), current, project.WithWeather(project.Weather.WithOverlays(drafted)));
    }

    /// <summary>Raised when <see cref="OverlayChoices"/> may offer other overlays: a row was added or removed, or its id changed.</summary>
    public event EventHandler? OverlayChoicesChanged;

    private IReadOnlyList<PickerOption> Choices(string owner, string property, string? current, GameProject? draft = null)
    {
        if (_workspace.Current is not { } project || ContentForms.Field(owner, property, property, project) is not { } field) return [];
        return ContentForms.Entries(field, ContentForms.Options(field.Reference, draft ?? project), current);
    }

    private void OnOverlaysChanged(object? sender, System.Collections.Specialized.NotifyCollectionChangedEventArgs e)
    {
        foreach (var row in e.NewItems?.OfType<OverlayDraft>() ?? [])
        {
            row.PropertyChanged += OnOverlayRowChanged;
        }

        foreach (var row in e.OldItems?.OfType<OverlayDraft>() ?? [])
        {
            row.PropertyChanged -= OnOverlayRowChanged;
        }

        OverlayChoicesChanged?.Invoke(this, EventArgs.Empty);
    }

    private void OnOverlayRowChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(OverlayDraft.Id)) OverlayChoicesChanged?.Invoke(this, EventArgs.Empty);
    }

    // ---- Mine section ----

    /// <summary>Whether the project has a mine (toggling it is applied at once: <see cref="SetMineEnabled"/>).</summary>
    public bool MineEnabled { get => _mineEnabled; private set => SetProperty(ref _mineEnabled, value); }

    public IReadOnlyList<SceneChoice> MineScenes { get => _mineScenes; private set => SetProperty(ref _mineScenes, value); }

    /// <summary>The entrance scene chosen (null: none of the scenes).</summary>
    public string? MineSceneId { get => _mineSceneId; set => SetProperty(ref _mineSceneId, value); }

    public string MineX { get => _mineX; set => SetProperty(ref _mineX, value ?? ""); }

    public string MineY { get => _mineY; set => SetProperty(ref _mineY, value ?? ""); }

    public string MineFloors { get => _mineFloors; set => SetProperty(ref _mineFloors, value ?? ""); }

    public string MineLadder { get => _mineLadder; set => SetProperty(ref _mineLadder, value ?? ""); }

    /// <summary>How players use the mine (the elevator checkpoints).</summary>
    public string MineNote { get => _mineNote; private set => SetProperty(ref _mineNote, value); }

    public string MineMessage { get => _mineMessage; private set => SetProperty(ref _mineMessage, value); }

    public RelayCommand SaveMineCommand { get; }

    /// <summary>Lets the creator click the mine entrance on the map; only offered with a map to pick on.</summary>
    public RelayCommand PickMineEntranceCommand { get; }

    /// <summary>True when the mine entrance can be picked on the map.</summary>
    public bool CanPickOnMap => _pickOnMap is not null;

    // ---- Export section ----

    /// <summary>The game title (empty: the project's name).</summary>
    public string ExportTitle { get => _exportTitle; set => SetProperty(ref _exportTitle, value ?? ""); }

    public string ExportExecutable { get => _exportExecutable; set => SetProperty(ref _exportExecutable, value ?? ""); }

    /// <summary>The game version (empty: the project's).</summary>
    public string ExportVersion { get => _exportVersion; set => SetProperty(ref _exportVersion, value ?? ""); }

    public string ExportAuthor { get => _exportAuthor; set => SetProperty(ref _exportAuthor, value ?? ""); }

    public string ExportCompany { get => _exportCompany; set => SetProperty(ref _exportCompany, value ?? ""); }

    /// <summary>Keeps save folders stable; shown, not edited.</summary>
    public string ExportGameId { get => _exportGameId; private set => SetProperty(ref _exportGameId, value); }

    public IReadOnlyList<PickerOption> IconOptions { get => _iconOptions; private set => SetProperty(ref _iconOptions, value); }

    /// <summary>The icon artwork chosen ("" for none; null when no choice matches).</summary>
    public string? ExportIcon { get => _exportIcon; set => SetProperty(ref _exportIcon, value); }

    public string ExportWidth { get => _exportWidth; set => SetProperty(ref _exportWidth, value ?? ""); }

    public string ExportHeight { get => _exportHeight; set => SetProperty(ref _exportHeight, value ?? ""); }

    public bool ExportFullscreen { get => _exportFullscreen; set => SetProperty(ref _exportFullscreen, value); }

    public IReadOnlyList<PickerOption> PixelScaleOptions { get => _pixelScaleOptions; private set => SetProperty(ref _pixelScaleOptions, value); }

    /// <summary>The pixel scale chosen (null when no choice matches: saving keeps the current one).</summary>
    public string? ExportPixelScale { get => _exportPixelScale; set => SetProperty(ref _exportPixelScale, value); }

    public string ExportCredits { get => _exportCredits; set => SetProperty(ref _exportCredits, value ?? ""); }

    /// <summary>What F# says about the saved export settings.</summary>
    public IReadOnlyList<Problem> ExportProblems { get => _exportProblems; private set => SetProperty(ref _exportProblems, value); }

    public string ExportMessage { get => _exportMessage; private set => SetProperty(ref _exportMessage, value); }

    public RelayCommand SaveExportCommand { get; }

    // ---- Drafts ----

    /// <summary>True while a section holds fields that are not saved (#70, #87).</summary>
    public bool HasUnsavedChanges => Enum.GetValues<SettingsSection>().Any(IsDirty);

    /// <summary>True while <paramref name="section"/> holds fields that are not saved.</summary>
    public bool IsDirty(SettingsSection section) => _loaded.TryGetValue(section, out var loaded) && loaded != Snapshot(section);

    /// <summary>
    /// Saves every section with unsaved fields (switching projects, closing the editor). False
    /// when one could not be saved; its message says why.
    /// </summary>
    public bool SaveDrafts()
    {
        if (IsDirty(SettingsSection.Project)) Save();
        if (IsDirty(SettingsSection.Weather)) SaveWeather();
        if (IsDirty(SettingsSection.Atmosphere)) SaveAtmosphere();
        if (IsDirty(SettingsSection.Mine)) SaveMine();
        if (IsDirty(SettingsSection.Export)) SaveExport();
        return !HasUnsavedChanges;
    }

    /// <summary>Stops following the project (the editor that built this tab was replaced).</summary>
    public void Retire() => _workspace.ProjectChanged -= OnProjectChanged;

    /// <summary>
    /// Reloads the sections without unsaved fields from the project (the tab is shown, the
    /// project changed). A section with unsaved fields keeps them and says so.
    /// </summary>
    public void Refresh()
    {
        if (_workspace.Current is not { } project) return;
        PeopleChanged?.Invoke();
        var kept = new List<string>();
        foreach (var section in Enum.GetValues<SettingsSection>())
        {
            if (IsDirty(section))
            {
                kept.Add(section.ToString().ToLowerInvariant());
                continue;
            }

            Load(section);
        }

        // The mine toggle is applied at once, whatever its fields hold.
        MineEnabled = project.Mine.Enabled;
        if (kept.Count > 0 && _isShown())
        {
            Message = $"Unsaved changes kept ({string.Join(", ", kept)}): save or revert them.";
        }
    }

    /// <summary>A section's fields as one string, to tell whether anything was typed since it was loaded.</summary>
    private string Snapshot(SettingsSection section)
    {
        static string Flag(bool value) => value.ToString();
        IEnumerable<string> fields = section switch
        {
            SettingsSection.Project =>
            [
                _name, _version, _locale, _playerSpeed, _maxEnergy, _collapseFraction, _collapsePenalty, _dayStart, _dayEnd, _minutesPerSecond,
                _dayMinutes, _moneyFormat, _energyWord,
                Flag(_pauseInModals), Flag(_energyEnabled), Flag(_skillsEnabled), Flag(_showCredit), .. SkillLevels.Select(level => level.Xp),
                .. SkillNames.Select(row => row.Name),
                .. Seasons.SelectMany(row => new[] { row.Id, row.Name, row.Days, row.Temperature, row.Music }),
                .. Festivals.SelectMany(row => new[] { row.Id, row.Name, row.SeasonId, row.Day, row.Music }),
            ],
            SettingsSection.Weather =>
            [
                .. WeatherCells().OrderBy(cell => (cell.SeasonId, cell.WeatherId)).Select(cell => cell.Weight),
                .. Overlays.SelectMany(row => new[] { row.Id, row.Style, row.Color, row.Density, row.Speed, row.Tint }),
                .. WeatherLooks.SelectMany(row => new[] { row.WeatherId, row.Overlay, row.Music }),
            ],
            SettingsSection.Atmosphere => Looks.SelectMany(row => new[] { row.SeasonId ?? "", row.Daylight, row.FoliageTint }),
            SettingsSection.Mine => [_mineSceneId ?? "", _mineX, _mineY, _mineFloors, _mineLadder],
            _ =>
            [
                _exportTitle, _exportExecutable, _exportVersion, _exportAuthor, _exportCompany, _exportIcon ?? "", _exportWidth, _exportHeight,
                Flag(_exportFullscreen), _exportPixelScale ?? "", _exportCredits,
            ],
        };
        var keys = section switch
        {
            SettingsSection.Project => $"{SkillLevels.Count}/{Seasons.Count}/{Festivals.Count}",
            SettingsSection.Weather => string.Join(",", WeatherCells().Select(cell => (cell.SeasonId, cell.WeatherId)).Order()) + $"/{Overlays.Count}",
            _ => "",
        };
        return keys + "\u001f" + string.Join("\u001f", fields);
    }

    private IEnumerable<WeatherCellDraft> WeatherCells() => _weather.SelectMany(row => row.Cells);

    /// <summary>Fills a section from the project and remembers it as loaded.</summary>
    private void Load(SettingsSection section)
    {
        if (_workspace.Current is not { } project) return;
        switch (section)
        {
            case SettingsSection.Project: LoadProject(project); break;
            case SettingsSection.Weather: LoadWeather(project); break;
            case SettingsSection.Atmosphere: LoadAtmosphere(project); break;
            case SettingsSection.Mine: LoadMine(project); break;
            default: LoadExport(project); break;
        }

        _loaded[section] = Snapshot(section);
    }

    private void OnProjectChanged(object? sender, ProjectChangedEventArgs e)
    {
        // Another project: nothing typed belongs to it.
        if (e.Kind == ProjectChangeKind.Opened)
        {
            _loaded.Clear();
        }

        if (_isShown()) Refresh();
    }

    // ---- Numbers ----

    private static string Number(double value) => DisplayFormat.Number(value);

    /// <summary>The number typed into the field called <paramref name="label"/>; the error names the field (#46).</summary>
    private static double Parse(string text, string label)
    {
        text = text.Trim();
        if (text.Length == 0) throw new FormatException($"{label} needs a value.");
        if (!double.TryParse(text, NumberStyles.Float, CultureInfo.InvariantCulture, out var value) || !double.IsFinite(value))
            throw new FormatException($"{label} must be a number.");
        return value;
    }

    /// <summary>A whole number, named in the error like <see cref="Parse"/>.</summary>
    private static double Whole(string text, string label)
    {
        var value = Parse(text, label);
        if (value != Math.Floor(value)) throw new FormatException($"{label} must be a whole number.");
        return value;
    }

    private static double Positive(string text, string label)
    {
        var value = Parse(text, label);
        return value > 0 ? value : throw new FormatException($"{label} must be more than 0.");
    }

    private static double AtLeastZero(string text, string label)
    {
        var value = Parse(text, label);
        return value >= 0 ? value : throw new FormatException($"{label} must be 0 or more.");
    }

    private static double Between(string text, string label, double low, double high)
    {
        var value = Parse(text, label);
        return value >= low && value <= high ? value : throw new FormatException($"{label} must be from {Number(low)} to {Number(high)}.");
    }

    private static int WholeNumber(string text, string label) =>
        int.TryParse(text.Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out var value)
            ? value
            : throw new FormatException($"{label} must be a whole number.");

    private static string? Optional(string text) => string.IsNullOrWhiteSpace(text) ? null : text.Trim();

    // ---- Project section ----

    private void LoadProject(GameProject project)
    {
        var settings = project.Settings;
        Name = project.Name;
        Version = project.Version;
        Locale = settings.Locale;
        PlayerSpeed = Number(settings.Movement.PlayerSpeed);
        MaxEnergy = Number(settings.MaxEnergy);
        CollapseFraction = Number(settings.CollapseEnergyFraction);
        CollapsePenalty = Number(settings.CollapseMoneyPenalty);
        DayStart = Number(settings.Time.DayStartMinute);
        DayEnd = Number(settings.Time.DayEndMinute);
        MinutesPerSecond = Number(settings.Time.MinutesPerRealSecond);
        DayMinutes = settings.Time.DayMinutes.OrNullable() is { } dayMinutes ? Number(dayMinutes) : "";
        PauseInModals = settings.Time.PauseInModals.OrNullable() != false;
        var words = settings.Words.OrNull();
        MoneyFormat = words?.Money.OrNull() ?? "";
        EnergyWord = words?.Energy.OrNull() ?? "";
        SkillNames.Clear();
        foreach (var skill in ContentForms.Options("skill", project))
        {
            var name = words?.Skills.FirstOrDefault(entry => entry.Item1 == skill.Id)?.Item2 ?? "";
            SkillNames.Add(new SkillNameDraft(skill.Id, name));
        }

        ShowSkillLevels(settings.SkillLevelCurve.Select(Number));
        EnergyEnabled = settings.EnergyEnabled;
        SkillsEnabled = settings.SkillsEnabled;
        ShowCredit = settings.ShowMadeWithCredit;
        Seasons.Clear();
        foreach (var season in settings.Calendar.Seasons)
        {
            var temperature = season.Temperature.OrNullable() is { } warmth ? Number(warmth) : "";
            Seasons.Add(new SeasonDraft(season.Id, season.Name, Number(season.Days), saved: true, temperature, season.Music.OrNull() ?? ""));
        }

        _loadedSeasons = settings.Calendar.Seasons.Select(season => season.Id).ToList();
        Festivals.Clear();
        foreach (var festival in settings.Calendar.Festivals) Festivals.Add(FestivalRow(festival));
        Message = "";
    }

    private static FestivalDraft FestivalRow(CalendarFestival festival) =>
        new(festival.Id, festival.Name, festival.SeasonId, Number(festival.Day), festival.Music.OrNull() ?? "");

    private void ShowSkillLevels(IEnumerable<string> values)
    {
        SkillLevels.Clear();
        foreach (var value in values) SkillLevels.Add(new SkillLevelDraft(value));
    }

    /// <summary>Drops level <paramref name="index"/>; the levels after it move up one.</summary>
    public void RemoveSkillLevel(int index)
    {
        if (index >= 0 && index < SkillLevels.Count) SkillLevels.RemoveAt(index);
    }

    /// <summary>A new last level: as far above the last as the last is above the one before, else 100 XP more.</summary>
    private void AddSkillLevel()
    {
        static double Value(SkillLevelDraft level) =>
            double.TryParse(level.Xp, NumberStyles.Float, CultureInfo.InvariantCulture, out var value) && double.IsFinite(value) ? value : 0;
        var count = SkillLevels.Count;
        var last = count > 0 ? Value(SkillLevels[^1]) : 0;
        var step = count > 1 ? last - Value(SkillLevels[^2]) : 0;
        SkillLevels.Add(new SkillLevelDraft(Number(count == 0 ? 0 : last + (step > 0 ? step : 100))));
    }

    /// <summary>The skill level rows as a curve: at least one level, each a finite XP amount of 0 or more and at least the one before.</summary>
    private List<double> SkillLevelCurve()
    {
        if (SkillLevels.Count == 0) throw new FormatException("Add at least one skill level.");
        var curve = new List<double>();
        for (var i = 0; i < SkillLevels.Count; i++)
        {
            if (!double.TryParse(SkillLevels[i].Xp, NumberStyles.Float, CultureInfo.InvariantCulture, out var xp) || !double.IsFinite(xp) || xp < 0)
                throw new FormatException($"Level {i} needs an XP amount of 0 or more.");
            if (curve.Count > 0 && xp < curve[^1]) throw new FormatException("Each level needs at least as much XP as the one before.");
            curve.Add(xp);
        }

        return curve;
    }

    /// <summary>Removes a season row (and the festival rows on it, as saving would); saved with the project settings (#70).</summary>
    public void RemoveSeason(SeasonDraft season)
    {
        ArgumentNullException.ThrowIfNull(season);
        if (!Seasons.Contains(season)) return;
        if (Seasons.Count <= 1)
        {
            Message = "The calendar needs at least one season.";
            return;
        }

        Seasons.Remove(season);
        foreach (var festival in Festivals.Where(f => f.SeasonId.Trim() == season.Id.Trim()).ToList()) Festivals.Remove(festival);
        Message = $"Removed {season.What} from the calendar. Save project settings to apply it.";
    }

    /// <summary>Moves a season row earlier (<paramref name="delta"/> −1) or later (+1) in the year.</summary>
    public void MoveSeason(SeasonDraft season, int delta)
    {
        var index = Seasons.IndexOf(season);
        var target = index + delta;
        if (index < 0 || target < 0 || target >= Seasons.Count) return;
        Seasons.Move(index, target);
        Message = "Season order changed. Save project settings to apply it.";
    }

    /// <summary>The first season can't move earlier, the last can't move later (web ProjectSettingsEditor).</summary>
    public bool CanMoveSeason(SeasonDraft season, int delta)
    {
        var target = Seasons.IndexOf(season) + delta;
        return Seasons.Contains(season) && target >= 0 && target < Seasons.Count;
    }

    public void RemoveFestival(FestivalDraft festival) => Festivals.Remove(festival);

    private void AddSeason()
    {
        if (_workspace.Current is null) return;
        var season = Defaults.NewSeason(CalendarConfig.Default.WithSeasons(DraftSeasons()));
        Seasons.Add(new SeasonDraft(season.Id, season.Name, Number(season.Days), saved: false));
    }

    private void AddFestival()
    {
        if (_workspace.Current is not { } project) return;
        var calendar = CalendarConfig.Default
            .WithSeasons(DraftSeasons())
            .WithFestivals(Festivals.Select(row => CalendarFestival.Default.WithId(row.Id)));
        var festival = Defaults.NewFestival(project, calendar);
        if (festival is not null) Festivals.Add(FestivalRow(festival));
    }

    /// <summary>The season rows as typed so far (unparseable day counts read as 28).</summary>
    private List<CalendarSeason> DraftSeasons() =>
        Seasons.Select(row => CalendarSeason.Default
            .WithId(row.Id)
            .WithName(row.Name)
            .WithDays(double.TryParse(row.Days, CultureInfo.InvariantCulture, out var days) ? days : 28)).ToList();

    private void Save()
    {
        if (_workspace.Current is not { } project) return;
        try
        {
            var seasons = Seasons.Select(row => SettingsForm.Season(row.Id.Trim(), row.Name.Trim(), Parse(row.Days, row.DaysLabel),
                row.Temperature.Trim().Length == 0 ? null : Parse(row.Temperature, row.TemperatureLabel), row.Music)).ToList();
            var festivals = Festivals.Select(row => SettingsForm.Festival(row.Id.Trim(), row.Name.Trim(), row.SeasonId.Trim(), Parse(row.Day, row.DayLabel), row.Music)).ToList();
            if (seasons.Count == 0) throw new FormatException("The calendar needs at least one season.");
            foreach (var season in seasons)
            {
                var label = season.Name.Length > 0 ? $"Season \"{season.Name}\"" : "A season";
                if (season.Id.Length == 0) throw new FormatException($"{label} needs an id.");
                if (season.Days <= 0 || season.Days != Math.Floor(season.Days)) throw new FormatException($"{label} days must be a whole number above 0.");
            }

            if (seasons.GroupBy(s => s.Id).FirstOrDefault(group => group.Count() > 1) is { } twice)
                throw new FormatException($"Two seasons have the id \"{twice.Key}\"; season ids must be different.");
            foreach (var festival in festivals)
            {
                var label = festival.Name.Length > 0 ? $"Festival \"{festival.Name}\"" : "A festival";
                if (festival.Id.Length == 0) throw new FormatException($"{label} needs an id.");
                if (seasons.FirstOrDefault(s => s.Id == festival.SeasonId) is not { } on) throw new FormatException($"{label} needs a season of the calendar.");
                if (festival.Day != Math.Floor(festival.Day) || festival.Day < 1 || festival.Day > on.Days)
                    throw new FormatException($"{label} day must be a whole number from 1 to {Number(on.Days)}.");
            }

            var curve = SkillLevelCurve();
            double? dayMinutes = _dayMinutes.Trim().Length == 0 ? null : Whole(_dayMinutes, DayMinutesLabel);
            if (dayMinutes < 60) throw new FormatException($"{DayMinutesLabel} must be at least 60.");
            var words = SettingsForm.Words(project, _moneyFormat, _energyWord, SkillNames.Select(row => (row.SkillId, row.Name)));
            var settings = SettingsForm.WithWords(project.Settings, words)
                .WithLocale(_locale.Trim())
                .WithEnergyEnabled(_energyEnabled)
                .WithSkillsEnabled(_skillsEnabled)
                .WithShowMadeWithCredit(_showCredit)
                .WithMaxEnergy(Positive(_maxEnergy, MaxEnergyLabel))
                .WithCollapseEnergyFraction(Between(_collapseFraction, CollapseFractionLabel, 0, 1))
                .WithCollapseMoneyPenalty(AtLeastZero(_collapsePenalty, CollapsePenaltyLabel))
                .WithSkillLevelCurve(curve)
                .WithMovement(project.Settings.Movement.WithPlayerSpeed(Positive(_playerSpeed, PlayerSpeedLabel)))
                .WithTime(SettingsForm.WithDayMinutes(project.Settings.Time, dayMinutes)
                    .WithDayStartMinute(Whole(_dayStart, DayStartLabel))
                    .WithDayEndMinute(Whole(_dayEnd, DayEndLabel))
                    .WithMinutesPerRealSecond(Positive(_minutesPerSecond, MinutesPerSecondLabel))
                    // Absent means on: unchecking writes false, checking clears an explicit false.
                    .WithPauseInModals(_pauseInModals ? (project.Settings.Time.PauseInModals.OrNullable() is null ? null : true) : false))
                .WithCalendar(project.Settings.Calendar.WithSeasons(seasons).WithFestivals(festivals));
            var name = _name.Trim();
            var version = _version.Trim();
            if (name.Length == 0) throw new FormatException($"{NameLabel} needs a value.");
            if (version.Length == 0) throw new FormatException($"{VersionLabel} needs a value.");
            // An end at or before the start collapsed the player on every tick.
            if (!SettingsSchema.validTime(settings.Time))
                throw new FormatException($"The day must end at least {SettingsSchema.MinDayWindowMinutes} minutes after it starts and by minute {SettingsSchema.MaxDayEndMinute}, and the clock can run at most {SettingsSchema.MaxMinutesPerRealSecond} minutes per second.");
            // Faster than this the player is hard to control (Problems warns about it too).
            if (settings.Movement.PlayerSpeed > MaxPlayerSpeed)
                throw new FormatException($"Player speed can be at most {MaxPlayerSpeed} tiles a second.");
            // Seasons removed from the rows go through RemoveSeason first, which also clears them
            // from weather, crops, shops, quests, fish and the sky (Cleanup.dropSeason); the
            // settings saved after it must not bring their sky looks back.
            var removedIds = _loadedSeasons.Where(id => !seasons.Any(season => season.Id == id)).ToList();
            settings = SettingsForm.WithoutSeasonLooks(settings, removedIds);
            _workspace.Apply(Edits.Batch("Project settings", [.. removedIds.Select(Edits.RemoveSeason), Edits.SetProjectInfo(name, version), Edits.SetSettings(settings)]));
            Load(SettingsSection.Project);
            Message = _workspace.SavedText("Settings");
        }
        catch (Exception error) when (error is FormatException or OverflowException)
        {
            Message = $"Could not save: {error.Message}";
        }
    }

    // ---- Weather section ----

    private void LoadWeather(GameProject project)
    {
        WeatherMessage = project.Weather.Types.Length == 0 ? "This project has no weather types." : "";
        Weather = [.. project.Settings.Calendar.Seasons.Select(season => new WeatherSeasonRow(season.Name, [.. project.Weather.Types.Select(type =>
            new WeatherCellDraft(season.Id, type.Id, type.Name, season.Name, Number(SettingsForm.WeatherWeight(project, season.Id, type.Id))))]))];
        Overlays.Clear();
        foreach (var overlay in project.Weather.Overlays.OrEmpty())
        {
            Overlays.Add(new OverlayDraft(overlay.Id, overlay.Style, overlay.Color, Number(overlay.Density), Number(overlay.Speed), overlay.Tint.OrNull() ?? ""));
        }

        WeatherLooks.Clear();
        foreach (var type in project.Weather.Types)
        {
            WeatherLooks.Add(new WeatherLookDraft(type.Id, type.Name, type.Overlay.OrNull() ?? "", type.Music.OrNull() ?? ""));
        }
    }

    /// <summary>A new overlay row: drifting dust, with an id no other row or overlay has.</summary>
    private void AddOverlay()
    {
        var taken = Overlays.Select(row => row.Id.Trim()).Concat(SettingsForm.OverlayStyles);
        Overlays.Add(new OverlayDraft(Defaults.NextId("overlay", taken), "dust", "#c8b090", "1", "1", ""));
    }

    /// <summary>Drops an overlay row; saved with the weather. Weather types that showed it then show none of the game's.</summary>
    public void RemoveOverlay(OverlayDraft overlay) => Overlays.Remove(overlay);

    private void SaveWeather()
    {
        if (_workspace.Current is not { } project) return;
        try
        {
            var weights = WeatherCells()
                .Select(cell => (cell.SeasonId, cell.WeatherId, string.IsNullOrWhiteSpace(cell.Weight) ? 0 : Parse(cell.Weight, cell.Label)))
                .ToList();
            var overlays = Overlays.Select(row => SettingsForm.Overlay(row.Id, row.Style, row.Color,
                Parse(row.Density, row.DensityLabel), Parse(row.Speed, row.SpeedLabel), row.Tint)).ToList();
            var looks = WeatherLooks.Select(row => (row.WeatherId, row.Overlay, row.Music)).ToList();
            // Looks first: they keep the odds table, which the weights then change.
            var saved = _workspace.Apply(Edits.Batch("Weather", [SettingsForm.WeatherLooks(project, overlays, looks), SettingsForm.WeatherOdds(project, weights)]));
            Load(SettingsSection.Weather);
            WeatherMessage = saved ? _workspace.SavedText("Weather") : "No changes were made.";
        }
        catch (FormatException error)
        {
            WeatherMessage = $"Could not save: {error.Message}";
        }
    }

    // ---- Atmosphere section ----

    private void LoadAtmosphere(GameProject project)
    {
        var atmosphere = project.Settings.Atmosphere.OrNull();
        LookDraft Row(string? seasonId, string what, AtmosphereLook? look) =>
            new(seasonId, what, look is null ? "" : SettingsForm.DaylightText(look.Daylight), look?.FoliageTint.OrNull() ?? "");
        Looks.Clear();
        Looks.Add(Row(null, "All seasons", atmosphere?.AllSeasons));
        foreach (var season in project.Settings.Calendar.Seasons)
        {
            Looks.Add(Row(season.Id, string.IsNullOrWhiteSpace(season.Name) ? season.Id : season.Name,
                atmosphere?.Seasons.FirstOrDefault(entry => entry.Item1 == season.Id)?.Item2));
        }

        AtmosphereMessage = "";
    }

    private void SaveAtmosphere()
    {
        if (_workspace.Current is not { } project) return;
        try
        {
            AtmosphereLook? all = null;
            var seasons = new List<(string, AtmosphereLook?)>();
            foreach (var row in Looks)
            {
                var look = SettingsForm.Look(row.What, row.Daylight, row.FoliageTint);
                if (row.SeasonId is { } seasonId) seasons.Add((seasonId, look));
                else all = look;
            }

            var saved = _workspace.Apply(SettingsForm.Atmosphere(project, all, seasons));
            Load(SettingsSection.Atmosphere);
            AtmosphereMessage = saved ? _workspace.SavedText("Atmosphere") : "No changes were made.";
        }
        catch (FormatException error)
        {
            AtmosphereMessage = $"Could not save: {error.Message}";
        }
    }

    // ---- People (M4) ----
    // Applied at once, like the mine toggle: the switches take effect when clicked, and the
    // relationship rules have their own Save.

    private string _peopleMessage = "";

    /// <summary>Raised when the project's People settings may have changed (the view rebuilds its form).</summary>
    public event Action? PeopleChanged;

    public string PeopleMessage { get => _peopleMessage; private set => SetProperty(ref _peopleMessage, value); }

    /// <summary>The open project (the People form builds on it).</summary>
    public ProjectWorkspace Workspace => _workspace;

    /// <summary>Shows a message of the People section (the relationship form's errors).</summary>
    public void ReportPeople(string text) => PeopleMessage = text;

    /// <summary>New games open the character creator.</summary>
    public bool CharacterCreator => _workspace.Current is { } project && project.Settings.CharacterCreator.Or(false);

    /// <summary>The game's relationship rules, or null when it has none.</summary>
    public RelationshipSettings? Relationships => _workspace.Current?.Settings.Relationships.OrNull();

    /// <summary>Turns the character creator on or off.</summary>
    public void SetCharacterCreator(bool on) => ApplyPeople(on, Relationships, "Character creator");

    /// <summary>Turns the relationship rules on (from the defaults) or off.</summary>
    public void SetRelationshipsOn(bool on) =>
        ApplyPeople(CharacterCreator, on ? Relationships ?? SettingsForm.DefaultRelationships : null, "Relationships");

    /// <summary>Saves the relationship rules as the form holds them.</summary>
    public bool SaveRelationships(RelationshipSettings relationships) => ApplyPeople(CharacterCreator, relationships, "Relationships");

    private bool ApplyPeople(bool characterCreator, RelationshipSettings? relationships, string what)
    {
        if (_workspace.Current is not { } project) return false;
        var saved = _workspace.Apply(SettingsForm.People(project, characterCreator, relationships));
        PeopleMessage = saved ? _workspace.SavedText(what) : "No changes were made.";
        PeopleChanged?.Invoke();
        return saved;
    }

    // ---- Mine section ----

    private void LoadMine(GameProject project)
    {
        var mine = project.Mine;
        var sceneId = mine.EntranceSceneId.OrNull() ?? project.StartSceneId;
        MineScenes = [.. project.Scenes.Select(scene => new SceneChoice(scene.Id, scene.Name))];
        MineSceneId = _mineScenes.Any(scene => scene.Id == sceneId) ? sceneId : null;
        MineX = Number(mine.EntranceX.Or(1));
        MineY = Number(mine.EntranceY.Or(1));
        MineFloors = Number(mine.Floors);
        MineLadder = Number(mine.LadderChance);
        MineNote = $"Players use the entrance tile to go down; an elevator checkpoint every {Number(mine.ElevatorEvery)} floors.";
        MineMessage = "";
    }

    /// <summary>Turns the mine on or off at once (its settings are kept).</summary>
    public void SetMineEnabled(bool enabled)
    {
        if (_workspace.Current is not { } project) return;
        _workspace.Apply(Edits.SetMine(Defaults.MineEnabled(project, enabled)));
        MineMessage = enabled ? "Mine enabled." : "Mine disabled; its settings are kept.";
    }

    /// <summary>The mine entrance clicked on the map, in the entrance scene picked here.</summary>
    private void PickMineEntrance()
    {
        if (_pickOnMap is null || _mineSceneId is not { } sceneId) return;
        _pickOnMap(sceneId, "Click the tile players use to go down into the mine.", (x, y) =>
        {
            MineX = x.ToString(CultureInfo.InvariantCulture);
            MineY = y.ToString(CultureInfo.InvariantCulture);
            MineMessage = $"Entrance set to ({x}, {y}). Save mine to keep it.";
        });
    }

    private void SaveMine()
    {
        if (_workspace.Current is not { } project) return;
        try
        {
            var mine = SettingsForm.Mine(project, true, _mineSceneId ?? "", Whole(_mineX, MineXLabel), Whole(_mineY, MineYLabel),
                Whole(_mineFloors, MineFloorsLabel), Parse(_mineLadder, MineLadderLabel));
            _workspace.Apply(Edits.SetMine(mine));
            Load(SettingsSection.Mine);
            MineMessage = _workspace.SavedText("Mine");
        }
        catch (FormatException error)
        {
            MineMessage = $"Could not save: {error.Message}";
        }
    }

    // ---- Export section ----

    /// <summary>The choice of <paramref name="options"/> that is <paramref name="current"/>, or null when none is.</summary>
    private static string? Matching(IReadOnlyList<PickerOption> options, string? current) =>
        options.FirstOrDefault(option => option.Id == (current ?? ""))?.Id;

    private void LoadExport(GameProject project)
    {
        var export = ExportSettingsForm.Current(project);
        ExportTitle = export.Title.OrNull() ?? "";
        ExportExecutable = export.ExecutableName.OrNull() ?? "";
        ExportVersion = export.Version.OrNull() ?? "";
        ExportAuthor = export.Author.OrNull() ?? "";
        ExportCompany = export.Company.OrNull() ?? "";
        ExportGameId = export.GameId;
        IconOptions = ExportSettingsForm.IconOptions(project, export.IconAssetId.OrNull());
        ExportIcon = Matching(_iconOptions, export.IconAssetId.OrNull());
        ExportWidth = export.Window.Width.ToString(CultureInfo.InvariantCulture);
        ExportHeight = export.Window.Height.ToString(CultureInfo.InvariantCulture);
        ExportFullscreen = export.Window.Fullscreen;
        PixelScaleOptions = ExportSettingsForm.PixelScales;
        ExportPixelScale = Matching(_pixelScaleOptions, export.PixelScale);
        ExportCredits = export.Credits.OrNull() ?? "";
        ExportMessage = !project.Export.HasValue() ? "Not saved yet: export uses these defaults." : "";
        ExportProblems = !project.Export.HasValue() ? [] : ExportSettingsForm.Check(project, export);
    }

    private void SaveExport()
    {
        if (_workspace.Current is not { } project) return;
        try
        {
            var current = ExportSettingsForm.Current(project);
            var settings = current
                .WithTitle(Optional(_exportTitle))
                .WithExecutableName(Optional(_exportExecutable))
                .WithVersion(Optional(_exportVersion))
                .WithAuthor(Optional(_exportAuthor))
                .WithCompany(Optional(_exportCompany))
                .WithIconAssetId(_exportIcon is { Length: > 0 } icon ? icon : null)
                .WithWindow(current.Window
                    .WithWidth(WholeNumber(_exportWidth, WindowWidthLabel))
                    .WithHeight(WholeNumber(_exportHeight, WindowHeightLabel))
                    .WithFullscreen(_exportFullscreen))
                .WithPixelScale(_exportPixelScale ?? current.PixelScale)
                .WithCredits(Optional(_exportCredits));
            var problems = ExportSettingsForm.Check(project, settings);
            _workspace.Apply(Edits.SetExportSettings(settings));
            Load(SettingsSection.Export);
            ExportProblems = problems;
            ExportMessage = problems.Any(problem => problem.IsError)
                ? _workspace.SavedText("Export settings") + " Fix the errors above before exporting."
                : _workspace.SavedText("Export settings");
        }
        catch (FormatException error)
        {
            ExportMessage = $"Could not save: {error.Message}";
        }
    }
}
