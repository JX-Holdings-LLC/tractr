using System.Text.Json.Nodes;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Headless.XUnit;
using Avalonia.Interactivity;
using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using Tractr.App.Game;
using static Tractr.App.Tests.Ui.UiTestHelpers;

namespace Tractr.App.Tests.Game;

/// <summary>
/// The editor side of the world vocabulary and of sound (M1, M2): the map's terrain brushes, a
/// scene's surroundings, cutscene and music outcome forms, sound and music import, and the
/// Settings tab's words, weather looks and atmosphere.
/// </summary>
public sealed class WorldVocabularyEditorTests
{
    private static void Press(Control root, string name) =>
        FindByName<Button>(root, name).RaiseEvent(new RoutedEventArgs(Button.ClickEvent));

    private static void Choose(GameTestHost host, string name, string id)
    {
        var picker = FindByName<ComboBox>(host.Window, name);
        picker.SelectedItem = picker.Items.OfType<ComboBoxItem>().First(item => (string)item.Tag! == id);
        Pump();
    }

    private static T With<T>(T record, string key, JsonNode? value)
    {
        var node = (JsonObject)RecordJson.ToNode(record!, typeof(T))!;
        node[key] = value;
        return RecordJson.FromNode<T>(node);
    }

    /// <summary>"OggS" and a few zero bytes: enough header for the import to call it OGG.</summary>
    private static readonly byte[] Ogg = [(byte)'O', (byte)'g', (byte)'g', (byte)'S', 0, 2, 0, 0];

    private static void AddMusic(GameTestHost host) =>
        host.Workspace.Apply(Edits.UpsertAsset(CustomAsset.Default.WithId("music-1").WithName("Theme").WithType(CustomAssetTypes.Music)
            .WithDataUrl("data:audio/ogg;base64," + Convert.ToBase64String(Ogg))));

    [AvaloniaFact]
    public void ThePaletteListsTheGamesTerrainsAndPaintsOnTheirLayer()
    {
        using var host = new GameTestHost();
        var edit = host.Surface.EditView;
        var palette = FindByName<WrapPanel>(host.Window, "TilePalette");
        Assert.Equal(["inspect", .. TileTypes.All], palette.Children.OfType<ToggleButton>().Select(toggle => (toggle.Tag as string) ?? "inspect"));

        var terrain = With(With(Defaults.NewTerrain(host.Workspace.Current!), "name", "Moss"), "layer", TileLayers.Overlay);
        host.Workspace.Apply(Edits.UpsertTerrain(terrain));
        Pump();
        var brush = FindByName<ToggleButton>(host.Window, $"Brush_{terrain.Id}");
        Assert.Equal($"Brush: Moss", Avalonia.Automation.AutomationProperties.GetName(brush));
        brush.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
        Pump();
        Assert.Equal(terrain.Id, edit.Brush);
        Assert.Equal(Edits.LayerFor(TileTypes.Path), edit.Layer);
        Assert.True(brush.IsChecked);

        edit.PaintTile(2, 2);
        var tile = host.Workspace.Current!.Scenes.First(scene => scene.Id == edit.SceneId).Tiles[2][2];
        Assert.Equal(terrain.Id, tile.Overlay.OrNull());
        Assert.Contains(FindByName<ComboBox>(host.Window, "NewSceneTile").Items.OfType<ComboBoxItem>(), item => Equals(item.Tag, terrain.Id));
    }

    [AvaloniaFact]
    public void ASceneKeepsItsSurroundingsAsOneUndoStepEach()
    {
        using var host = new GameTestHost();
        AddMusic(host);
        var edit = host.Surface.EditView;
        var sceneId = edit.SceneId!;
        FarmEngine.Schemas.Scene Scene() => host.Workspace.Current!.Scenes.First(scene => scene.Id == sceneId);

        FindByName<CheckBox>(host.Window, "SceneDark").IsChecked = true;
        Pump();
        Assert.True(Scene().Dark.OrNullable());
        Choose(host, "SceneMusic", "music-1");
        Assert.Equal("music-1", Scene().Music.OrNull());

        FindByName<TextBox>(host.Window, "SceneTemperature").Text = "warm";
        Press(host.Window, "SaveSurroundingsButton");
        Assert.Equal("Could not save the surroundings: Temperature must be a number.", FindByName<TextBlock>(host.Window, "MapEditorMessage").Text);
        FindByName<TextBox>(host.Window, "SceneTemperature").Text = "8";
        FindByName<TextBox>(host.Window, "SceneTags").Text = "cave, damp";
        FindByName<TextBox>(host.Window, "SceneFoliageTint").Text = "#406040";
        FindByName<TextBox>(host.Window, "SceneDaylight").Text = "0 #202040";
        var before = host.Workspace.Current;
        Press(host.Window, "SaveSurroundingsButton");
        var scene = Scene();
        Assert.Equal((8.0, "cave,damp", "#406040"), (scene.Temperature.OrNullable(), string.Join(",", scene.Tags.OrEmpty()), scene.Atmosphere.OrNull()?.FoliageTint.OrNull()));
        Assert.True(scene.Dark.OrNullable());
        Assert.Equal("music-1", scene.Music.OrNull());

        host.Workspace.Undo();
        Assert.Same(before, host.Workspace.Current);
        Pump();
        Assert.Equal("", FindByName<TextBox>(host.Window, "SceneTags").Text);
    }

    [AvaloniaFact]
    public void CutsceneStepsShowTheFieldsOfTheirTypeAndMusicStepsPickMusic()
    {
        using var host = new GameTestHost();
        AddMusic(host);
        FindByName<TabControl>(host.Window, "EditorTabs").SelectedIndex = 1;
        Pump();
        var view = FindByName<ContentEditorView>(host.Window, "ContentEditorView");
        view.ViewModel.SelectCategory("Cutscenes");
        Pump();
        Press(host.Window, "AddContentButton");
        Pump();
        Assert.Equal("say", (FindByName<ComboBox>(host.Window, "ContentField_Steps_1_Type").SelectedItem as ComboBoxItem)?.Tag);
        Assert.NotNull(TryFindByName<TextBox>(host.Window, "ContentField_Steps_1_Text"));
        Assert.Null(TryFindByName<Control>(host.Window, "ContentField_Steps_1_X"));
        Assert.Null(TryFindByName<Control>(host.Window, "ContentField_Steps_1_SoundId"));

        Choose(host, "ContentField_Steps_1_Type", "music");
        Assert.Null(TryFindByName<TextBox>(host.Window, "ContentField_Steps_1_Text"));
        Choose(host, "ContentField_Steps_1_SoundId", "music-1");
        Press(host.Window, "ContentAdd_Steps");
        Assert.Equal("say", (FindByName<ComboBox>(host.Window, "ContentField_Steps_3_Type").SelectedItem as ComboBoxItem)?.Tag);
        Press(host.Window, "SaveContentButton");
        var cutscene = host.Workspace.Current!.Cutscenes.OrEmpty().Single();
        Assert.Equal(["fade", "music", "fade", "say"], cutscene.Steps.Select(step => step.Type));
        Assert.Equal("music-1", cutscene.Steps[1].SoundId.OrNull());

        // Events start it and play music through the outcome editors.
        view.ViewModel.SelectCategory("Events");
        Pump();
        Press(host.Window, "AddContentButton");
        Pump();
        Choose(host, "ContentAdd_Outcomes", "startCutscene");
        Choose(host, "ContentField_Outcomes_1_CutsceneId", cutscene.Id);
        Choose(host, "ContentAdd_Outcomes", "playMusic");
        var music = FindByName<ComboBox>(host.Window, "ContentField_Outcomes_2_SoundId");
        Assert.Equal(["", "music-1"], music.Items.OfType<ComboBoxItem>().Select(item => (string)item.Tag!));
        Choose(host, "ContentField_Outcomes_2_SoundId", "music-1");
        Press(host.Window, "SaveContentButton");
        var outcomes = host.Workspace.Current!.Events[^1].Outcomes;
        Assert.Equal((cutscene.Id, "music-1"), (outcomes[1].CutsceneId.OrNull(), outcomes[2].SoundId.OrNull()));
    }

    [AvaloniaFact]
    public void CutsceneActorsAndSpeakersPickThePlayerOrAnNpc()
    {
        using var host = new GameTestHost();
        var npcs = host.Workspace.Current!.Npcs.Select(npc => npc.Id).ToArray();
        FindByName<TabControl>(host.Window, "EditorTabs").SelectedIndex = 1;
        Pump();
        FindByName<ContentEditorView>(host.Window, "ContentEditorView").ViewModel.SelectCategory("Cutscenes");
        Pump();
        Press(host.Window, "AddContentButton");
        Pump();
        static string[] Ids(ComboBox picker) => [.. picker.Items.OfType<ComboBoxItem>().Select(item => (string)item.Tag!)];

        // A line's speaker: a narrator, the player or an NPC.
        Assert.Equal(["", "player", .. npcs], Ids(FindByName<ComboBox>(host.Window, "ContentField_Steps_1_Speaker")));
        Choose(host, "ContentField_Steps_1_Speaker", npcs[0]);
        // A move's actor: the player or an NPC.
        Choose(host, "ContentField_Steps_1_Type", "move");
        Assert.Equal(["", "player", .. npcs], Ids(FindByName<ComboBox>(host.Window, "ContentField_Steps_1_Actor")));
        Choose(host, "ContentField_Steps_1_Actor", "player");
        Press(host.Window, "SaveContentButton");
        var step = host.Workspace.Current!.Cutscenes.OrEmpty().Single().Steps[1];
        Assert.Equal(("move", "player"), (step.Type, step.Actor.OrNull()));
    }

    [AvaloniaFact]
    public void CropsKeepTheirGrowthRequirementsOnTheirOwnTab()
    {
        using var host = new GameTestHost();
        FindByName<TabControl>(host.Window, "EditorTabs").SelectedIndex = 1;
        Pump();
        FindByName<ContentEditorView>(host.Window, "ContentEditorView").ViewModel.SelectCategory("Crops");
        Pump();
        Press(host.Window, "AddContentButton");
        Pump();
        var include = FindByName<CheckBox>(host.Window, "ContentInclude_Requirements");
        include.IsChecked = true;
        Pump();
        Choose(host, "ContentChipAdd_Requirements_Terrains", TileTypes.Soil);
        Choose(host, "ContentField_Requirements_Light", "dark");
        FindByName<TextBox>(host.Window, "ContentField_Requirements_MinTemperature").Text = "5";
        Press(host.Window, "ContentAdd_Requirements_Neighbors");
        Press(host.Window, "SaveContentButton");
        var needs = host.Workspace.Current!.CustomCrops.OrEmpty()[^1].Requirements.OrNull()!;
        Assert.Equal([TileTypes.Soil], needs.Terrains);
        Assert.Equal(("dark", 5.0), (needs.Light.OrNull(), needs.MinTemperature.OrNullable()));
        Assert.Equal(NeighborKinds.Water, needs.Neighbors.Single().Kind);
    }

    [AvaloniaFact]
    public void SoundsAndMusicImportAsOneUndoStepAndListApartFromArt()
    {
        using var host = new GameTestHost();
        FindByName<TabControl>(host.Window, "EditorTabs").SelectedIndex = 5;
        Pump();
        var art = FindByName<ArtEditorView>(host.Window, "ArtEditorView");
        var before = host.Workspace.Current!;
        byte[] wav = [.. "RIFF"u8.ToArray(), 4, 0, 0, 0, .. "WAVE"u8.ToArray()];
        art.ImportSounds([("theme.ogg", Ogg), ("step.wav", wav), ("cover.png", [137, 80, 78, 71])], CustomAssetTypes.Music);
        var assets = host.Workspace.Current!.CustomAssets.Where(asset => asset.Type == CustomAssetTypes.Music).ToList();
        Assert.Equal(["theme.ogg", "step.wav"], assets.Select(asset => asset.Name));
        Assert.StartsWith("data:audio/ogg;base64,", assets[0].DataUrl, StringComparison.Ordinal);
        Assert.StartsWith("data:audio/wav;base64,", assets[1].DataUrl, StringComparison.Ordinal);
        var message = FindByName<TextBlock>(host.Window, "ArtMessage").Text ?? "";
        Assert.Contains("Imported 2 music tracks.", message, StringComparison.Ordinal);
        Assert.Contains("cover.png: Use OGG (Vorbis) or WAV sound files.", message, StringComparison.Ordinal);

        var sounds = FindByName<ListBox>(host.Window, "SoundAssets");
        Assert.Equal(2, sounds.ItemCount);
        Assert.Equal(assets[1].Id, art.SelectedSoundId);
        Assert.DoesNotContain(FindByName<ListBox>(host.Window, "ArtAssets").Items.OfType<ListBoxItem>(), item => assets.Any(asset => Equals(item.Tag, asset.Id)));
        Assert.DoesNotContain(Problems.Collect(host.Workspace.Current!), problem => problem.Code == "asset.notAudio");
        // Music nothing plays yet is not unused art.
        Assert.DoesNotContain(ArtLibrary.Unused(host.Workspace.Current!), asset => asset.Type == CustomAssetTypes.Music);

        Press(host.Window, "RemoveSoundButton");
        Assert.Single(host.Workspace.Current!.CustomAssets, asset => asset.Type == CustomAssetTypes.Music);
        host.Workspace.Undo();
        host.Workspace.Undo();
        Assert.Same(before, host.Workspace.Current);
    }

    [AvaloniaFact]
    public void SettingsShowTheWordsWeatherLooksAndAtmosphere()
    {
        using var host = new GameTestHost();
        AddMusic(host);
        FindByName<TabControl>(host.Window, "EditorTabs").SelectedIndex = 3;
        Pump();
        FindByName<TextBox>(host.Window, "Setting_MoneyFormat").Text = "{0} shells";
        FindByName<TextBox>(host.Window, "Setting_SkillName_farming").Text = "Gardening";
        Choose(host, "Season_Music", "music-1");
        Press(host.Window, "SaveSettingsButton");
        var settings = host.Workspace.Current!.Settings;
        Assert.Equal("{0} shells", settings.Words.OrNull()?.Money.OrNull());
        Assert.Equal("music-1", settings.Calendar.Seasons[0].Music.OrNull());

        // A new overlay is offered to the weather types before the weather is saved, under its id as typed.
        Press(host.Window, "AddOverlayButton");
        Pump();
        var weatherId = host.Workspace.Current!.Weather.Types[0].Id;
        var picker = FindByName<ComboBox>(host.Window, $"WeatherLook_Overlay_{weatherId}");
        Assert.Contains(picker.Items.OfType<ComboBoxItem>(), item => Equals(item.Tag, "overlay-1"));
        FindByName<TextBox>(host.Window, "Overlay_Id_overlay-1").Text = "acid";
        Pump();
        Assert.Contains(picker.Items.OfType<ComboBoxItem>(), item => Equals(item.Tag, "acid"));
        Assert.DoesNotContain(picker.Items.OfType<ComboBoxItem>(), item => Equals(item.Tag, "overlay-1"));
        FindByName<TextBox>(host.Window, "Overlay_Id_overlay-1").Text = "overlay-1";
        Pump();
        Choose(host, $"WeatherLook_Overlay_{weatherId}", "overlay-1");
        Choose(host, $"WeatherLook_Music_{weatherId}", "music-1");
        Press(host.Window, "SaveWeatherButton");
        var weather = host.Workspace.Current!.Weather;
        Assert.Equal("overlay-1", weather.Overlays.OrEmpty().Single().Id);
        Assert.Equal(("overlay-1", "music-1"), (weather.Types[0].Overlay.OrNull(), weather.Types[0].Music.OrNull()));

        FindByName<TextBox>(host.Window, "Look_Daylight_all").Text = "360 #ffd8a0";
        FindByName<TextBox>(host.Window, "Look_Foliage_all").Text = "#80a060";
        Press(host.Window, "SaveAtmosphereButton");
        Assert.Equal("Atmosphere saved.", FindByName<TextBlock>(host.Window, "AtmosphereMessage").Text);
        Assert.Equal("#80a060", host.Workspace.Current!.Settings.Atmosphere.OrNull()?.AllSeasons.FoliageTint.OrNull());
    }
}
