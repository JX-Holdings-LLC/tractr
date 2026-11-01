using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Platform.Storage;
using Avalonia.Threading;
using FarmEngine.Authoring;
using FarmEngine.Schemas;

namespace Tractr.App.Game;

/// <summary>
/// The Art tab's Aseprite import (#232): pick a <c>.aseprite</c> file or a sheet export's JSON,
/// check the tag → clip names in the preview, import; the file is then watched and imported
/// again whenever it changes, into the same asset.
/// </summary>
public sealed partial class ArtEditorView
{
    private static readonly FilePickerFileType SpriteSheetFiles = new("Aseprite sprite sheets") { Patterns = [.. SourceImports.SpriteSheetPatterns] };
    private readonly Border _asepritePreview = new() { Name = "AsepritePreview", IsVisible = false };
    private readonly ComboBox _asepriteType = new() { Name = "AsepriteAssetType" };
    private readonly Dictionary<string, TextBox> _asepriteNames = [];
    private SourceImportFiles.SpriteSheetFile? _asepriteFile;
    /// <summary>The watched source files, by full path.</summary>
    private readonly Dictionary<string, FileSystemWatcher> _watchers = new(StringComparer.Ordinal);
    private readonly HashSet<string> _changedSources = new(StringComparer.Ordinal);
    private readonly DispatcherTimer _reimportTimer = new() { Interval = TimeSpan.FromMilliseconds(400) };

    private Control BuildAsepriteImport()
    {
        foreach (var type in new[] { CustomAssetTypes.Npc, CustomAssetTypes.Player, CustomAssetTypes.Item, CustomAssetTypes.Tile, CustomAssetTypes.Art })
            _asepriteType.Items.Add(new ComboBoxItem { Content = type, Tag = type });
        _asepriteType.SelectedIndex = 0;
        AutomationProperties.SetName(_asepriteType, "Asset type");
        _reimportTimer.Tick += (_, _) => ReimportChanged();
        var import = Ui.AsyncButton("Import Aseprite…", PickSpriteSheetAsync, error => _model.Message = $"Could not import: {error.Message}", "tool");
        import.Name = "ImportAsepriteButton";
        ToolTip.SetTip(import, "A .aseprite file, or the JSON of File ▸ Export Sprite Sheet with its PNG: tags become clips, slices single frames.");
        var panel = new StackPanel { Spacing = 6 };
        panel.Children.Add(import);
        panel.Children.Add(_asepritePreview);
        return panel;
    }

    private async Task PickSpriteSheetAsync()
    {
        if (TopLevel.GetTopLevel(this)?.StorageProvider is not { CanOpen: true } storage) return;
        var picked = await storage.OpenFilePickerAsync(new FilePickerOpenOptions { Title = "Import an Aseprite sprite sheet", FileTypeFilter = [SpriteSheetFiles, FilePickerFileTypes.All] });
        if (picked.Count == 0) return;
        if (picked[0].TryGetLocalPath() is not { } path)
        {
            _model.Message = "Choose a file on this computer: re-import watches it for changes.";
            return;
        }

        ShowSpriteSheetPreview(path);
    }

    /// <summary>Reads <paramref name="path"/> and shows its tags and slices with the clips they become.</summary>
    public void ShowSpriteSheetPreview(string path)
    {
        if (_workspace.Current is not { } project) return;
        try
        {
            _asepriteFile = SourceImportFiles.LoadSpriteSheet(project, path);
        }
        catch (Exception error) when (error is IOException or ArgumentException or UnauthorizedAccessException)
        {
            _model.Message = $"Could not import: {error.Message}";
            return;
        }

        var mappings = SourceImportFiles.Preview(project, _asepriteFile, new Dictionary<string, string>());
        var list = new StackPanel { Spacing = 4 };
        list.Children.Add(Ui.Text(Path.GetFileName(path), "section"));
        list.Children.Add(Ui.Wrapped("Each tag becomes a clip (slices become single frames). Rename a clip here; the next import keeps your names.", "muted", "small"));
        _asepriteNames.Clear();
        foreach (var mapping in mappings)
        {
            var key = mapping.IsSlice ? "slice:" + mapping.Source : mapping.Source;
            var box = new TextBox { Text = mapping.Clip, Width = 130, Name = "AsepriteClip-" + mapping.Clip };
            AutomationProperties.SetName(box, $"Clip for {mapping.Source}");
            _asepriteNames[key] = box;
            var status = mapping.Status switch { "update" => "replaces a clip", "kept" => "updated", _ => "new" };
            list.Children.Add(Ui.HStack(6, Ui.Text(mapping.Source, "small"), Ui.Text("→", "muted"), box, Ui.Text($"{mapping.Frames} frame{(mapping.Frames == 1 ? "" : "s")} · {status}", "muted", "small")));
        }

        foreach (var warning in _asepriteFile.Sheet.Warnings) list.Children.Add(Ui.Wrapped(warning, "muted", "small"));
        // The type picker moves from the last preview to this one.
        if (_asepriteType.Parent is Panel previous) previous.Children.Remove(_asepriteType);
        list.Children.Add(Ui.HStack(6, Ui.Text("New asset type", "muted", "small"), _asepriteType));
        var confirm = Ui.Button("Import", ImportSpriteSheet, "accent");
        confirm.Name = "ConfirmAsepriteImportButton";
        var cancel = Ui.Button("Cancel", () => _asepritePreview.IsVisible = false, "tool");
        list.Children.Add(Ui.HStack(8, confirm, cancel));
        _asepritePreview.Child = list;
        _asepritePreview.IsVisible = true;
    }

    /// <summary>Imports the previewed sheet with the names in the preview.</summary>
    public void ImportSpriteSheet()
    {
        if (_workspace.Current is not { } project || _asepriteFile is not { } file) return;
        var names = _asepriteNames.ToDictionary(pair => pair.Key, pair => pair.Value.Text ?? "");
        var type = (_asepriteType.SelectedItem as ComboBoxItem)?.Tag as string ?? CustomAssetTypes.Art;
        var result = SourceImportFiles.ImportSpriteSheet(project, file, type, names);
        _workspace.Apply(Edits.ReplaceProject(result.Project));
        _asepritePreview.IsVisible = false;
        _model.SelectedAssetId = result.AssetId;
        _model.Message = SpriteSheetSummary(result);
        Refresh();
    }

    private static string SpriteSheetSummary(AsepriteImportResult result)
    {
        var parts = new List<string> { result.Created ? "Imported the sprite sheet." : "Re-imported the sprite sheet." };
        if (result.Added.Any()) parts.Add("New clips: " + string.Join(", ", result.Added) + ".");
        if (result.Updated.Any()) parts.Add("Updated: " + string.Join(", ", result.Updated) + ".");
        if (result.Removed.Any()) parts.Add("Removed (tag gone): " + string.Join(", ", result.Removed) + ".");
        parts.AddRange(result.Warnings);
        return string.Join(" ", parts);
    }

    /// <summary>Watches every file the project's art was imported from (called on refresh).</summary>
    private void SyncWatchers()
    {
        var wanted = _workspace.Current is { } project
            ? SourceImports.Sources(project, ImportKinds.Aseprite).Select(source => source.Path).ToHashSet(StringComparer.Ordinal)
            : [];
        foreach (var path in _watchers.Keys.Where(path => !wanted.Contains(path)).ToList())
        {
            _watchers[path].Dispose();
            _watchers.Remove(path);
        }

        foreach (var path in wanted.Where(path => !_watchers.ContainsKey(path)))
        {
            var directory = Path.GetDirectoryName(path);
            if (string.IsNullOrEmpty(directory) || !Directory.Exists(directory)) continue;
            try
            {
                var watcher = new FileSystemWatcher(directory, Path.GetFileName(path)) { NotifyFilter = NotifyFilters.LastWrite | NotifyFilters.Size | NotifyFilters.FileName };
                watcher.Changed += (_, _) => Dispatcher.UIThread.Post(() => QueueReimport(path));
                watcher.Created += (_, _) => Dispatcher.UIThread.Post(() => QueueReimport(path));
                watcher.Renamed += (_, _) => Dispatcher.UIThread.Post(() => QueueReimport(path));
                watcher.EnableRaisingEvents = true;
                _watchers[path] = watcher;
            }
            catch (Exception error) when (error is IOException or ArgumentException or PlatformNotSupportedException)
            {
                // Without a watcher the creator imports again by hand.
            }
        }
    }

    private void QueueReimport(string path)
    {
        _changedSources.Add(path);
        // Editors write a file in several steps: wait until it has been quiet for a moment.
        _reimportTimer.Stop();
        _reimportTimer.Start();
    }

    /// <summary>Imports every changed source file again, keeping its clip names.</summary>
    public void ReimportChanged()
    {
        _reimportTimer.Stop();
        var changed = _changedSources.ToList();
        _changedSources.Clear();
        foreach (var path in changed)
        {
            if (_workspace.Current is not { } project) return;
            try
            {
                var file = SourceImportFiles.LoadSpriteSheet(project, path);
                var result = SourceImportFiles.ImportSpriteSheet(project, file, CustomAssetTypes.Art, new Dictionary<string, string>());
                _workspace.Apply(Edits.ReplaceProject(result.Project));
                _model.Message = $"{Path.GetFileName(path)} changed. " + SpriteSheetSummary(result);
            }
            catch (Exception error) when (error is IOException or ArgumentException or UnauthorizedAccessException)
            {
                _model.Message = $"{Path.GetFileName(path)} changed but could not be imported: {error.Message}";
            }
        }

        Refresh();
    }

    private void StopWatching()
    {
        _reimportTimer.Stop();
        foreach (var watcher in _watchers.Values) watcher.Dispose();
        _watchers.Clear();
    }
}
