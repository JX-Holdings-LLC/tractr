using System.ComponentModel;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Data;
using Avalonia.Platform.Storage;
using FarmingRpgMaker.App.Projects;
using FarmingRpgMaker.App.ViewModels.EditMode;

namespace FarmingRpgMaker.App.Game;

/// <summary>
/// The Mods tab: shows its <see cref="ModsViewModel"/> (installed packs, the review, the
/// registry, the export choices) and picks and saves the pack files.
/// </summary>
public sealed class ModsEditorView : UserControl, IRetirable
{
    private static readonly FilePickerFileType PackFiles = new("Content pack JSON") { Patterns = ["*.json"], MimeTypes = ["application/json"] };
    private readonly StackPanel _installed = new() { Name = "InstalledPacks", Spacing = 8 };
    private readonly StackPanel _review = new() { Name = "PackReview", Spacing = 8 };
    private readonly StackPanel _registry = new() { Name = "PackRegistry", Spacing = 6 };
    private readonly StackPanel _exportCategories = new() { Name = "ExportCategories", Spacing = 4 };

    public ModsEditorView(ProjectWorkspace workspace)
    {
        ViewModel = new ModsViewModel(workspace, () => IsEffectivelyVisible);
        DataContext = ViewModel;
        Name = "ModsEditorView";
        var message = Shown(Ui.Wrapped("", "muted", "small"), nameof(ModsViewModel.Message));
        message.Name = "ModsMessage";
        var layout = new StackPanel { Spacing = 12, Margin = new Thickness(20), MaxWidth = 900 };
        layout.Children.Add(Ui.Text("INSTALLED PACKS", "section"));
        layout.Children.Add(_installed);
        layout.Children.Add(Ui.Text("ADD PACK", "section"));
        var choose = Ui.AsyncButton("Choose pack JSON", ChoosePackAsync, ViewModel.PackReadFailed, "tool");
        choose.Name = "ChoosePackButton";
        layout.Children.Add(choose);
        layout.Children.Add(message);
        layout.Children.Add(_review);
        var install = Ui.CommandButton("Install reviewed pack", ViewModel.InstallCommand, "accent");
        install.Name = "InstallPackButton";
        layout.Children.Add(install);
        layout.Children.Add(Ui.Text("REGISTRY", "section"));
        layout.Children.Add(Ui.Wrapped("Curated packs that ship with the editor. Install opens the same review as a pack file.", "muted", "small"));
        layout.Children.Add(_registry);
        layout.Children.Add(Ui.Text("EXPORT SELECTION AS PACK", "section"));
        layout.Children.Add(Ui.Wrapped("Turn what you built here into a shareable content pack. Tick a type to include all of it, or open it to pick entries.", "muted", "small"));
        var exportName = new TextBox { Name = "ExportPackName", MinWidth = 220 };
        exportName.Bind(TextBox.TextProperty, new Binding(nameof(ModsViewModel.ExportName)) { Mode = BindingMode.TwoWay });
        Ui.Label((exportName, "Pack name"));
        layout.Children.Add(Ui.HStack(8, Ui.Text("Pack name", "muted", "small"), exportName));
        layout.Children.Add(_exportCategories);
        var export = Ui.AsyncButton("Save pack JSON…", ExportSelectionAsync, ViewModel.ExportFailed, "accent");
        export.Name = "ExportPackButton";
        layout.Children.Add(export);
        var exportMessage = Shown(Ui.Wrapped("", "muted", "small"), nameof(ModsViewModel.ExportMessage));
        exportMessage.Name = "ExportPackMessage";
        layout.Children.Add(exportMessage);
        Content = new ScrollViewer { Content = layout };
        ViewModel.PropertyChanged += OnViewModelChanged;
        ShowInstalled();
        ShowRegistry();
        ShowExportCategories();
    }

    public ModsViewModel ViewModel { get; }

    /// <summary>Stops following the project (the editor that built this view was replaced).</summary>
    public void Retire() => ViewModel.Retire();

    public void Refresh() => ViewModel.Refresh();

    private static TextBlock Shown(TextBlock block, string property)
    {
        block.Bind(TextBlock.TextProperty, new Binding(property));
        return block;
    }

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs e)
    {
        switch (e.PropertyName)
        {
            case nameof(ModsViewModel.Installed): ShowInstalled(); break;
            case nameof(ModsViewModel.Registry): ShowRegistry(); break;
            case nameof(ModsViewModel.ExportCategories): ShowExportCategories(); break;
            case nameof(ModsViewModel.Review): ShowReview(); break;
        }
    }

    private void ShowReview()
    {
        _review.Children.Clear();
        if (ViewModel.Review is not { } review) return;
        _review.Children.Add(Ui.Text(review.Title, "h2"));
        _review.Children.Add(Ui.Wrapped(review.Byline, "muted", "small"));
        _review.Children.Add(Ui.Wrapped(review.Engine, "muted", "small"));
        _review.Children.Add(Ui.Wrapped(review.Permissions, "small"));
        if (review.PluginPermissions is { } plugins) _review.Children.Add(Ui.Wrapped(plugins, "small"));
        if (review.Minigames is { } minigames) _review.Children.Add(Ui.Wrapped(minigames, "small"));
        if (review.Dependencies is { } dependencies) _review.Children.Add(Ui.Wrapped(dependencies, "muted", "small"));
        if (review.Overrides is { } overrides) _review.Children.Add(Ui.Wrapped(overrides, "muted", "small"));
        _review.Children.Add(Ui.Wrapped(review.Content, "small"));
        if (review.Art is { } artText)
        {
            var art = Ui.Wrapped(artText, "small");
            art.Name = "PackReviewArt";
            _review.Children.Add(art);
        }

        if (review.ArtConflicts is { } conflicts)
        {
            var clash = Ui.Wrapped(conflicts, "small");
            clash.Name = "PackReviewArtConflicts";
            _review.Children.Add(clash);
        }

        foreach (var plugin in review.Plugins)
        {
            _review.Children.Add(Ui.Text(plugin.Header, "section"));
            var source = new TextBox { Text = plugin.Source, IsReadOnly = true, AcceptsReturn = true, MinHeight = 80, MaxHeight = 180, TextWrapping = Avalonia.Media.TextWrapping.Wrap };
            Ui.Label((source, $"Plugin source: {plugin.Id}"));
            _review.Children.Add(source);
        }
    }

    private async Task ChoosePackAsync()
    {
        if (TopLevel.GetTopLevel(this)?.StorageProvider is not { CanOpen: true } storage) return;
        var files = await storage.OpenFilePickerAsync(new FilePickerOpenOptions
        {
            Title = "Install content pack",
            AllowMultiple = false,
            FileTypeFilter = [PackFiles, FilePickerFileTypes.All],
        });
        if (files.Count == 0) return;
        try
        {
            ViewModel.ReviewPackJson(await PickedFiles.ReadTextAsync(files[0], PickedFiles.MaxPackBytes, "content packs"));
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            ViewModel.PackOpenFailed(error);
        }
    }

    private void ShowRegistry()
    {
        _registry.Children.Clear();
        foreach (var entry in ViewModel.Registry)
        {
            var pack = entry.Pack;
            var install = Ui.Button(entry.Installed ? "Installed" : "Install", () => ViewModel.ReviewPack(pack), "tool", "small");
            install.Name = $"RegistryInstall_{entry.Id}";
            install.IsEnabled = !entry.Installed;
            var text = Ui.VStack(2, Ui.Text(entry.Title, "h3"), Ui.Wrapped(entry.Description, "muted", "small"));
            _registry.Children.Add(Ui.Row(text, install));
        }
    }

    private void ShowExportCategories()
    {
        _exportCategories.Children.Clear();
        foreach (var category in ViewModel.ExportCategories)
        {
            var key = category.Key;
            var include = new CheckBox
            {
                Name = $"ExportCategory_{key}",
                Content = category.Label,
                IsChecked = category.Included,
                IsEnabled = category.Count > 0,
            };
            var entries = new StackPanel { Spacing = 2, Margin = new Thickness(24, 0, 0, 0) };
            foreach (var option in category.Entries)
            {
                var id = option.Id;
                var pick = new CheckBox { Name = $"ExportEntry_{key}_{id}", Content = option.Label, IsChecked = option.Included };
                pick.Click += (_, _) => ViewModel.SetEntryIncluded(key, id, pick.IsChecked == true);
                entries.Children.Add(pick);
            }

            include.Click += (_, _) => ViewModel.SetCategoryIncluded(key, include.IsChecked == true);
            var expander = new Expander { Name = $"ExportEntries_{key}", Header = "Entries", Content = entries, IsEnabled = category.Count > 0 };
            _exportCategories.Children.Add(Ui.HStack(8, include, expander));
        }
    }

    private async Task ExportSelectionAsync()
    {
        if (ViewModel.PrepareExport() is not { } result) return;
        if (TopLevel.GetTopLevel(this)?.StorageProvider is not { CanSave: true } storage)
        {
            ViewModel.ExportUnavailable();
            return;
        }

        try
        {
            var file = await storage.SaveFilePickerAsync(new FilePickerSaveOptions
            {
                Title = "Save content pack",
                SuggestedFileName = result.FileName,
                DefaultExtension = "json",
                FileTypeChoices = [PackFiles],
                ShowOverwritePrompt = true,
            });
            if (file is null) return;
            await PickedFiles.WriteTextAsync(file, result.Text);
            ViewModel.Exported(file.Name, result);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            ViewModel.ExportFailed(error);
        }
    }

    private void ShowInstalled()
    {
        _installed.Children.Clear();
        if (!ViewModel.HasProject) return;
        if (ViewModel.Installed.Count == 0)
        {
            _installed.Children.Add(Ui.Wrapped("No packs installed.", "muted", "small"));
            return;
        }

        foreach (var pack in ViewModel.Installed)
        {
            var id = pack.Id;
            var header = Ui.Text(pack.Header, "h3");
            var details = Ui.Wrapped(pack.Details, "muted", "small");
            var enabled = new CheckBox { Name = $"PackEnabled_{id}", Content = "Enabled", IsChecked = pack.Enabled };
            enabled.Click += (_, _) => ViewModel.SetPackEnabled(id, enabled.IsChecked == true);
            var up = Ui.Button("↑", () => ViewModel.MovePack(id, -1), "tool", "small");
            up.Name = $"MovePackUp_{id}";
            up.IsEnabled = pack.CanMoveUp;
            var down = Ui.Button("↓", () => ViewModel.MovePack(id, 1), "tool", "small");
            down.Name = $"MovePackDown_{id}";
            down.IsEnabled = pack.CanMoveDown;
            Ui.Label((up, $"Load {pack.Name} earlier"), (down, $"Load {pack.Name} later"), (enabled, $"{pack.Name} enabled"));
            var import = Ui.Button("Import into project", () => ViewModel.ImportPack(id), "tool", "small");
            import.Name = $"ImportPack_{id}";
            var remove = Ui.Button("Remove", () => ViewModel.RemovePack(id), "tool", "small");
            remove.Name = $"RemovePack_{id}";
            var row = Ui.VStack(6, header, details, Ui.HStack(8, enabled, up, down, import, remove));
            _installed.Children.Add(new Border { Child = row }.WithClasses("row"));
        }
    }
}
