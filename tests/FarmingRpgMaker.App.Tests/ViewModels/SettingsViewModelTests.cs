using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmingRpgMaker.App.ViewModels.EditMode;

namespace FarmingRpgMaker.App.Tests.ViewModels;

/// <summary>The Settings tab's sections, drafts (#70) and field errors (#46), driven without the editor.</summary>
public sealed class SettingsViewModelTests
{
    [Fact]
    public void ProjectSettingsSaveAsOneUndoStepAndErrorsNameTheField()
    {
        using var test = new TestWorkspace();
        var viewModel = new SettingsViewModel(test.Workspace);
        var before = test.Project;
        Assert.Equal(before.Name, viewModel.Name);
        Assert.False(viewModel.HasUnsavedChanges);

        viewModel.DayEnd = "12.5";
        Assert.True(viewModel.IsDirty(SettingsSection.Project));
        viewModel.SaveCommand.Execute(null);
        Assert.Equal("Could not save: Day end minute must be a whole number.", viewModel.Message);
        viewModel.DayEnd = "1200";
        viewModel.MaxEnergy = "0";
        viewModel.SaveCommand.Execute(null);
        Assert.Equal("Could not save: Max energy must be more than 0.", viewModel.Message);
        viewModel.MaxEnergy = "100";
        viewModel.CollapseFraction = "2";
        viewModel.SaveCommand.Execute(null);
        Assert.Equal("Could not save: Collapse energy fraction must be from 0 to 1.", viewModel.Message);
        viewModel.CollapseFraction = "0.5";
        viewModel.PlayerSpeed = "16";
        viewModel.SaveCommand.Execute(null);
        Assert.Equal("Could not save: Player speed can be at most 15 tiles a second.", viewModel.Message);
        Assert.Same(before, test.Project);

        viewModel.PlayerSpeed = "7.5";
        viewModel.Name = "  Renamed Farm ";
        viewModel.SaveCommand.Execute(null);
        Assert.Equal("Settings saved.", viewModel.Message);
        Assert.Equal(7.5, test.Project.Settings.Movement.PlayerSpeed);
        Assert.Equal("Renamed Farm", test.Project.Name);
        Assert.False(viewModel.HasUnsavedChanges);
        test.Workspace.Undo();
        Assert.Same(before, test.Project);
        Assert.Equal(before.Name, viewModel.Name);
    }

    [Fact]
    public void SeasonRowsMoveAndGoAwayInTheDraftUntilSaved()
    {
        using var test = new TestWorkspace();
        var viewModel = new SettingsViewModel(test.Workspace);
        var ids = test.Project.Settings.Calendar.Seasons.Select(s => s.Id).ToList();
        var first = viewModel.Seasons[0];
        Assert.False(viewModel.CanMoveSeason(first, -1));
        Assert.True(viewModel.CanMoveSeason(first, 1));

        viewModel.MoveSeason(viewModel.Seasons[1], -1);
        Assert.Equal("Season order changed. Save project settings to apply it.", viewModel.Message);
        Assert.Equal([ids[1], ids[0], .. ids[2..]], viewModel.Seasons.Select(s => s.Id));
        var last = viewModel.Seasons[^1];
        viewModel.RemoveSeason(last);
        Assert.Equal($"Removed {last.What} from the calendar. Save project settings to apply it.", viewModel.Message);
        Assert.Equal(ids, test.Project.Settings.Calendar.Seasons.Select(s => s.Id));

        viewModel.SaveCommand.Execute(null);
        Assert.Equal([ids[1], ids[0], .. ids[2..^1]], test.Project.Settings.Calendar.Seasons.Select(s => s.Id));
        Assert.DoesNotContain(test.Project.Weather.Table, entry => entry.Item1 == last.Key);

        // The calendar keeps at least one season.
        while (viewModel.Seasons.Count > 1) viewModel.RemoveSeason(viewModel.Seasons[0]);
        viewModel.RemoveSeason(viewModel.Seasons[0]);
        Assert.Single(viewModel.Seasons);
        Assert.Equal("The calendar needs at least one season.", viewModel.Message);

        // A new season's id can be typed; a bad day count names the season.
        viewModel.AddSeasonCommand.Execute(null);
        var added = viewModel.Seasons[^1];
        Assert.False(added.IsSaved);
        added.Days = "many";
        viewModel.SaveCommand.Execute(null);
        Assert.Equal($"Could not save: {added.What} days must be a number.", viewModel.Message);
        viewModel.RevertCommand.Execute(null);
        Assert.False(viewModel.HasUnsavedChanges);
    }

    [Fact]
    public void SkillLevelsContinueTheLastStep()
    {
        using var test = new TestWorkspace();
        var viewModel = new SettingsViewModel(test.Workspace);
        Assert.Equal(["0", "50", "150", "300", "500", "750", "1050", "1400", "1800", "2250"], viewModel.SkillLevels.Select(level => level.Xp));
        viewModel.AddSkillLevelCommand.Execute(null);
        Assert.Equal("2700", viewModel.SkillLevels[^1].Xp);
        viewModel.SkillLevels[3].Xp = "100";
        viewModel.SaveCommand.Execute(null);
        Assert.Equal("Could not save: Each level needs at least as much XP as the one before.", viewModel.Message);
        viewModel.SkillLevels[3].Xp = "-5";
        viewModel.SaveCommand.Execute(null);
        Assert.Equal("Could not save: Level 3 needs an XP amount of 0 or more.", viewModel.Message);

        while (viewModel.SkillLevels.Count > 0) viewModel.RemoveSkillLevel(0);
        viewModel.SaveCommand.Execute(null);
        Assert.Equal("Could not save: Add at least one skill level.", viewModel.Message);
        for (var i = 0; i < 3; i++) viewModel.AddSkillLevelCommand.Execute(null);
        Assert.Equal(["0", "100", "200"], viewModel.SkillLevels.Select(level => level.Xp));
    }

    [Fact]
    public void SavingOneSectionKeepsTheUnsavedFieldsOfTheOthers()
    {
        using var test = new TestWorkspace();
        var viewModel = new SettingsViewModel(test.Workspace);
        viewModel.PlayerSpeed = "7.5";
        var storm = viewModel.Weather.SelectMany(row => row.Cells).First(cell => cell.SeasonId == "spring" && cell.WeatherId == "storm");
        Assert.Equal("Storm weight in Spring", storm.Label);
        storm.Weight = "4";
        viewModel.SaveWeatherCommand.Execute(null);
        Assert.Equal(4, SettingsForm.WeatherWeight(test.Project, "spring", "storm"));
        Assert.Equal("Weather odds saved.", viewModel.WeatherMessage);
        Assert.Equal("7.5", viewModel.PlayerSpeed);
        // The refresh the weather edit caused ran before the weather fields were reloaded.
        Assert.Equal("Unsaved changes kept (project, weather): save or revert them.", viewModel.Message);

        viewModel.SaveWeatherCommand.Execute(null);
        Assert.Equal("No changes were made.", viewModel.WeatherMessage);
        viewModel.Weather[0].Cells[0].Weight = "lots";
        viewModel.SaveWeatherCommand.Execute(null);
        Assert.Contains("must be a number", viewModel.WeatherMessage, StringComparison.Ordinal);

        // Leaving the project saves the rest; the bad weather weight can't be.
        Assert.False(viewModel.SaveDrafts());
        Assert.Equal(7.5, test.Project.Settings.Movement.PlayerSpeed);
        Assert.True(viewModel.IsDirty(SettingsSection.Weather));
        Assert.False(viewModel.IsDirty(SettingsSection.Project));
    }

    [Fact]
    public void TheMineTogglesAtOnceAndPicksItsEntranceOnTheMap()
    {
        using var test = new TestWorkspace();
        var picks = new List<(string Scene, Action<int, int> Done)>();
        var viewModel = new SettingsViewModel(test.Workspace, (scene, _, done) => picks.Add((scene, done)));
        Assert.False(viewModel.MineEnabled);
        Assert.True(viewModel.CanPickOnMap);
        viewModel.SetMineEnabled(true);
        Assert.True(test.Project.Mine.Enabled);
        Assert.True(viewModel.MineEnabled);
        Assert.Equal("Mine enabled.", viewModel.MineMessage);
        Assert.Equal(test.Project.StartSceneId, viewModel.MineSceneId);

        viewModel.PickMineEntranceCommand.Execute(null);
        var (scene, done) = Assert.Single(picks);
        Assert.Equal(test.Project.StartSceneId, scene);
        done(6, 4);
        Assert.Equal("Entrance set to (6, 4). Save mine to keep it.", viewModel.MineMessage);
        viewModel.MineFloors = "2.5";
        viewModel.SaveMineCommand.Execute(null);
        Assert.Equal("Could not save: Mine floors must be a whole number.", viewModel.MineMessage);
        viewModel.MineFloors = "0";
        viewModel.MineLadder = "3";
        viewModel.SaveMineCommand.Execute(null);
        var mine = test.Project.Mine;
        Assert.Equal((6.0, 4.0, 1.0, 1.0), (mine.EntranceX.OrNullable(), mine.EntranceY.OrNullable(), mine.Floors, mine.LadderChance));
        Assert.Equal("1", viewModel.MineFloors);

        viewModel.SetMineEnabled(false);
        Assert.Equal("Mine disabled; its settings are kept.", viewModel.MineMessage);
        Assert.False(viewModel.MineEnabled);
        Assert.False(new SettingsViewModel(test.Workspace).CanPickOnMap);
    }

    [Fact]
    public void ExportSettingsAreCheckedWhenSaved()
    {
        using var test = new TestWorkspace();
        var viewModel = new SettingsViewModel(test.Workspace);
        Assert.Equal("Not saved yet: export uses these defaults.", viewModel.ExportMessage);
        Assert.Empty(viewModel.ExportProblems);
        Assert.Equal("1280", viewModel.ExportWidth);
        Assert.NotNull(viewModel.ExportPixelScale);

        viewModel.ExportTitle = "Willow Creek";
        viewModel.ExportWidth = "100";
        viewModel.SaveExportCommand.Execute(null);
        Assert.Equal("Willow Creek", test.Project.Export.OrNull()?.Title.OrNull());
        Assert.Contains(viewModel.ExportProblems, problem => problem.Message.Contains("320×240", StringComparison.Ordinal));
        Assert.EndsWith("Fix the errors above before exporting.", viewModel.ExportMessage, StringComparison.Ordinal);

        viewModel.ExportWidth = "wide";
        viewModel.SaveExportCommand.Execute(null);
        Assert.Equal("Could not save: Window width must be a whole number.", viewModel.ExportMessage);
    }
}
