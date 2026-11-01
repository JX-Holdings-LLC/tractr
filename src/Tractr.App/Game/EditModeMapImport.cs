using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Platform.Storage;
using FarmEngine.Authoring;
using FarmEngine.Schemas;

namespace Tractr.App.Game;

/// <summary>
/// The scene panel's map import (#233): a Tiled map (<c>.tmx</c>, <c>.tmj</c>) or an LDtk level
/// becomes a scene, and Re-import map reads the file again over the scene it made.
/// </summary>
public sealed partial class EditModeView
{
    private static readonly FilePickerFileType MapFiles = new("Tiled and LDtk maps") { Patterns = [.. SourceImports.MapPatterns] };
    private readonly ComboBox _mapLevels = new() { Name = "MapImportLevel", IsVisible = false, MinWidth = 140 };
    private readonly Button _importLevel = new() { Name = "ImportMapLevelButton", Content = "Import level", IsVisible = false };
    private string? _pickedMap;

    private Control BuildMapImport()
    {
        var import = Ui.AsyncButton("Import map…", PickMapAsync, error => ViewModel.Message = $"Could not import: {error.Message}", "tool");
        import.Name = "ImportMapButton";
        ToolTip.SetTip(import, "A Tiled map (.tmx, .tmj) or an LDtk level becomes a scene; tile properties set terrains, objects set doors, NPCs and markers.");
        var reimport = NamedButton("ReimportMapButton", "Re-import map", ReimportMap);
        ToolTip.SetTip(reimport, "Read the scene's map file again: tiles and markers update, crops and soil stay.");
        AutomationProperties.SetName(_mapLevels, "LDtk level");
        _importLevel.Classes.Add("tool");
        _importLevel.Click += (_, _) =>
        {
            if (_pickedMap is { } path && _mapLevels.SelectedItem is string level) ImportMap(path, level);
        };
        var row = WrapButtons(import, reimport, _importLevel);
        row.Children.Insert(2, _mapLevels);
        return row;
    }

    private async Task PickMapAsync()
    {
        if (TopLevel.GetTopLevel(this)?.StorageProvider is not { CanOpen: true } storage) return;
        var picked = await storage.OpenFilePickerAsync(new FilePickerOpenOptions { Title = "Import a map", FileTypeFilter = [MapFiles, FilePickerFileTypes.All] });
        if (picked.Count == 0) return;
        if (picked[0].TryGetLocalPath() is not { } path)
        {
            ViewModel.Message = "Choose a file on this computer: its tilesets are read from beside it.";
            return;
        }

        var levels = SourceImportFiles.Levels(path);
        if (levels.Count > 1)
        {
            _pickedMap = path;
            _mapLevels.ItemsSource = levels;
            _mapLevels.SelectedIndex = 0;
            _mapLevels.IsVisible = _importLevel.IsVisible = true;
            ViewModel.Message = $"{Path.GetFileName(path)} has {levels.Count} levels: choose one and Import level.";
            return;
        }

        ImportMap(path, levels.Count == 1 ? levels[0] : null);
    }

    /// <summary>Imports <paramref name="path"/> (an LDtk <paramref name="level"/>) as a scene and shows it.</summary>
    public void ImportMap(string path, string? level)
    {
        if (_workspace.Current is not { } project) return;
        _mapLevels.IsVisible = _importLevel.IsVisible = false;
        try
        {
            var result = SourceImportFiles.ImportMap(project, path, level);
            _workspace.Apply(Edits.ReplaceProject(result.Project));
            SelectScene(result.SceneId);
            ViewModel.Message = SourceImportFiles.Summary(result);
        }
        catch (Exception error) when (error is IOException or ArgumentException or UnauthorizedAccessException)
        {
            ViewModel.Message = $"Could not import {Path.GetFileName(path)}: {error.Message}";
        }
    }

    private void ReimportMap()
    {
        if (_workspace.Current is not { } project || ViewModel.SceneId is not { } sceneId) return;
        if (SourceImports.SceneSource(project, sceneId) is not { } source)
        {
            ViewModel.Message = "This scene wasn't imported from a map file.";
            return;
        }

        ImportMap(source.Path, source.Level?.Value);
    }
}
