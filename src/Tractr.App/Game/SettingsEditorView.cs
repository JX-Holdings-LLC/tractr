using System.ComponentModel;
using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Data;
using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using Tractr.App.Localization;
using Tractr.App.Projects;
using Tractr.App.ViewModels.EditMode;

namespace Tractr.App.Game;

/// <summary>
/// The Settings tab: shows and collects the sections of its <see cref="SettingsViewModel"/>
/// (project, words and calendar, the weather, the atmosphere, the mine, export settings).
/// </summary>
public sealed class SettingsEditorView : UserControl, IRetirable
{
    private readonly StackPanel _seasons = new() { Spacing = 5 };
    private readonly StackPanel _weekdays = new() { Name = "Weekdays", Spacing = 5 };
    private readonly StackPanel _festivals = new() { Spacing = 5 };
    private readonly StackPanel _skillLevels = new() { Name = "SkillLevels", Spacing = 5 };
    private readonly StackPanel _weather = new() { Name = "WeatherOdds", Spacing = 6 };
    private readonly StackPanel _skillNames = new() { Name = "SkillNames", Spacing = 5 };
    private readonly StackPanel _overlays = new() { Name = "WeatherOverlays", Spacing = 5 };
    /// <summary>Each weather type's overlay picker, refilled as the overlay rows change.</summary>
    private readonly Dictionary<WeatherLookDraft, ComboBox> _overlayPickers = [];
    private readonly StackPanel _weatherLooks = new() { Name = "WeatherLooks", Spacing = 5 };
    private readonly StackPanel _looks = new() { Name = "AtmosphereLooks", Spacing = 8 };
    private readonly StackPanel _exportProblems = new() { Name = "ExportSettingsProblems", Spacing = 4 };
    private readonly StackPanel _mineFields = new() { Name = "MineFields", Spacing = 6 };
    private readonly CheckBox _mineEnabled = new() { Name = "Mine_Enabled", Content = "Mine enabled" };
    private readonly ComboBox _mineScene = new() { Name = "Mine_EntranceScene", MinWidth = 220 };
    private readonly ComboBox _exportIcon = new() { Name = "Export_Icon", MinWidth = 260 };
    private readonly ComboBox _exportPixelScale = new() { Name = "Export_PixelScale", MinWidth = 260 };
    private readonly CheckBox _characterCreator = new() { Name = "Setting_CharacterCreator", Content = "New games open the character creator (name, farm name, colours)" };
    private readonly CheckBox _relationshipsOn = new() { Name = "Setting_Relationships", Content = "Relationships: friendship decay, dating, marriage, children" };
    private readonly StackPanel _relationships = new() { Name = "RelationshipSettings", Spacing = 6 };
    private ContentForm? _relationshipForm;
    private readonly CheckBox _farmOn = new() { Name = "Setting_Farm" };
    private readonly StackPanel _farm = new() { Name = "FarmSettings", Spacing = 6 };
    private ContentForm? _farmForm;
    private readonly CheckBox _combatOn = new() { Name = "Setting_Combat" };
    private readonly StackPanel _combat = new() { Name = "CombatSettings", Spacing = 6 };
    private ContentForm? _combatForm;
    private ContentForm? _progressionForm;
    private readonly StackPanel _progression = new() { Name = "ProgressionSettings", Spacing = 6 };
    private readonly StackPanel _mineBands = new() { Name = "MineBands", Spacing = 8 };
    private readonly List<ContentForm> _mineBandForms = [];
    private readonly DraftRows<SeasonDraft> _seasonRows;
    private readonly Dictionary<SeasonDraft, (Button Up, Button Down)> _seasonArrows = [];
    private bool _filling;

    /// <param name="workspace">The open project.</param>
    /// <param name="pickOnMap">Lets the creator click the mine entrance on the map (scene id, prompt, what to do with the tile); null hides the button.</param>
    public SettingsEditorView(ProjectWorkspace workspace, Action<string, string, Action<int, int>>? pickOnMap = null)
    {
        ViewModel = new SettingsViewModel(workspace, pickOnMap, () => IsEffectivelyVisible);
        DataContext = ViewModel;
        Name = "SettingsEditorView";
        var message = Shown("SettingsMessage", nameof(SettingsViewModel.Message));
        var form = new StackPanel { Spacing = 10, Margin = new Thickness(20), MaxWidth = 760 };
        form.Children.Add(Ui.Text("PROJECT", "section"));
        Field(form, SettingsViewModel.NameLabel, "Setting_Name", nameof(SettingsViewModel.Name));
        Field(form, SettingsViewModel.VersionLabel, "Setting_Version", nameof(SettingsViewModel.Version));
        Field(form, SettingsViewModel.LocaleLabel, "Setting_Locale", nameof(SettingsViewModel.Locale));
        form.Children.Add(Ui.Text("GAMEPLAY", "section"));
        form.Children.Add(Ui.HStack(15,
            Check("Setting_EnergyEnabled", "Energy enabled", nameof(SettingsViewModel.EnergyEnabled)),
            Check("Setting_SkillsEnabled", "Skills enabled", nameof(SettingsViewModel.SkillsEnabled)),
            Check("Setting_ShowMadeWithCredit", "Show creator credit", nameof(SettingsViewModel.ShowCredit))));
        Field(form, SettingsViewModel.PlayerSpeedLabel, "Setting_PlayerSpeed", nameof(SettingsViewModel.PlayerSpeed));
        Field(form, SettingsViewModel.MaxEnergyLabel, "Setting_MaxEnergy", nameof(SettingsViewModel.MaxEnergy));
        Field(form, SettingsViewModel.CollapseFractionLabel, "Setting_CollapseEnergyFraction", nameof(SettingsViewModel.CollapseFraction));
        Field(form, SettingsViewModel.CollapsePenaltyLabel, "Setting_CollapseMoneyPenalty", nameof(SettingsViewModel.CollapsePenalty));
        form.Children.Add(Ui.Text("Skill levels", "muted", "small"));
        form.Children.Add(Ui.Wrapped("The total XP a skill needs to reach each level. Players start at level 0; the game announces \"Farming level 1!\" once farming XP reaches level 1's amount.", "muted", "small"));
        form.Children.Add(_skillLevels);
        var addSkillLevel = Ui.CommandButton("Add level", ViewModel.AddSkillLevelCommand, "tool");
        addSkillLevel.Name = "AddSkillLevelButton";
        form.Children.Add(addSkillLevel);
        form.Children.Add(Ui.Text(EditorStrings.Get("settings.words"), "section"));
        form.Children.Add(Ui.Wrapped(EditorStrings.Get("settings.wordsHelp"), "muted", "small"));
        Field(form, SettingsViewModel.MoneyFormatLabel + " ({0} is the amount)", "Setting_MoneyFormat", nameof(SettingsViewModel.MoneyFormat)).PlaceholderText = "${0}";
        Field(form, SettingsViewModel.EnergyWordLabel, "Setting_EnergyWord", nameof(SettingsViewModel.EnergyWord)).PlaceholderText = "Energy";
        form.Children.Add(Ui.Text(EditorStrings.Get("settings.skillNames"), "muted", "small"));
        form.Children.Add(_skillNames);
        form.Children.Add(Ui.Text("TIME", "section"));
        Field(form, SettingsViewModel.DayStartLabel, "Setting_DayStartMinute", nameof(SettingsViewModel.DayStart));
        Field(form, SettingsViewModel.DayEndLabel, "Setting_DayEndMinute", nameof(SettingsViewModel.DayEnd));
        Field(form, SettingsViewModel.MinutesPerSecondLabel, "Setting_MinutesPerRealSecond", nameof(SettingsViewModel.MinutesPerSecond));
        Field(form, SettingsViewModel.DayMinutesLabel, "Setting_DayMinutes", nameof(SettingsViewModel.DayMinutes)).PlaceholderText = "1440";
        form.Children.Add(Check("Setting_PauseInModals", "Pause the clock in dialogue, shops, minigames and menus", nameof(SettingsViewModel.PauseInModals)));
        form.Children.Add(Ui.Text("SEASONS", "section"));
        form.Children.Add(Ui.Wrapped(EditorStrings.Get("settings.seasonsHelp"), "muted", "small"));
        form.Children.Add(_seasons);
        var addSeason = Ui.CommandButton("Add season", ViewModel.AddSeasonCommand, "tool");
        addSeason.Name = "AddSeasonButton";
        form.Children.Add(addSeason);
        form.Children.Add(Ui.Text("WEEKDAYS", "section"));
        form.Children.Add(Ui.Wrapped("Days repeat across seasons. An empty list disables weekdays. The translated name uses the project language above.", "muted", "small"));
        form.Children.Add(_weekdays);
        form.Children.Add(Ui.HStack(6, Ui.Button("Add weekday", ViewModel.AddWeekday, "tool"), Ui.Button("Classic week", ViewModel.ResetWeekdays, "tool")));
        Field(form, "Weekday offset (0 starts on the first day)", "WeekdayOffset", nameof(SettingsViewModel.WeekdayOffset));
        form.Children.Add(Ui.Text("FESTIVALS", "section"));
        form.Children.Add(_festivals);
        var addFestival = Ui.CommandButton("Add festival", ViewModel.AddFestivalCommand, "tool");
        addFestival.Name = "AddFestivalButton";
        form.Children.Add(addFestival);
        form.Children.Add(message);
        var save = Ui.CommandButton("Save project settings", ViewModel.SaveCommand, "accent");
        save.Name = "SaveSettingsButton";
        var revert = Ui.CommandButton("Revert fields", ViewModel.RevertCommand, "tool");
        revert.Name = "RevertSettingsButton";
        form.Children.Add(Ui.HStack(8, save, revert));
        form.Children.Add(Ui.Text("WEATHER ODDS PER SEASON", "section"));
        form.Children.Add(Ui.Wrapped("Weights, rolled at each day start. Rain waters the soil, storms can damage crops. 0 means never.", "muted", "small"));
        form.Children.Add(_weather);
        form.Children.Add(Ui.Text(EditorStrings.Get("settings.overlays"), "section"));
        form.Children.Add(Ui.Wrapped(EditorStrings.Get("settings.overlaysHelp"), "muted", "small"));
        form.Children.Add(_overlays);
        var addOverlay = Ui.CommandButton(EditorStrings.Get("settings.addOverlay"), ViewModel.AddOverlayCommand, "tool");
        addOverlay.Name = "AddOverlayButton";
        form.Children.Add(addOverlay);
        form.Children.Add(Ui.Text(EditorStrings.Get("settings.weatherLooks"), "muted", "small"));
        form.Children.Add(_weatherLooks);
        form.Children.Add(Shown("WeatherMessage", nameof(SettingsViewModel.WeatherMessage)));
        var saveWeather = Ui.CommandButton("Save weather", ViewModel.SaveWeatherCommand, "accent");
        saveWeather.Name = "SaveWeatherButton";
        form.Children.Add(saveWeather);
        form.Children.Add(Ui.Text(EditorStrings.Get("settings.sky"), "section"));
        form.Children.Add(Ui.Wrapped(EditorStrings.Get("settings.skyHelp"), "muted", "small"));
        form.Children.Add(_looks);
        form.Children.Add(Shown("AtmosphereMessage", nameof(SettingsViewModel.AtmosphereMessage)));
        var saveAtmosphere = Ui.CommandButton(EditorStrings.Get("settings.saveSky"), ViewModel.SaveAtmosphereCommand, "accent");
        saveAtmosphere.Name = "SaveAtmosphereButton";
        var revertAtmosphere = Ui.CommandButton(EditorStrings.Get("settings.revertSky"), ViewModel.RevertAtmosphereCommand, "tool");
        revertAtmosphere.Name = "RevertAtmosphereButton";
        form.Children.Add(Ui.HStack(8, saveAtmosphere, revertAtmosphere));
        form.Children.Add(Ui.Text("MINE", "section"));
        form.Children.Add(Ui.Wrapped("Procedural floors below an entrance tile; broken rocks reveal the ladder down.", "muted", "small"));
        _mineEnabled.Click += (_, _) => ViewModel.SetMineEnabled(_mineEnabled.IsChecked == true);
        form.Children.Add(_mineEnabled);
        var mineX = Box("Mine_EntranceX", nameof(SettingsViewModel.MineX), 70);
        var mineY = Box("Mine_EntranceY", nameof(SettingsViewModel.MineY), 70);
        var mineFloors = Box("Mine_Floors", nameof(SettingsViewModel.MineFloors), 70);
        var mineLadder = Box("Mine_LadderChance", nameof(SettingsViewModel.MineLadder), 70);
        var exportWidth = Box("Export_WindowWidth", nameof(SettingsViewModel.ExportWidth), 90);
        var exportHeight = Box("Export_WindowHeight", nameof(SettingsViewModel.ExportHeight), 90);
        Ui.Label((_mineScene, SettingsViewModel.MineSceneLabel), (mineX, SettingsViewModel.MineXLabel), (mineY, SettingsViewModel.MineYLabel),
            (mineFloors, SettingsViewModel.MineFloorsLabel), (mineLadder, SettingsViewModel.MineLadderLabel), (_exportIcon, "Icon (PNG artwork, at least 256×256)"),
            (exportWidth, SettingsViewModel.WindowWidthLabel), (exportHeight, SettingsViewModel.WindowHeightLabel), (_exportPixelScale, "Pixel scale"));
        _mineFields.Children.Add(Ui.HStack(8, Ui.Text("Entrance scene", "muted", "small"), _mineScene));
        var mineRow = Ui.HStack(8, Ui.Text("Entrance X", "muted", "small"), mineX, Ui.Text("Y", "muted", "small"), mineY);
        if (ViewModel.CanPickOnMap)
        {
            var pick = Ui.CommandButton("Pick on map", ViewModel.PickMineEntranceCommand, "tool", "small");
            pick.Name = "Mine_PickOnMap";
            AutomationProperties.SetName(pick, "Pick the mine entrance on the map");
            mineRow.Children.Add(pick);
        }

        _mineFields.Children.Add(mineRow);
        _mineFields.Children.Add(Ui.HStack(8, Ui.Text("Floors", "muted", "small"), mineFloors, Ui.Text("Ladder chance (0.02–1)", "muted", "small"), mineLadder));
        _mineFields.Children.Add(Shown("MineNote", nameof(SettingsViewModel.MineNote)));
        var saveMine = Ui.CommandButton("Save mine", ViewModel.SaveMineCommand, "accent");
        saveMine.Name = "SaveMineButton";
        _mineFields.Children.Add(saveMine);
        form.Children.Add(_mineFields);
        form.Children.Add(Shown("MineMessage", nameof(SettingsViewModel.MineMessage)));
        // The depth bands: rocks, and monsters, hazards and treasure (M7), one form per band.
        form.Children.Add(Ui.Text(EditorStrings.Get("settings.mineBands"), "muted", "small"));
        form.Children.Add(Ui.Wrapped(EditorStrings.Get("settings.mineBandsHelp"), "muted", "small"));
        form.Children.Add(_mineBands);
        var addBand = Ui.Button(EditorStrings.Get("settings.addMineBand"), AddMineBand, "tool");
        addBand.Name = "AddMineBandButton";
        var saveBands = Ui.Button(EditorStrings.Get("settings.saveMineBands"), SaveMineBands, "accent");
        saveBands.Name = "SaveMineBandsButton";
        form.Children.Add(Ui.HStack(8, addBand, saveBands));
        form.Children.Add(Shown("MineBandsMessage", nameof(SettingsViewModel.MineBandsMessage)));
        form.Children.Add(Ui.Text("PEOPLE", "section"));
        form.Children.Add(Ui.Wrapped("The player character and the game's relationship rules. Each part is off until it is turned on here; NPCs are romanceable one by one in their own forms.", "muted", "small"));
        _characterCreator.Click += (_, _) => ViewModel.SetCharacterCreator(_characterCreator.IsChecked == true);
        _relationshipsOn.Click += (_, _) => ViewModel.SetRelationshipsOn(_relationshipsOn.IsChecked == true);
        form.Children.Add(_characterCreator);
        form.Children.Add(_relationshipsOn);
        form.Children.Add(_relationships);
        form.Children.Add(Shown("PeopleMessage", nameof(SettingsViewModel.PeopleMessage)));
        form.Children.Add(Ui.Text(EditorStrings.Get("settings.farm"), "section"));
        form.Children.Add(Ui.Wrapped(EditorStrings.Get("settings.farmHelp"), "muted", "small"));
        _farmOn.Content = EditorStrings.Get("settings.farmOn");
        _farmOn.Click += (_, _) => ViewModel.SetFarmOn(_farmOn.IsChecked == true);
        form.Children.Add(_farmOn);
        form.Children.Add(_farm);
        form.Children.Add(Shown("FarmMessage", nameof(SettingsViewModel.FarmMessage)));
        form.Children.Add(Ui.Text(EditorStrings.Get("settings.combat"), "section"));
        form.Children.Add(Ui.Wrapped(EditorStrings.Get("settings.combatHelp"), "muted", "small"));
        _combatOn.Content = EditorStrings.Get("settings.combatOn");
        _combatOn.Click += (_, _) => ViewModel.SetCombatOn(_combatOn.IsChecked == true);
        form.Children.Add(_combatOn);
        form.Children.Add(_combat);
        form.Children.Add(Shown("CombatMessage", nameof(SettingsViewModel.CombatMessage)));
        form.Children.Add(Ui.Text("PLAYER PROGRESSION", "section"));
        form.Children.Add(Ui.Wrapped("Set permanent upgrade limits and skills, action XP, level rewards and perk choices. Empty skills retain the built-in rules.", "muted", "small"));
        form.Children.Add(_progression);
        form.Children.Add(Ui.Text("EXPORT", "section"));
        form.Children.Add(Ui.Wrapped("How File → Export Game names and packages the standalone game. Empty title and version use the project's.", "muted", "small"));
        Field(form, "Game title", "Export_Title", nameof(SettingsViewModel.ExportTitle)).PlaceholderText = "Project name";
        Field(form, "Executable name", "Export_ExecutableName", nameof(SettingsViewModel.ExportExecutable));
        Field(form, "Game version", "Export_Version", nameof(SettingsViewModel.ExportVersion)).PlaceholderText = "Project version";
        Field(form, "Author", "Export_Author", nameof(SettingsViewModel.ExportAuthor));
        Field(form, "Company", "Export_Company", nameof(SettingsViewModel.ExportCompany));
        var gameId = Field(form, "Game id (keeps save folders stable; set once)", "Export_GameId", nameof(SettingsViewModel.ExportGameId), BindingMode.OneWay);
        gameId.IsReadOnly = true;
        form.Children.Add(Ui.Text("Icon (PNG artwork, at least 256×256)", "muted", "small"));
        form.Children.Add(_exportIcon);
        form.Children.Add(Ui.HStack(8, Ui.Text("Window", "muted", "small"), exportWidth, Ui.Text("×"), exportHeight,
            Check("Export_Fullscreen", "Start fullscreen", nameof(SettingsViewModel.ExportFullscreen))));
        form.Children.Add(Ui.Text("Pixel scale", "muted", "small"));
        form.Children.Add(_exportPixelScale);
        var credits = Field(form, "Credits", "Export_Credits", nameof(SettingsViewModel.ExportCredits));
        credits.AcceptsReturn = true;
        credits.TextWrapping = Avalonia.Media.TextWrapping.Wrap;
        credits.MinHeight = 60;
        form.Children.Add(_exportProblems);
        form.Children.Add(Shown("ExportSettingsMessage", nameof(SettingsViewModel.ExportMessage)));
        var saveExport = Ui.CommandButton("Save export settings", ViewModel.SaveExportCommand, "accent");
        saveExport.Name = "SaveExportSettingsButton";
        form.Children.Add(saveExport);
        Content = new ScrollViewer { Content = form };

        _mineScene.SelectionChanged += (_, _) =>
        {
            if (!_filling) ViewModel.MineSceneId = (_mineScene.SelectedItem as ComboBoxItem)?.Tag as string;
        };
        _exportIcon.SelectionChanged += (_, _) =>
        {
            if (!_filling) ViewModel.ExportIcon = (_exportIcon.SelectedItem as ComboBoxItem)?.Tag as string;
        };
        _exportPixelScale.SelectionChanged += (_, _) =>
        {
            if (!_filling) ViewModel.ExportPixelScale = (_exportPixelScale.SelectedItem as ComboBoxItem)?.Tag as string;
        };
        _seasonRows = new DraftRows<SeasonDraft>(_seasons, ViewModel.Seasons, SeasonRow, UpdateSeasonArrows);
        UpdateSeasonArrows();
        _ = new DraftRows<WeekdayDraft>(_weekdays, ViewModel.Weekdays, WeekdayRow);
        _ = new DraftRows<FestivalDraft>(_festivals, ViewModel.Festivals, FestivalRow);
        _ = new DraftRows<SkillNameDraft>(_skillNames, ViewModel.SkillNames, SkillNameRow);
        _ = new DraftRows<OverlayDraft>(_overlays, ViewModel.Overlays, OverlayRow);
        _ = new DraftRows<WeatherLookDraft>(_weatherLooks, ViewModel.WeatherLooks, WeatherLookRow);
        ViewModel.OverlayChoicesChanged += (_, _) => RefreshOverlayPickers();
        _ = new DraftRows<LookDraft>(_looks, ViewModel.Looks, LookRow);
        ViewModel.SkillLevels.CollectionChanged += (_, _) => ShowSkillLevels();
        ShowSkillLevels();
        ShowWeather();
        ShowMine();
        ShowMineScenes();
        ShowIcons();
        ShowPixelScales();
        ShowExportProblems();
        ShowPeople();
        ViewModel.PeopleChanged += ShowPeople;
        ShowFarm();
        ViewModel.FarmChanged += ShowFarm;
        ShowCombat();
        ViewModel.CombatChanged += ShowCombat;
        ShowProgression();
        ViewModel.CombatChanged += ShowProgression;
        ViewModel.PropertyChanged += OnViewModelChanged;
    }

    /// <summary>The People section from the project: the two switches and, when relationships are on, their rules as a form with Save.</summary>
    private void ShowPeople()
    {
        _characterCreator.IsChecked = ViewModel.CharacterCreator;
        var relationships = ViewModel.Relationships;
        _relationshipsOn.IsChecked = relationships is not null;
        _relationships.Children.Clear();
        _relationshipForm = null;
        if (relationships is null || ViewModel.Workspace.Current is not { } project) return;
        var fields = new StackPanel { Spacing = 6 };
        _relationshipForm = new ContentForm(project, relationships, typeof(RelationshipSettings), fields, text => ViewModel.ReportPeople(text));
        _relationships.Children.Add(fields);
        var save = Ui.Button("Save relationships", SaveRelationships, "accent");
        save.Name = "SaveRelationshipsButton";
        _relationships.Children.Add(save);
    }

    private void SaveRelationships()
    {
        if (_relationshipForm is null) return;
        try
        {
            ViewModel.SaveRelationships((RelationshipSettings)_relationshipForm.Commit());
        }
        catch (Exception error) when (error is FormatException or System.Text.Json.JsonException)
        {
            ViewModel.ReportPeople($"Could not save: {error.Message}");
        }
    }

    /// <summary>The Farm section (M5): its switch and, when on, the farm scene, layouts and pests as a form with Save.</summary>
    private void ShowFarm()
    {
        var farm = ViewModel.Farm;
        _farmOn.IsChecked = farm is not null;
        _farm.Children.Clear();
        _farmForm = null;
        if (farm is null || ViewModel.Workspace.Current is not { } project) return;
        var fields = new StackPanel { Spacing = 6 };
        _farmForm = new ContentForm(project, farm, typeof(FarmSettings), fields, ViewModel.ReportFarm);
        _farm.Children.Add(fields);
        var save = Ui.Button(EditorStrings.Get("settings.saveFarm"), () => SaveRules<FarmSettings>(_farmForm, ViewModel.SaveFarm, ViewModel.ReportFarm), "accent");
        save.Name = "SaveFarmButton";
        _farm.Children.Add(save);
    }

    /// <summary>
    /// The Combat section (M7): its switch and, when on, the health and fainting rules as a form
    /// with Save; and the mine's depth bands, whose monsters come with combat.
    /// </summary>
    private void ShowProgression()
    {
        _progression.Children.Clear();
        _progressionForm = null;
        if (ViewModel.Workspace.Current is not { } project) return;
        var fields = new StackPanel { Spacing = 6 };
        _progressionForm = new ContentForm(project, SettingsForm.ProgressionDraft(project), typeof(ProgressionSettings), fields, ViewModel.ReportCombat);
        _progression.Children.Add(fields);
        var classic = Ui.Button("Classic skills", () => ViewModel.UseClassicSkills());
        classic.Name = "ClassicSkillsButton";
        _progression.Children.Add(classic);
        var save = Ui.Button("Save progression", () => SaveRules<ProgressionSettings>(_progressionForm, ViewModel.SaveProgression, ViewModel.ReportCombat), "accent");
        save.Name = "SaveProgressionButton";
        _progression.Children.Add(save);
    }

    private void ShowCombat()
    {
        var combat = ViewModel.Combat;
        _combatOn.IsChecked = combat is not null;
        _combat.Children.Clear();
        _combatForm = null;
        ShowMineBands();
        if (combat is null || ViewModel.Workspace.Current is not { } project) return;
        var fields = new StackPanel { Spacing = 6 };
        _combatForm = new ContentForm(project, combat, typeof(CombatSettings), fields, ViewModel.ReportCombat);
        _combat.Children.Add(fields);
        var save = Ui.Button(EditorStrings.Get("settings.saveCombat"), () => SaveRules<CombatSettings>(_combatForm, ViewModel.SaveCombat, ViewModel.ReportCombat), "accent");
        save.Name = "SaveCombatButton";
        _combat.Children.Add(save);
    }

    /// <summary>Commits a rules form and saves it, or says why it cannot be saved.</summary>
    private static void SaveRules<T>(ContentForm? form, Func<T, bool> save, Action<string> report)
        where T : class
    {
        if (form is null) return;
        try
        {
            save((T)form.Commit());
        }
        catch (Exception error) when (error is FormatException or System.Text.Json.JsonException)
        {
            report(EditorStrings.Format("settings.couldNotSave", error.Message));
        }
    }

    /// <summary>One form per depth band of the mine, as stored.</summary>
    private void ShowMineBands() => ShowMineBands(ViewModel.MineBands);

    private void ShowMineBands(IReadOnlyList<MineBand> bands)
    {
        _mineBands.Children.Clear();
        _mineBandForms.Clear();
        if (ViewModel.Workspace.Current is not { } project) return;
        for (var i = 0; i < bands.Count; i++)
        {
            var index = i;
            var fields = new StackPanel { Spacing = 6 };
            _mineBandForms.Add(new ContentForm(project, bands[i], typeof(MineBand), fields, ViewModel.ReportMineBands));
            var remove = Ui.Button(EditorStrings.Get("common.remove"), () => RemoveMineBand(index), "tool", "small");
            remove.Name = $"RemoveMineBand_{i}";
            var card = Ui.VStack(6, Ui.HStack(8, Ui.Text(EditorStrings.Format("settings.mineBand", i + 1), "muted", "small"), remove), fields);
            _mineBands.Children.Add(new Border { Name = $"MineBand_{i}", Child = card }.WithClasses("row"));
        }
    }

    /// <summary>The bands as their forms hold them, or null (with the reason shown) while one is invalid.</summary>
    private List<MineBand>? DraftMineBands()
    {
        try
        {
            return [.. _mineBandForms.Select(form => (MineBand)form.Commit())];
        }
        catch (Exception error) when (error is FormatException or System.Text.Json.JsonException)
        {
            ViewModel.ReportMineBands(EditorStrings.Format("settings.couldNotSave", error.Message));
            return null;
        }
    }

    /// <summary>A new band below the last one (unsaved until Save depth bands).</summary>
    private void AddMineBand()
    {
        if (DraftMineBands() is not { } bands) return;
        var from = bands.Count == 0 ? 1.0 : bands[^1].ToFloor + 1;
        bands.Add(MineBand.Default.WithFromFloor(from).WithToFloor(from + 9));
        ShowMineBands(bands);
    }

    private void RemoveMineBand(int index)
    {
        if (DraftMineBands() is not { } bands || index >= bands.Count) return;
        bands.RemoveAt(index);
        ShowMineBands(bands);
    }

    private void SaveMineBands()
    {
        if (DraftMineBands() is { } bands) ViewModel.SaveMineBands(bands);
    }

    public SettingsViewModel ViewModel { get; }

    /// <summary>Stops following the project (the editor that built this view was replaced).</summary>
    public void Retire() => ViewModel.Retire();

    public void Refresh() => ViewModel.Refresh();

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs e)
    {
        switch (e.PropertyName)
        {
            case nameof(SettingsViewModel.Weather): ShowWeather(); break;
            case nameof(SettingsViewModel.MineEnabled): ShowMine(); break;
            case nameof(SettingsViewModel.MineScenes) or nameof(SettingsViewModel.MineSceneId): ShowMineScenes(); break;
            case nameof(SettingsViewModel.IconOptions) or nameof(SettingsViewModel.ExportIcon): ShowIcons(); break;
            case nameof(SettingsViewModel.PixelScaleOptions) or nameof(SettingsViewModel.ExportPixelScale): ShowPixelScales(); break;
            case nameof(SettingsViewModel.ExportProblems): ShowExportProblems(); break;
        }
    }

    private static TextBlock Shown(string name, string property)
    {
        var block = Ui.Wrapped("", "muted", "small");
        block.Name = name;
        block.Bind(TextBlock.TextProperty, new Binding(property));
        return block;
    }

    private static TextBox Field(StackPanel form, string label, string name, string property, BindingMode mode = BindingMode.TwoWay)
    {
        var input = new TextBox { Name = name };
        input.Bind(TextBox.TextProperty, new Binding(property) { Mode = mode });
        Ui.Label((input, label));
        form.Children.Add(Ui.Text(label, "muted", "small"));
        form.Children.Add(input);
        return input;
    }

    private static TextBox Box(string name, string property, double width, object? source = null)
    {
        var box = new TextBox { Name = name, Width = width };
        var binding = new Binding(property) { Mode = BindingMode.TwoWay };
        if (source is not null) binding.Source = source;
        box.Bind(TextBox.TextProperty, binding);
        return box;
    }

    private static CheckBox Check(string name, string content, string property)
    {
        var check = new CheckBox { Name = name, Content = content };
        check.Bind(ToggleButton.IsCheckedProperty, new Binding(property) { Mode = BindingMode.TwoWay });
        return check;
    }

    /// <summary>
    /// One row per skill level: "Level 0 [0] XP", "Level 1 [50] XP", … (the curve's index is the
    /// level the game reports). Rebuilt whenever a level is added or removed, so the names follow
    /// the levels.
    /// </summary>
    private void ShowSkillLevels()
    {
        _skillLevels.Children.Clear();
        for (var index = 0; index < ViewModel.SkillLevels.Count; index++)
        {
            var i = index;
            var box = Box($"Setting_SkillLevel_{i}", nameof(SkillLevelDraft.Xp), 90, ViewModel.SkillLevels[i]);
            AutomationProperties.SetName(box, $"Level {i} XP");
            var remove = Ui.Button("Remove", () => ViewModel.RemoveSkillLevel(i), "tool", "small");
            remove.Name = $"Setting_SkillLevelRemove_{i}";
            AutomationProperties.SetName(remove, $"Remove level {i}");
            _skillLevels.Children.Add(Ui.HStack(8, new TextBlock { Text = $"Level {i}", Width = 70, VerticalAlignment = Avalonia.Layout.VerticalAlignment.Center },
                box, Ui.Text("XP", "muted", "small"), remove));
        }
    }

    private Control SeasonRow(SeasonDraft season)
    {
        var id = Box("Season_Id", nameof(SeasonDraft.Id), 115, season);
        id.IsReadOnly = season.IsSaved;
        var name = Box("Season_Name", nameof(SeasonDraft.Name), 145, season);
        var days = Box("Season_Days", nameof(SeasonDraft.Days), 65, season);
        var temperature = Box("Season_Temperature", nameof(SeasonDraft.Temperature), 60, season);
        temperature.PlaceholderText = EditorStrings.Get("settings.temperaturePlaceholder");
        ToolTip.SetTip(temperature, EditorStrings.Get("settings.seasonTemperatureTip"));
        var music = RowPicker("Season_Music", ViewModel.MusicChoices(season.Music), season.Music, id => season.Music = id, 150);
        Ui.Label((id, season.IdLabel), (name, season.NameLabel), (days, season.DaysLabel), (temperature, season.TemperatureLabel), (music, season.MusicLabel));
        // Removing and moving change the rows; Save project settings applies them (#70).
        var remove = Ui.Button("Remove", () => ViewModel.RemoveSeason(season), "tool", "small");
        remove.Name = $"Season_Remove_{season.Key}";
        Ui.Label((remove, $"Remove {season.What}"));
        var up = Ui.Button("↑", () => ViewModel.MoveSeason(season, -1), "tool", "small");
        up.Name = $"Season_Up_{season.Key}";
        ToolTip.SetTip(up, "Earlier in the year");
        var down = Ui.Button("↓", () => ViewModel.MoveSeason(season, 1), "tool", "small");
        down.Name = $"Season_Down_{season.Key}";
        ToolTip.SetTip(down, "Later in the year");
        Ui.Label((up, $"Move {season.What} earlier in the year"), (down, $"Move {season.What} later in the year"));
        _seasonArrows[season] = (up, down);
        return new WrapPanel { Children = { Spaced(up), Spaced(down), Spaced(id), Spaced(name), Spaced(days), Spaced(temperature), Spaced(music), Spaced(remove) } };
    }

    /// <summary>The first season can't move earlier, the last can't move later.</summary>
    private void UpdateSeasonArrows()
    {
        // Called while the rows are first built too: the constructor updates them once it has them.
        if (_seasonRows is null) return;
        var shown = _seasonRows.Rows.Select(row => row.Item).ToHashSet();
        foreach (var gone in _seasonArrows.Keys.Where(season => !shown.Contains(season)).ToList()) _seasonArrows.Remove(gone);
        foreach (var (season, (up, down)) in _seasonArrows)
        {
            up.IsEnabled = ViewModel.CanMoveSeason(season, -1);
            down.IsEnabled = ViewModel.CanMoveSeason(season, 1);
        }
    }

    private Control WeekdayRow(WeekdayDraft day)
    {
        var id = Box("Weekday_Id", nameof(WeekdayDraft.Id), 110, day);
        var name = Box("Weekday_Name", nameof(WeekdayDraft.Name), 145, day);
        var local = Box("Weekday_LocalName", nameof(WeekdayDraft.LocalName), 145, day);
        AutomationProperties.SetName(id, "Weekday id");
        AutomationProperties.SetName(name, "Weekday name");
        AutomationProperties.SetName(local, "Translated weekday name");
        local.PlaceholderText = "Translated name";
        return Ui.HStack(6, id, name, local,
            Ui.Button("Up", () => ViewModel.MoveWeekday(day, -1), "tool", "small"),
            Ui.Button("Down", () => ViewModel.MoveWeekday(day, 1), "tool", "small"),
            Ui.Button("Remove", () => ViewModel.Weekdays.Remove(day), "tool", "small"));
    }

    private Control FestivalRow(FestivalDraft festival)
    {
        var id = Box("Festival_Id", nameof(FestivalDraft.Id), 110, festival);
        var name = Box("Festival_Name", nameof(FestivalDraft.Name), 145, festival);
        var season = Box("Festival_SeasonId", nameof(FestivalDraft.SeasonId), 110, festival);
        var day = Box("Festival_Day", nameof(FestivalDraft.Day), 60, festival);
        var music = RowPicker("Festival_Music", ViewModel.MusicChoices(festival.Music), festival.Music, value => festival.Music = value, 150);
        Ui.Label((id, festival.IdLabel), (name, festival.NameLabel), (season, festival.SeasonIdLabel), (day, festival.DayLabel), (music, festival.MusicLabel));
        var remove = Ui.Button("Remove", () => ViewModel.RemoveFestival(festival), "tool", "small");
        Ui.Label((remove, $"Remove {festival.What}"));
        var row = new StackPanel { Spacing = 6 };
        row.Children.Add(new WrapPanel { Children = { Spaced(id), Spaced(name), Spaced(season), Spaced(day), Spaced(music), Spaced(remove) } });
        if (ViewModel.Workspace.Current is { } project)
        {
            var enabled = new CheckBox { Content = "Festival place and activity", IsChecked = festival.Program is not null };
            var fields = new StackPanel { Spacing = 6 };
            var program = new ContentForm(project, festival.Program ?? FestivalProgram.Default, typeof(FestivalProgram), fields, ViewModel.ReportPeople);
            var expand = new Expander { Header = "Festival program", Content = fields, IsVisible = enabled.IsChecked == true };
            enabled.IsCheckedChanged += (_, _) => expand.IsVisible = enabled.IsChecked == true;
            festival.ReadProgram = () => enabled.IsChecked == true ? (FestivalProgram)program.Commit() : null;
            festival.ProgramDirty = () => (enabled.IsChecked == true) != (festival.Program is not null) || (enabled.IsChecked == true && program.IsDirty);
            row.Children.Add(enabled);
            row.Children.Add(expand);
        }
        return row;
    }

    /// <summary>A row's control with the gap a wrapping row keeps after it.</summary>
    private static Control Spaced(Control control)
    {
        control.Margin = new Thickness(0, 0, 8, 6);
        return control;
    }

    /// <summary>
    /// A picker over a draft row's field: <paramref name="options"/> (from the view model, with a
    /// missing current id kept as "(missing: id)"); choosing one sets the field.
    /// </summary>
    private static ComboBox RowPicker(string name, IReadOnlyList<PickerOption> options, string current, Action<string> set, double width)
    {
        var box = new ComboBox { Name = name, Width = width };
        FillPicker(box, options, current);
        box.SelectionChanged += (_, _) =>
        {
            if (box.SelectedItem is ComboBoxItem { Tag: string id }) set(id);
        };
        return box;
    }

    /// <summary>Puts <paramref name="options"/> in a <see cref="RowPicker"/>, with <paramref name="current"/> chosen.</summary>
    private static void FillPicker(ComboBox box, IReadOnlyList<PickerOption> options, string current)
    {
        box.Items.Clear();
        foreach (var option in options)
        {
            var item = new ComboBoxItem { Content = option.Label, Tag = option.Id };
            if (option.Missing) item.Classes.Add("missing");
            box.Items.Add(item);
        }

        box.SelectedItem = box.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, current));
    }

    /// <summary>Offers each weather type the overlays the rows have now, before the weather is saved.</summary>
    private void RefreshOverlayPickers()
    {
        foreach (var look in _overlayPickers.Keys.Where(look => !ViewModel.WeatherLooks.Contains(look)).ToList())
        {
            _overlayPickers.Remove(look);
        }

        foreach (var (look, picker) in _overlayPickers)
        {
            FillPicker(picker, ViewModel.OverlayChoices(look.Overlay), look.Overlay);
        }
    }

    private Control SkillNameRow(SkillNameDraft skill)
    {
        var name = Box($"Setting_SkillName_{skill.SkillId}", nameof(SkillNameDraft.Name), 180, skill);
        name.PlaceholderText = skill.SkillId;
        Ui.Label((name, skill.Label));
        return Ui.HStack(8, new TextBlock { Text = skill.SkillId, Width = 90, VerticalAlignment = Avalonia.Layout.VerticalAlignment.Center }, name);
    }

    private Control OverlayRow(OverlayDraft overlay)
    {
        var id = Box($"Overlay_Id_{overlay.Key}", nameof(OverlayDraft.Id), 100, overlay);
        var styles = ViewModel.OverlayStyles.Select(style => new PickerOption(style, style, false)).ToList();
        var style = RowPicker($"Overlay_Style_{overlay.Key}", styles, overlay.Style, value => overlay.Style = value, 90);
        var color = Box($"Overlay_Color_{overlay.Key}", nameof(OverlayDraft.Color), 85, overlay);
        var density = Box($"Overlay_Density_{overlay.Key}", nameof(OverlayDraft.Density), 55, overlay);
        var speed = Box($"Overlay_Speed_{overlay.Key}", nameof(OverlayDraft.Speed), 55, overlay);
        var tint = Box($"Overlay_Tint_{overlay.Key}", nameof(OverlayDraft.Tint), 85, overlay);
        tint.PlaceholderText = EditorStrings.Get("settings.noTint");
        ToolTip.SetTip(density, EditorStrings.Get("settings.densityTip"));
        ToolTip.SetTip(speed, EditorStrings.Get("settings.speedTip"));
        ToolTip.SetTip(tint, EditorStrings.Get("settings.tintTip"));
        var remove = Ui.Button(EditorStrings.Get("common.remove"), () => ViewModel.RemoveOverlay(overlay), "tool", "small");
        remove.Name = $"Overlay_Remove_{overlay.Key}";
        Ui.Label((id, overlay.IdLabel), (style, overlay.StyleLabel), (color, overlay.ColorLabel), (density, overlay.DensityLabel),
            (speed, overlay.SpeedLabel), (tint, overlay.TintLabel), (remove, $"Remove {overlay.What}"));
        return new WrapPanel { Children = { Spaced(id), Spaced(style), Spaced(color), Spaced(density), Spaced(speed), Spaced(tint), Spaced(remove) } };
    }

    private Control WeatherLookRow(WeatherLookDraft look)
    {
        var overlay = RowPicker($"WeatherLook_Overlay_{look.WeatherId}", ViewModel.OverlayChoices(look.Overlay), look.Overlay, value => look.Overlay = value, 150);
        _overlayPickers[look] = overlay;
        var music = RowPicker($"WeatherLook_Music_{look.WeatherId}", ViewModel.MusicChoices(look.Music), look.Music, value => look.Music = value, 150);
        Ui.Label((overlay, look.OverlayLabel), (music, look.MusicLabel));
        return Ui.HStack(8, new TextBlock { Text = look.WeatherName, Width = 90, VerticalAlignment = Avalonia.Layout.VerticalAlignment.Center },
            Ui.Text(EditorStrings.Get("settings.overlay"), "muted", "small"), overlay, Ui.Text(EditorStrings.Get("common.music"), "muted", "small"), music);
    }

    private Control LookRow(LookDraft look)
    {
        var key = look.SeasonId ?? "all";
        var daylight = Box($"Look_Daylight_{key}", nameof(LookDraft.Daylight), 220, look);
        daylight.AcceptsReturn = true;
        daylight.MinHeight = 54;
        daylight.PlaceholderText = EditorStrings.Get(look.SeasonId is null ? "settings.builtinSky" : "settings.allSeasonsSky");
        var foliage = Box($"Look_Foliage_{key}", nameof(LookDraft.FoliageTint), 90, look);
        foliage.PlaceholderText = "#rrggbb";
        Ui.Label((daylight, look.DaylightLabel), (foliage, look.FoliageTintLabel));
        return Ui.HStack(8, new TextBlock { Text = look.What, Width = 90, VerticalAlignment = Avalonia.Layout.VerticalAlignment.Top },
            Ui.VStack(3, Ui.Text(EditorStrings.Get("common.daylight"), "muted", "small"), daylight), Ui.VStack(3, Ui.Text(EditorStrings.Get("common.foliageTint"), "muted", "small"), foliage));
    }

    private void ShowWeather()
    {
        _weather.Children.Clear();
        foreach (var season in ViewModel.Weather)
        {
            var row = Ui.HStack(6, new TextBlock { Text = season.SeasonName, Width = 110, VerticalAlignment = Avalonia.Layout.VerticalAlignment.Center });
            foreach (var cell in season.Cells)
            {
                var box = Box($"Weather_{cell.SeasonId}_{cell.WeatherId}", nameof(WeatherCellDraft.Weight), 55, cell);
                ToolTip.SetTip(box, cell.Tip);
                Ui.Label((box, cell.Label));
                row.Children.Add(Ui.Text(cell.WeatherName, "muted", "small"));
                row.Children.Add(box);
            }

            _weather.Children.Add(row);
        }
    }

    private void ShowMine()
    {
        _mineEnabled.IsChecked = ViewModel.MineEnabled;
        _mineFields.IsVisible = ViewModel.MineEnabled;
    }

    private void ShowMineScenes() =>
        Fill(_mineScene, ViewModel.MineScenes.Select(scene => (scene.Id, scene.Name, false)), ViewModel.MineSceneId);

    private void ShowIcons() =>
        Fill(_exportIcon, ViewModel.IconOptions.Select(option => (option.Id, option.Label, option.Missing)), ViewModel.ExportIcon);

    private void ShowPixelScales() =>
        Fill(_exportPixelScale, ViewModel.PixelScaleOptions.Select(option => (option.Id, option.Label, option.Missing)), ViewModel.ExportPixelScale);

    /// <summary>Fills a picker with the choices, unless it already has them, and selects <paramref name="current"/>.</summary>
    private void Fill(ComboBox box, IEnumerable<(string Id, string Label, bool Missing)> options, string? current)
    {
        _filling = true;
        try
        {
            var choices = options.ToList();
            var items = box.Items.OfType<ComboBoxItem>().ToList();
            var same = items.Count == choices.Count && items.Zip(choices).All(pair =>
                Equals(pair.First.Tag, pair.Second.Id) && Equals(pair.First.Content, pair.Second.Label) && pair.First.Classes.Contains("missing") == pair.Second.Missing);
            if (!same)
            {
                box.SelectedItem = null;
                box.Items.Clear();
                foreach (var (id, label, missing) in choices)
                {
                    var item = new ComboBoxItem { Content = label, Tag = id };
                    if (missing) item.Classes.Add("missing");
                    box.Items.Add(item);
                }
            }

            box.SelectedItem = box.Items.OfType<ComboBoxItem>().FirstOrDefault(item => current is not null && Equals(item.Tag, current));
        }
        finally { _filling = false; }
    }

    private void ShowExportProblems()
    {
        _exportProblems.Children.Clear();
        var problems = ViewModel.ExportProblems;
        for (var i = 0; i < problems.Count; i++)
        {
            var line = Ui.Wrapped(problems[i].Message, problems[i].IsError ? "error" : "muted", "small");
            line.Name = $"ExportSettingsProblem_{i}";
            _exportProblems.Children.Add(line);
        }
    }
}
