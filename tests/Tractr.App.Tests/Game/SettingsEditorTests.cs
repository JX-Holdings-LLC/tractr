using Avalonia.Controls;
using Avalonia.Headless.XUnit;
using Avalonia.Interactivity;
using Avalonia.VisualTree;
using FarmEngine.Authoring;
using static Tractr.App.Tests.Ui.UiTestHelpers;

namespace Tractr.App.Tests.Game;

/// <summary>Project Settings: weather odds, the mine card, the season arrows, the game's rules and its properties.</summary>
public sealed class SettingsEditorTests
{
    private static readonly string? OutputDir = Environment.GetEnvironmentVariable("FRM_SCREENSHOT_DIR");

    /// <summary>Saves a look at the tab scrolled to <paramref name="control"/> when FRM_SCREENSHOT_DIR is set.</summary>
    private static void Snapshot(GameTestHost host, Control control, string file)
    {
        if (string.IsNullOrEmpty(OutputDir)) return;
        control.BringIntoView();
        Pump();
        GameTestHost.Capture(host.Window).Save(Path.Combine(OutputDir, file), Avalonia.Media.Imaging.PngBitmapEncoderOptions.Default);
    }

    private static void OpenSettings(GameTestHost host)
    {
        FindByName<TabControl>(host.Window, "EditorTabs").SelectedIndex = 3;
        Pump();
    }

    private static void Press(GameTestHost host, string name) =>
        FindByName<Button>(host.Window, name).RaiseEvent(new RoutedEventArgs(Button.ClickEvent));

    private static void Toggle(GameTestHost host, string name, bool on)
    {
        var box = FindByName<CheckBox>(host.Window, name);
        box.IsChecked = on;
        box.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
        Pump();
    }

    /// <summary>The Rules section (#243): the switch spells the classic rules out, the workshop preset drops sleep and farming, and Play Mode follows.</summary>
    [AvaloniaFact]
    public void RulesSwitchAndWorkshopPresetChangeHowTheGameSleeps()
    {
        using var host = new GameTestHost();
        OpenSettings(host);
        var classic = host.Workspace.Current!;
        Assert.Null(classic.Settings.Rules.OrNull());
        Assert.Null(TryFindByName<Button>(host.Window, "SaveRulesButton"));

        Toggle(host, "Setting_Rules", true);
        var rules = host.Workspace.Current!.Settings.Rules.OrNull();
        Assert.NotNull(rules);
        Assert.Equal("required", rules!.Sleep.OrNull());
        Assert.NotNull(TryFindByName<Button>(host.Window, "SaveRulesButton"));
        Assert.Equal("Rules saved.", FindByName<TextBlock>(host.Window, "RulesMessage").Text);

        Press(host, "WorkshopRulesButton");
        Pump();
        rules = host.Workspace.Current!.Settings.Rules.OrNull();
        Assert.Equal("none", rules!.Sleep.OrNull());
        Assert.Equal("rest", rules.Exhaustion.OrNull());
        Assert.False(rules.Farming.OrNullable());
        Snapshot(host, FindByName<Button>(host.Window, "SaveRulesButton"), "settings-rules.png");
        // Saving the form as it stands changes nothing.
        Press(host, "SaveRulesButton");
        Assert.Equal("No changes were made.", FindByName<TextBlock>(host.Window, "RulesMessage").Text);

        // In play nobody sleeps: the sleep command leaves the day as it is.
        host.EnterPlay();
        host.Play.Use(player => player.RunCommands("""[{"type":"sleep"}]"""));
        Assert.Equal(1, host.Play.Use(player => player.Summary()).Day);
        host.ViewModel.Mode = Tractr.App.Hosting.EditorMode.Edit;
        Pump();

        // Off again: the classic rules, and each step undoes.
        OpenSettings(host);
        Toggle(host, "Setting_Rules", false);
        Assert.Null(host.Workspace.Current!.Settings.Rules.OrNull());
        host.Workspace.Undo();
        Assert.Equal("none", host.Workspace.Current!.Settings.Rules.OrNull()!.Sleep.OrNull());
    }

    /// <summary>The Properties section (#244): rows are drafts until Save properties, which is one undo step.</summary>
    [AvaloniaFact]
    public void PropertiesAreAddedRemovedAndSavedTogether()
    {
        using var host = new GameTestHost();
        OpenSettings(host);
        var before = host.Workspace.Current!;
        Assert.Empty(SettingsForm.PropertyList(before));

        Press(host, "AddPropertyButton");
        Press(host, "AddPropertyButton");
        Pump();
        Assert.NotNull(TryFindByName<Border>(host.Window, "Property_0"));
        Assert.NotNull(TryFindByName<Border>(host.Window, "Property_1"));
        // Drafts: nothing is in the project until they are saved.
        Assert.Same(before, host.Workspace.Current);

        Press(host, "SavePropertiesButton");
        Pump();
        var saved = SettingsForm.PropertyList(host.Workspace.Current!);
        Assert.Equal(["property-1", "property-2"], saved.Select(property => property.Id));
        Assert.All(saved, property => Assert.Equal("player", property.Scope));
        Assert.Equal("Properties saved.", FindByName<TextBlock>(host.Window, "PropertiesMessage").Text);
        Assert.DoesNotContain(Problems.Collect(host.Workspace.Current!), problem => problem.Code.StartsWith("property.", StringComparison.Ordinal));

        Snapshot(host, FindByName<Button>(host.Window, "SavePropertiesButton"), "settings-properties.png");
        Press(host, "RemoveProperty_0");
        Press(host, "SavePropertiesButton");
        Pump();
        Assert.Equal(["property-2"], SettingsForm.PropertyList(host.Workspace.Current!).Select(property => property.Id));
        Press(host, "SavePropertiesButton");
        Assert.Equal("No changes were made.", FindByName<TextBlock>(host.Window, "PropertiesMessage").Text);

        // Both saves undo.
        host.Workspace.Undo();
        host.Workspace.Undo();
        Assert.Same(before, host.Workspace.Current);
    }

    [AvaloniaFact]
    public void WeatherOddsSaveAsOneUndoStep()
    {
        using var host = new GameTestHost();
        OpenSettings(host);
        var before = host.Workspace.Current!;
        Assert.Equal("1", FindByName<TextBox>(host.Window, "Weather_spring_storm").Text);
        Assert.Equal("0", FindByName<TextBox>(host.Window, "Weather_spring_snow").Text);

        FindByName<TextBox>(host.Window, "Weather_spring_storm").Text = "4";
        FindByName<TextBox>(host.Window, "Weather_summer_rain").Text = "0";
        Press(host, "SaveWeatherButton");
        var project = host.Workspace.Current!;
        Assert.Equal(4, SettingsForm.WeatherWeight(project, "spring", "storm"));
        Assert.Equal(0, SettingsForm.WeatherWeight(project, "summer", "rain"));
        Assert.Equal("Weather saved.", FindByName<TextBlock>(host.Window, "WeatherMessage").Text);

        Press(host, "SaveWeatherButton");
        Assert.Equal("No changes were made.", FindByName<TextBlock>(host.Window, "WeatherMessage").Text);
        FindByName<TextBox>(host.Window, "Weather_fall_sun").Text = "lots";
        Press(host, "SaveWeatherButton");
        Assert.Contains("Could not save", FindByName<TextBlock>(host.Window, "WeatherMessage").Text);
        Assert.Same(project, host.Workspace.Current);

        host.Workspace.Undo();
        Assert.Same(before, host.Workspace.Current);
    }

    [AvaloniaFact]
    public void MineCardEnablesAndSavesClampedSettings()
    {
        using var host = new GameTestHost();
        OpenSettings(host);
        Assert.False(FindByName<StackPanel>(host.Window, "MineFields").IsVisible);
        var toggle = FindByName<CheckBox>(host.Window, "Mine_Enabled");
        toggle.IsChecked = true;
        toggle.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
        var project = host.Workspace.Current!;
        Assert.True(project.Mine.Enabled);
        Assert.Equal(project.StartSceneId, project.Mine.EntranceSceneId.OrNull());
        Assert.NotEmpty(project.Mine.Bands);
        Assert.True(FindByName<StackPanel>(host.Window, "MineFields").IsVisible);

        FindByName<TextBox>(host.Window, "Mine_EntranceX").Text = "6";
        FindByName<TextBox>(host.Window, "Mine_EntranceY").Text = "4";
        FindByName<TextBox>(host.Window, "Mine_Floors").Text = "0";
        FindByName<TextBox>(host.Window, "Mine_LadderChance").Text = "3";
        Press(host, "SaveMineButton");
        var mine = host.Workspace.Current!.Mine;
        Assert.Equal(6, mine.EntranceX.OrNullable());
        Assert.Equal(4, mine.EntranceY.OrNullable());
        Assert.Equal(1, mine.Floors);
        Assert.Equal(1, mine.LadderChance);
        Assert.Equal("1", FindByName<TextBox>(host.Window, "Mine_Floors").Text);

        toggle.IsChecked = false;
        toggle.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
        Assert.False(host.Workspace.Current!.Mine.Enabled);
        Assert.Equal(6, host.Workspace.Current.Mine.EntranceX.OrNullable());
        host.Workspace.Undo();
        Assert.True(host.Workspace.Current!.Mine.Enabled);
    }

    private static List<string> SeasonRows(GameTestHost host) =>
        FindByName<Tractr.App.Game.SettingsEditorView>(host.Window, "SettingsEditorView").GetVisualDescendants().OfType<TextBox>()
            .Where(box => box.Name == "Season_Id").Select(box => box.Text ?? "").ToList();

    [AvaloniaFact]
    public void SeasonArrowsReorderTheDraftAndSaveApplies()
    {
        using var host = new GameTestHost();
        OpenSettings(host);
        Assert.False(FindByName<Button>(host.Window, "Season_Up_spring").IsEnabled);
        Assert.False(FindByName<Button>(host.Window, "Season_Down_winter").IsEnabled);
        Press(host, "Season_Up_summer");
        Press(host, "Season_Down_fall");
        // The rows move; the project changes on Save (#70).
        Assert.Equal(["summer", "spring", "winter", "fall"], SeasonRows(host));
        Assert.False(FindByName<Button>(host.Window, "Season_Up_summer").IsEnabled);
        Assert.Equal(["spring", "summer", "fall", "winter"], host.Workspace.Current!.Settings.Calendar.Seasons.Select(s => s.Id));
        Press(host, "SaveSettingsButton");
        Assert.Equal(["summer", "spring", "winter", "fall"], host.Workspace.Current!.Settings.Calendar.Seasons.Select(s => s.Id));
        host.Workspace.Undo();
        Assert.Equal(["spring", "summer", "fall", "winter"], host.Workspace.Current!.Settings.Calendar.Seasons.Select(s => s.Id));
    }

    [AvaloniaFact]
    public void RemovingASeasonIsSavedWithTheProjectSettings()
    {
        using var host = new GameTestHost();
        OpenSettings(host);
        Press(host, "Season_Remove_winter");
        Assert.DoesNotContain("winter", SeasonRows(host));
        Assert.Contains(host.Workspace.Current!.Settings.Calendar.Seasons, s => s.Id == "winter");
        Press(host, "SaveSettingsButton");
        var project = host.Workspace.Current!;
        Assert.DoesNotContain(project.Settings.Calendar.Seasons, s => s.Id == "winter");
        Assert.DoesNotContain(project.Weather.Table, entry => entry.Item1 == "winter");
    }

    [AvaloniaFact]
    public void SavingOneSectionKeepsUnsavedFieldsInTheOthers()
    {
        using var host = new GameTestHost();
        OpenSettings(host);
        var speed = FindByName<TextBox>(host.Window, "Setting_PlayerSpeed");
        speed.Text = "7.5";
        FindByName<TextBox>(host.Window, "Weather_spring_storm").Text = "4";
        Press(host, "SaveWeatherButton");
        Assert.Equal(4, SettingsForm.WeatherWeight(host.Workspace.Current!, "spring", "storm"));
        // The Gameplay edit is still there, unsaved (#70).
        Assert.Equal("7.5", speed.Text);
        Assert.NotEqual(7.5, host.Workspace.Current!.Settings.Movement.PlayerSpeed);

        // Toggling the mine, undo and a tab switch keep it too.
        var toggle = FindByName<CheckBox>(host.Window, "Mine_Enabled");
        toggle.IsChecked = !toggle.IsChecked;
        toggle.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
        host.Workspace.Undo();
        FindByName<TabControl>(host.Window, "EditorTabs").SelectedIndex = 0;
        Pump();
        OpenSettings(host);
        Assert.Equal("7.5", speed.Text);
        Press(host, "SaveSettingsButton");
        Assert.Equal(7.5, host.Workspace.Current!.Settings.Movement.PlayerSpeed);
    }

    [AvaloniaFact]
    public void ErrorsNameTheField()
    {
        using var host = new GameTestHost();
        OpenSettings(host);
        FindByName<TextBox>(host.Window, "Setting_DayEndMinute").Text = "12.5";
        Press(host, "SaveSettingsButton");
        Assert.Equal("Could not save: Day end minute must be a whole number.", FindByName<TextBlock>(host.Window, "SettingsMessage").Text);
        FindByName<TextBox>(host.Window, "Setting_DayEndMinute").Text = "lots";
        Press(host, "SaveSettingsButton");
        Assert.Equal("Could not save: Day end minute must be a number.", FindByName<TextBlock>(host.Window, "SettingsMessage").Text);
        FindByName<TextBox>(host.Window, "Setting_DayEndMinute").Text = "1200";
        FindByName<TextBox>(host.Window, "Setting_MaxEnergy").Text = "0";
        Press(host, "SaveSettingsButton");
        Assert.Equal("Could not save: Max energy must be more than 0.", FindByName<TextBlock>(host.Window, "SettingsMessage").Text);
        FindByName<TextBox>(host.Window, "Setting_MaxEnergy").Text = "100";
        FindByName<TextBox>(host.Window, "Setting_CollapseEnergyFraction").Text = "2";
        Press(host, "SaveSettingsButton");
        Assert.Equal("Could not save: Collapse energy fraction must be from 0 to 1.", FindByName<TextBlock>(host.Window, "SettingsMessage").Text);
        FindByName<TextBox>(host.Window, "Weather_fall_sun").Text = "lots";
        Press(host, "SaveWeatherButton");
        Assert.Contains("weight in Fall must be a number", FindByName<TextBlock>(host.Window, "WeatherMessage").Text, StringComparison.Ordinal);
    }
}
