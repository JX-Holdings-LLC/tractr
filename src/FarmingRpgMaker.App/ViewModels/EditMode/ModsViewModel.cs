using System.Text.Json;
using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using FarmingRpgMaker.App.Mvvm;
using FarmingRpgMaker.App.Projects;

namespace FarmingRpgMaker.App.ViewModels.EditMode;

/// <summary>An installed pack's row: load order buttons, enabled, import and remove.</summary>
public sealed record InstalledPackRow(string Id, string Name, string Header, string Details, bool Enabled, bool CanMoveUp, bool CanMoveDown);

/// <summary>A curated registry pack and whether this project has it.</summary>
public sealed record RegistryPackRow(string Id, string Title, string Description, bool Installed, ContentPack Pack);

/// <summary>One entry the export can leave out.</summary>
public sealed record ExportEntryRow(string Id, string Label, bool Included);

/// <summary>A content type the export can include ("Items (3)"), with its entries.</summary>
public sealed record ExportCategoryRow(string Key, string Label, int Count, bool Included, IReadOnlyList<ExportEntryRow> Entries);

/// <summary>A plugin of a pack under review, with the source the creator can read.</summary>
public sealed record PluginReviewRow(string Id, string Header, string Source);

/// <summary>
/// What the review shows of a pack before it is installed: its manifest, permissions,
/// dependencies, content and plugins. Optional lines are null when the pack has nothing to say.
/// </summary>
public sealed record PackReview(
    string Title,
    string Byline,
    string Engine,
    string Permissions,
    string? PluginPermissions,
    string? Dependencies,
    string? Overrides,
    string Content,
    string? Art,
    string? ArtConflicts,
    IReadOnlyList<PluginReviewRow> Plugins);

/// <summary>
/// The Mods tab: installed pack order, an explicit manifest and permission review before
/// installing, the curated registry, and "Export selection as pack" (web ModsEditor). F#
/// validates, reviews and exports packs; the edits go through <see cref="ProjectWorkspace"/>.
/// Picking and saving files stay in the view, which hands the text over.
/// </summary>
public sealed class ModsViewModel : ObservableObject
{
    private readonly ProjectWorkspace _workspace;
    private readonly Func<bool> _isShown;
    private readonly HashSet<string> _exportKeys = [.. Mods.DefaultExportKeys];
    private readonly HashSet<(string Key, string Id)> _exportExcluded = [];
    private IReadOnlyList<InstalledPackRow> _installed = [];
    private IReadOnlyList<RegistryPackRow> _registry = [];
    private IReadOnlyList<ExportCategoryRow> _exportCategories = [];
    private bool _hasProject;
    private ContentPack? _pending;
    private PackReview? _review;
    private string _message = "Choose a content pack JSON file to review it.";
    private string _exportName = "my-pack";
    private string _exportMessage = "";
    private string? _selectedPackId;

    /// <param name="workspace">The open project.</param>
    /// <param name="isShown">Whether the tab is on show: project changes refresh it only then (opening it refreshes too).</param>
    public ModsViewModel(ProjectWorkspace workspace, Func<bool>? isShown = null)
    {
        _workspace = workspace ?? throw new ArgumentNullException(nameof(workspace));
        _isShown = isShown ?? (() => true);
        InstallCommand = new RelayCommand(Install, () => _pending is not null);
        _workspace.ProjectChanged += OnProjectChanged;
        Refresh();
    }

    /// <summary>The project's packs in load order (empty: "No packs installed.").</summary>
    public IReadOnlyList<InstalledPackRow> Installed
    {
        get => _installed;
        private set => SetProperty(ref _installed, value);
    }

    /// <summary>False when no project is open.</summary>
    public bool HasProject
    {
        get => _hasProject;
        private set => SetProperty(ref _hasProject, value);
    }

    /// <summary>The curated packs that ship with the editor.</summary>
    public IReadOnlyList<RegistryPackRow> Registry
    {
        get => _registry;
        private set => SetProperty(ref _registry, value);
    }

    /// <summary>The content types the export can include.</summary>
    public IReadOnlyList<ExportCategoryRow> ExportCategories
    {
        get => _exportCategories;
        private set => SetProperty(ref _exportCategories, value);
    }

    /// <summary>The pack under review (null: none, or it was refused).</summary>
    public PackReview? Review
    {
        get => _review;
        private set => SetProperty(ref _review, value);
    }

    /// <summary>The review's status line: what to do, why a pack was refused, what was installed.</summary>
    public string Message
    {
        get => _message;
        private set => SetProperty(ref _message, value);
    }

    /// <summary>The name of the pack the export makes.</summary>
    public string ExportName
    {
        get => _exportName;
        set => SetProperty(ref _exportName, value ?? "");
    }

    /// <summary>The export's status line.</summary>
    public string ExportMessage
    {
        get => _exportMessage;
        private set => SetProperty(ref _exportMessage, value);
    }

    /// <summary>Installs the reviewed pack; only a reviewed pack can be.</summary>
    public RelayCommand InstallCommand { get; }

    /// <summary>Stops following the project (the editor that built this tab was replaced).</summary>
    public void Retire() => _workspace.ProjectChanged -= OnProjectChanged;

    /// <summary>Marks the pack a Problems row is about.</summary>
    public void SelectPack(string id)
    {
        _selectedPackId = id;
        Refresh();
    }

    /// <summary>Validates a pack file's text (F#) and reviews it, or says why it can't be installed.</summary>
    public void ReviewPackJson(string json)
    {
        ClearReview();
        try
        {
            var parsed = JsonModule.parse(json);
            if (parsed.IsError) throw new JsonException(parsed.ErrorValue);
            var validation = PackRules.validateContentPack(parsed.ResultValue);
            if (validation.IsError)
            {
                Message = $"Pack is invalid: {string.Join("; ", validation.ErrorValue)}";
                return;
            }

            ReviewPack(validation.ResultValue);
        }
        catch (JsonException error)
        {
            Message = $"Pack JSON could not be read: {error.Message}";
        }
    }

    /// <summary>Reviews a validated pack's manifest, permissions and plugins before installing it.</summary>
    public void ReviewPack(ContentPack pack)
    {
        ArgumentNullException.ThrowIfNull(pack);
        ClearReview();
        if (_workspace.Current is { } project && Mods.IsInstalled(project, pack.Manifest.Id))
        {
            Message = $"{pack.Manifest.Id} is already installed.";
            return;
        }

        var manifest = pack.Manifest;
        Message = $"Review {manifest.Name} before installing.";
        var permissions = manifest.Permissions;
        // contentInject off means the pack's content is not loaded; packs cannot add UI panels
        // yet, so uiPanels grants nothing.
        var permissionText = $"Permissions: content {(permissions.ContentInject ? "added to your game" : "off (none of the pack's content loads)")}; "
            + $"hooks {(permissions.Hooks.Length == 0 ? "none" : string.Join(", ", permissions.Hooks))}"
            + (permissions.UiPanels ? "; UI panels requested (not supported, grants nothing)" : "");
        var content = pack.Content;
        string? art = null;
        string? artConflicts = null;
        if (Mods.PackAssets(pack) is { Count: > 0 } assets)
        {
            art = $"Art: {assets.Count} image{(assets.Count == 1 ? "" : "s")}, added to this project's art when you install.";
            if (_workspace.Current is { } current && Mods.PackAssetConflicts(current, pack) is { Count: > 0 } conflicts)
            {
                artConflicts = $"This project already has different art named {string.Join(", ", conflicts)}; it keeps its own, so the pack's entries using those ids look different here.";
            }
        }

        _pending = pack;
        Review = new PackReview(
            $"{manifest.Name} · {manifest.Version}",
            $"{manifest.Description.OrNull() ?? "No description"} · by {manifest.Author.OrNull() ?? "unknown author"}",
            $"Engine: {manifest.EngineCompatibility} · ID: {manifest.Id}",
            permissionText,
            pack.Plugins.Length > 0 ? $"Plugins may: {PackRules.describeMutations(permissions)}" : null,
            manifest.Dependencies.Length > 0 ? $"Dependencies: {string.Join(", ", manifest.Dependencies.Select(dep => $"{dep.PackId} {dep.Version.OrNull() ?? "*"}"))}" : null,
            manifest.Overrides.Length > 0 ? $"Overrides: {string.Join(", ", manifest.Overrides)}" : null,
            $"Content: {content.Items.Length} items, {content.Npcs.Length} NPCs, {content.Scenes.Length} scenes, {content.Recipes.Length} recipes, {content.Quests.Length} quests, {pack.Plugins.Length} plugins",
            art,
            artConflicts,
            [.. pack.Plugins.Select(plugin => new PluginReviewRow(plugin.Id, $"Plugin: {plugin.Name.OrNull() ?? plugin.Id} · hooks {string.Join(", ", plugin.Hooks)}", plugin.Source))]);
        InstallCommand.NotifyCanExecuteChanged();
    }

    /// <summary>The pack file the creator chose could not be read (too big, not text…).</summary>
    public void PackReadFailed(Exception error)
    {
        ArgumentNullException.ThrowIfNull(error);
        Message = $"Could not read the pack: {error.Message}";
    }

    /// <summary>The pack file the creator chose could not be opened.</summary>
    public void PackOpenFailed(Exception error)
    {
        ArgumentNullException.ThrowIfNull(error);
        Message = $"Could not open pack: {error.Message}";
    }

    public void SetPackEnabled(string id, bool enabled) => _workspace.Apply(Edits.SetPackEnabled(id, enabled));

    /// <summary>Loads a pack earlier (<paramref name="delta"/> −1) or later (+1).</summary>
    public void MovePack(string id, int delta)
    {
        if (_workspace.Current is { } project) _workspace.Apply(Edits.MovePack(project, id, delta));
    }

    /// <summary>Copies a pack's content into the project.</summary>
    public void ImportPack(string id) => _workspace.Apply(Edits.ImportPack(id));

    public void RemovePack(string id) => _workspace.Apply(Edits.RemovePack(id));

    /// <summary>Includes or leaves out a whole content type of the export.</summary>
    public void SetCategoryIncluded(string key, bool included)
    {
        if (included) _exportKeys.Add(key);
        else _exportKeys.Remove(key);
    }

    /// <summary>Includes or leaves out one entry of an included type.</summary>
    public void SetEntryIncluded(string key, string id, bool included)
    {
        if (included) _exportExcluded.Remove((key, id));
        else _exportExcluded.Add((key, id));
    }

    /// <summary>The pack the Export button would save: the ticked types, less unticked entries.</summary>
    public PackExportResult BuildSelectedPack()
    {
        var project = _workspace.Current ?? throw new InvalidOperationException("No project is open.");
        var selection = new Dictionary<string, IReadOnlyList<string>>();
        foreach (var category in Mods.ExportCategories(project))
        {
            if (!_exportKeys.Contains(category.Key)) continue;
            selection[category.Key] = category.Entries.Select(entry => entry.Id).Where(id => !_exportExcluded.Contains((category.Key, id))).ToList();
        }

        return Mods.ExportPack(project, _exportName, selection);
    }

    /// <summary>
    /// The pack to save, or null when there is nothing to export or it is refused (the export
    /// line says why).
    /// </summary>
    public PackExportResult? PrepareExport()
    {
        if (_workspace.Current is null) return null;
        var result = BuildSelectedPack();
        if (result.Ok) return result;
        ExportMessage = $"Could not export: {string.Join("; ", result.Errors)}";
        return null;
    }

    /// <summary>The view could not offer a place to save the pack.</summary>
    public void ExportUnavailable() => ExportMessage = "Saving files is not available here.";

    /// <summary>The pack was written to <paramref name="fileName"/>.</summary>
    public void Exported(string fileName, PackExportResult result) => ExportMessage = ExportedText(fileName, result);

    /// <summary>Writing the pack failed.</summary>
    public void ExportFailed(Exception error)
    {
        ArgumentNullException.ThrowIfNull(error);
        ExportMessage = $"Could not save the pack: {error.Message}";
    }

    /// <summary>"Saved my-pack.json with 2 images (the art its entries use)."</summary>
    public static string ExportedText(string fileName, PackExportResult result)
    {
        ArgumentNullException.ThrowIfNull(result);
        return result.Assets.Count switch
        {
            0 => $"Saved {fileName}.",
            1 => $"Saved {fileName} with 1 image (the art its entries use).",
            var count => $"Saved {fileName} with {count} images (the art its entries use).",
        };
    }

    /// <summary>Catches up with the project: installed packs, the registry and the export choices.</summary>
    public void Refresh()
    {
        if (_workspace.Current is not { } project)
        {
            HasProject = false;
            Installed = [];
            return;
        }

        HasProject = true;
        Registry = [.. Mods.Registry.Select(entry =>
            new RegistryPackRow(entry.Id, $"{entry.Name} · by {entry.Author}", entry.Description, Mods.IsInstalled(project, entry.Id), entry.Pack))];
        ExportCategories = [.. Mods.ExportCategories(project).Select(category => new ExportCategoryRow(
            category.Key,
            $"{category.Label} ({category.Entries.Length})",
            category.Entries.Length,
            category.Entries.Length > 0 && _exportKeys.Contains(category.Key),
            [.. category.Entries.Select(entry => new ExportEntryRow(entry.Id, entry.Label, !_exportExcluded.Contains((category.Key, entry.Id))))]))];
        var packs = project.ContentPacks;
        Installed = [.. packs.Select((install, index) =>
        {
            var manifest = install.Pack.Manifest;
            var id = manifest.Id;
            var hooks = manifest.Permissions.Hooks.Length == 0 ? "none" : string.Join(", ", manifest.Permissions.Hooks);
            return new InstalledPackRow(
                id,
                manifest.Name,
                $"{manifest.Name} · {manifest.Version}{(id == _selectedPackId ? "  ←" : "")}",
                $"{id} · {install.Pack.Plugins.Length} plugins · hooks: {hooks}",
                install.Enabled,
                index > 0,
                index < packs.Length - 1);
        })];
    }

    private void Install()
    {
        if (_pending is not { } pack) return;
        if (_workspace.Apply(Edits.InstallPack(pack)))
        {
            ClearReview();
            Message = $"Installed {pack.Manifest.Name}.";
        }
    }

    private void ClearReview()
    {
        _pending = null;
        Review = null;
        InstallCommand.NotifyCanExecuteChanged();
    }

    private void OnProjectChanged(object? sender, ProjectChangedEventArgs e)
    {
        if (_isShown()) Refresh();
    }
}
