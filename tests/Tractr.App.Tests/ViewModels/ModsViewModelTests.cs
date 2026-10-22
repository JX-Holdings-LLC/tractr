using FarmEngine.Authoring;
using Tractr.App.ViewModels.EditMode;

namespace Tractr.App.Tests.ViewModels;

/// <summary>The Mods tab's review, install, load order and export rules, driven without the editor.</summary>
public sealed class ModsViewModelTests
{
    private static string DemoModJson() => File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "demo-mod.json"));

    [Fact]
    public void APackIsReviewedBeforeItIsInstalled()
    {
        using var test = new TestWorkspace();
        var viewModel = new ModsViewModel(test.Workspace);
        Assert.Equal("Choose a content pack JSON file to review it.", viewModel.Message);
        Assert.Empty(viewModel.Installed);
        Assert.False(viewModel.InstallCommand.CanExecute(null));

        viewModel.ReviewPackJson("{ not json");
        Assert.StartsWith("Pack JSON could not be read: ", viewModel.Message, StringComparison.Ordinal);
        viewModel.ReviewPackJson("{}");
        Assert.StartsWith("Pack is invalid: ", viewModel.Message, StringComparison.Ordinal);
        Assert.Null(viewModel.Review);

        viewModel.ReviewPackJson(DemoModJson());
        var review = Assert.IsType<PackReview>(viewModel.Review);
        Assert.StartsWith("Permissions: content ", review.Permissions, StringComparison.Ordinal);
        Assert.StartsWith("Content: ", review.Content, StringComparison.Ordinal);
        Assert.StartsWith("Review ", viewModel.Message, StringComparison.Ordinal);
        Assert.True(viewModel.InstallCommand.CanExecute(null));
        Assert.Empty(test.Project.ContentPacks);

        viewModel.InstallCommand.Execute(null);
        var installed = Assert.Single(test.Project.ContentPacks);
        Assert.StartsWith("Installed ", viewModel.Message, StringComparison.Ordinal);
        Assert.Null(viewModel.Review);
        Assert.False(viewModel.InstallCommand.CanExecute(null));
        var row = Assert.Single(viewModel.Installed);
        Assert.Equal(installed.Pack.Manifest.Id, row.Id);
        Assert.True(row.Enabled);
        Assert.False(row.CanMoveUp || row.CanMoveDown);

        // The same pack again is refused before the review.
        viewModel.ReviewPackJson(DemoModJson());
        Assert.Equal($"{row.Id} is already installed.", viewModel.Message);
        Assert.Null(viewModel.Review);

        // Enabled, selected from Problems, removed.
        viewModel.SetPackEnabled(row.Id, false);
        Assert.False(Assert.Single(viewModel.Installed).Enabled);
        viewModel.SelectPack(row.Id);
        Assert.EndsWith("  ←", Assert.Single(viewModel.Installed).Header, StringComparison.Ordinal);
        viewModel.RemovePack(row.Id);
        Assert.Empty(viewModel.Installed);
        Assert.Empty(test.Project.ContentPacks);
    }

    [Fact]
    public void TheReviewNamesTheMinigameKindsAPackDefines()
    {
        using var test = new TestWorkspace();
        var viewModel = new ModsViewModel(test.Workspace);
        viewModel.ReviewPackJson(DemoModJson());
        Assert.Null(Assert.IsType<PackReview>(viewModel.Review).Minigames);

        viewModel.ReviewPackJson(File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "lantern-night-mod.json")));
        var minigames = Assert.IsType<PackReview>(viewModel.Review).Minigames;
        Assert.NotNull(minigames);
        Assert.StartsWith("Minigames: ", minigames, StringComparison.Ordinal);
        Assert.Contains("lantern-catch", minigames, StringComparison.Ordinal);
    }

    [Fact]
    public void RegistryPacksLoadInTheOrderTheCreatorSets()
    {
        using var test = new TestWorkspace();
        var viewModel = new ModsViewModel(test.Workspace);
        Assert.True(viewModel.Registry.Count >= 2);
        foreach (var entry in viewModel.Registry.Take(2))
        {
            Assert.False(entry.Installed);
            viewModel.ReviewPack(entry.Pack);
            viewModel.InstallCommand.Execute(null);
        }

        Assert.All(viewModel.Registry.Take(2), entry => Assert.True(entry.Installed));
        var first = viewModel.Installed[0];
        Assert.False(first.CanMoveUp);
        Assert.True(first.CanMoveDown);
        viewModel.MovePack(first.Id, 1);
        Assert.Equal(first.Id, viewModel.Installed[1].Id);
        Assert.True(viewModel.Installed[1].CanMoveUp);
    }

    [Fact]
    public void ExportTakesTheTickedTypesLessTheUntickedEntries()
    {
        using var test = new TestWorkspace();
        var viewModel = new ModsViewModel(test.Workspace);
        Assert.Contains(viewModel.ExportCategories, category => category.Key == "items" && category.Included);
        Assert.Contains(viewModel.ExportCategories, category => category.Key == "npcs" && !category.Included);

        viewModel.ExportName = "Starter Goods";
        viewModel.SetCategoryIncluded("recipes", false);
        viewModel.SetEntryIncluded("items", "material-wood", false);
        var result = Assert.IsType<PackExportResult>(viewModel.PrepareExport());
        Assert.Equal("starter-goods.json", result.FileName);
        Assert.DoesNotContain(result.Pack!.Content.Items, item => item.Id == "material-wood");
        Assert.Empty(result.Pack.Content.Recipes);
        viewModel.Exported("starter-goods.json", result);
        Assert.Equal(ModsViewModel.ExportedText("starter-goods.json", result), viewModel.ExportMessage);

        foreach (var category in viewModel.ExportCategories) viewModel.SetCategoryIncluded(category.Key, false);
        Assert.Null(viewModel.PrepareExport());
        Assert.StartsWith("Could not export: ", viewModel.ExportMessage, StringComparison.Ordinal);
        viewModel.ExportUnavailable();
        Assert.Equal("Saving files is not available here.", viewModel.ExportMessage);
        viewModel.ExportFailed(new IOException("disk full"));
        Assert.Equal("Could not save the pack: disk full", viewModel.ExportMessage);
    }

    [Fact]
    public void AHiddenTabCatchesUpWhenItIsOpened()
    {
        using var test = new TestWorkspace();
        var shown = false;
        var viewModel = new ModsViewModel(test.Workspace, () => shown);
        viewModel.ReviewPack(viewModel.Registry[0].Pack);
        viewModel.InstallCommand.Execute(null);
        Assert.Empty(viewModel.Installed);
        shown = true;
        viewModel.Refresh();
        Assert.Single(viewModel.Installed);
    }
}
