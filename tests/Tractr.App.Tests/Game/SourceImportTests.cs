using Avalonia.Controls;
using Avalonia.Headless.XUnit;
using Avalonia.Interactivity;
using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using Tractr.App.Game;
using Tractr.App.Tests.Interop;
using static Tractr.App.Tests.Ui.UiTestHelpers;

namespace Tractr.App.Tests.Game;

/// <summary>The editor's Aseprite and map imports (#232, #233) and the interface skin form (#236).</summary>
public sealed class SourceImportTests
{
    private static void Press(Control root, string name) =>
        FindByName<Button>(root, name).RaiseEvent(new RoutedEventArgs(Button.ClickEvent));

    [AvaloniaFact]
    public void AnAsepriteFileImportsThroughThePreviewAndKeepsTheRenamedClip()
    {
        using var host = new GameTestHost();
        FindByName<TabControl>(host.Window, "EditorTabs").SelectedIndex = 5;
        Pump();
        var art = FindByName<ArtEditorView>(host.Window, "ArtEditorView");
        var path = RustSessionTests.RepoFile("fixtures", "import", "aseprite", "hero.aseprite");
        art.ShowSpriteSheetPreview(path);
        Pump();
        Assert.True(FindByName<Border>(host.Window, "AsepritePreview").IsVisible);
        FindByName<TextBox>(host.Window, "AsepriteClip-idle-blink").Text = "blink";
        Press(host.Window, "ConfirmAsepriteImportButton");
        Pump();
        var asset = host.Workspace.Current!.CustomAssets.Last();
        Assert.Equal(["walk-down", "blink", "icon"], asset.Animations.OrEmpty().Select(clip => clip.Name));
        Assert.StartsWith("data:image/png;base64,", asset.DataUrl, StringComparison.Ordinal);
        // Changing the file imports it again into the same asset, with the creator's names.
        art.ShowSpriteSheetPreview(path);
        Assert.NotNull(FindByName<TextBox>(host.Window, "AsepriteClip-blink"));
    }

    [AvaloniaFact]
    public void ATiledMapBecomesASceneAndReimportKeepsIt()
    {
        using var host = new GameTestHost();
        var path = RustSessionTests.RepoFile("fixtures", "import", "tiled", "farm.tmx");
        var result = SourceImportFiles.ImportMap(host.Workspace.Current!, path, null);
        Assert.True(result.Created);
        // The fixture has no tileset image: tiles without a terrain property are reported.
        Assert.Contains(result.Report, problem => problem.Code == "import.tilesetImage");
        Assert.True(host.Workspace.Apply(Edits.ReplaceProject(result.Project)));
        var again = SourceImportFiles.ImportMap(host.Workspace.Current!, path, null);
        Assert.False(again.Created);
        Assert.Equal(result.SceneId, again.SceneId);
        Assert.Contains("Re-imported", SourceImportFiles.Summary(again), StringComparison.Ordinal);
    }

    [AvaloniaFact]
    public void TheSkinFormSavesAndResets()
    {
        using var host = new GameTestHost();
        var font = File.ReadAllBytes(RustSessionTests.RepoFile("assets", "fonts", "AtkinsonHyperlegible-Regular.ttf"));
        FindByName<TabControl>(host.Window, "EditorTabs").SelectedIndex = 7;
        Pump();
        var panel = FindByName<SkinEditorPanel>(host.Window, "SkinEditor");
        panel.ImportFont(host.Workspace.Current!, "Atkinson.ttf", font);
        var fontId = host.Workspace.Current!.CustomAssets.Single(asset => asset.Type == CustomAssetTypes.Font).Id;
        var body = FindByName<ComboBox>(host.Window, "SkinFont-body");
        body.SelectedItem = body.Items.OfType<ComboBoxItem>().Single(item => Equals(item.Tag, fontId));
        FindByName<TextBox>(host.Window, "SkinColor-text").Text = "#102030";
        Press(host.Window, "SaveSkinButton");
        var skin = host.Workspace.Current!.UiSkin!.Value;
        Assert.Equal(fontId, skin.BodyFont!.Value);
        Assert.Equal("#102030", SkinForm.Color(skin, "text"));
        Assert.Throws<ArgumentException>(() => panel.ImportFont(host.Workspace.Current!, "nope.ttf", [1, 2, 3]));
        Press(host.Window, "ResetSkinButton");
        Assert.Null(host.Workspace.Current!.UiSkin);
    }
}
