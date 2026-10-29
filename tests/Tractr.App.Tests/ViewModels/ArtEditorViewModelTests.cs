using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using SkiaSharp;
using Tractr.App.ViewModels.EditMode;

namespace Tractr.App.Tests.ViewModels;

public sealed class ArtEditorViewModelTests
{
    private static byte[] Png()
    {
        using var bitmap = new SKBitmap(32, 16);
        bitmap.Erase(SKColors.SeaGreen);
        using var image = SKImage.FromBitmap(bitmap);
        using var encoded = image.Encode(SKEncodedImageFormat.Png, 100);
        return encoded.ToArray();
    }

    [Fact]
    public async Task BatchImportDecodesOffTheViewAndKeepsOneUndoStep()
    {
        using var test = new TestWorkspace();
        var model = new ArtEditorViewModel(test.Workspace);
        var before = test.Project.CustomAssets.Length;
        await model.ImportAsync([("first.png", Png()), ("bad.png", [1, 2, 3]), ("second.png", Png())], CustomAssetTypes.Art);
        Assert.Equal(before + 2, test.Project.CustomAssets.Length);
        Assert.Equal("second.png", model.SelectedAsset!.Name);
        Assert.Contains("bad.png", model.Message, StringComparison.Ordinal);
        test.Workspace.Undo();
        Assert.Equal(before, test.Project.CustomAssets.Length);
    }

    [Fact]
    public void ClipsAndBindingsAreEditedWithoutControls()
    {
        using var test = new TestWorkspace();
        var model = new ArtEditorViewModel(test.Workspace);
        model.Import([("sheet.png", Png())], CustomAssetTypes.Art);
        var clip = model.Slice(16, 16, 6, "walk", true);
        Assert.Equal(2, clip.Frames.Length);
        model.MoveFrame(clip, 1);
        Assert.NotNull(model.SelectedAsset);
        Assert.NotNull(model.SelectedAsset.Animations);
        Assert.Equal(16, model.SelectedAsset.Animations.Value[0].Frames[0].X);
        var visual = model.Visual(model.SelectedAssetId!, "walk", null, 0, 0);
        model.Bind("player", "", visual);
        Assert.NotNull(test.Project.PlayerVisual);
        Assert.Equal(model.SelectedAssetId, test.Project.PlayerVisual.Value.AssetId);
        Assert.Throws<FormatException>(() => model.Slice(0, 16, 6, "bad", false));
        Assert.Throws<FormatException>(() => model.Visual(model.SelectedAssetId!, null, 16, int.MaxValue, 0));
        var frame = ArtEditorViewModel.Frame(17, 1, 16, 16, 6, 32, 16, snap: true);
        Assert.Equal(16, frame.X);
        Assert.Equal(0, frame.Y);
    }

    [Fact]
    public async Task RetiredStudioCannotApplyAnImport()
    {
        using var test = new TestWorkspace();
        var model = new ArtEditorViewModel(test.Workspace);
        var before = test.Project.CustomAssets.Length;
        model.Retire();
        await model.ImportAsync([("sheet.png", Png())], CustomAssetTypes.Art);
        Assert.Equal(before, test.Project.CustomAssets.Length);
    }
}
