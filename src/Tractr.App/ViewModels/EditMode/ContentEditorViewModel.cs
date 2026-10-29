using System.Text.Json;
using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using Tractr.App.Localization;
using Tractr.App.Mvvm;
using Tractr.App.Projects;

namespace Tractr.App.ViewModels.EditMode;

/// <summary>
/// The fields of the entry being edited, as the Content tab's generated form holds them
/// (<c>Game/ContentForm</c>): whether they changed, the record they describe, and that record
/// for readouts while typing.
/// </summary>
public interface IContentDraft
{
    /// <summary>True when the fields no longer describe the entry the form was built from (#87).</summary>
    bool IsDirty { get; }

    /// <summary>The edited record.</summary>
    /// <exception cref="FormatException">A field holds something that is not a valid value.</exception>
    /// <exception cref="JsonException">The fields no longer fit the record.</exception>
    object Commit();

    /// <summary>The record as the fields describe it right now, or null while one is invalid.</summary>
    object? Preview();
}

/// <summary>The summary card above the form (its name names the card for tests and screen readers).</summary>
public sealed record ContentSummaryCard(string Name, Func<GameProject, object, IReadOnlyList<ReadoutLine>> Lines);

/// <summary>The profit line under the form.</summary>
public sealed record ContentProfitLine(string Name, Func<GameProject, object, string> Text);

/// <summary>
/// Built-ins a category lists beside the project's own entries (web CropEditor's default crops,
/// NodeTypeEditor's built-in types). F# says which remain (a project entry with the same id
/// replaces one) and how one becomes a project entry. Deleting that entry (the usual F# remove)
/// brings the built-in back with its uses intact.
/// </summary>
public sealed record ContentBuiltins(
    string Kind,
    Func<GameProject, IEnumerable<object>> Entries,
    Func<string, bool> IsBuiltinId,
    Func<object, Edit> Customize);

/// <summary>A content type of the Content tab: its entries and the F# edits that create, save and remove one.</summary>
public sealed record ContentCategory(
    string Name,
    Type EntityType,
    Func<GameProject, IEnumerable<object>> Entries,
    Func<GameProject, object?> Create,
    Func<object, Edit> Upsert,
    Func<string, Edit> Remove,
    Func<GameProject, object, object>? Duplicate)
{
    /// <summary>The entry's art, shown as its list thumbnail.</summary>
    public Func<object, VisualRef?>? Art { get; init; }

    /// <summary>The muted line under an entry in the list (the flag: a listed built-in).</summary>
    public Func<GameProject, object, bool, string>? Note { get; init; }

    public ContentSummaryCard? Summary { get; init; }

    public ContentProfitLine? Profit { get; init; }

    /// <summary>Built-in entries listed read-only after the project's own.</summary>
    public ContentBuiltins? Builtins { get; init; }
}

/// <summary>
/// An entry of the list: <see cref="Label"/> is "Name · id", <see cref="Note"/> the muted line,
/// <see cref="Art"/> what its thumbnail draws (<see cref="HasArtSlot"/>: the category shows art,
/// so names line up even without it).
/// </summary>
public sealed record ContentListRow(string Id, string Label, string? Note, bool HasArtSlot, VisualRef? Art);

/// <summary>A summary card's lines.</summary>
public sealed record ContentSummary(string Name, IReadOnlyList<ReadoutLine> Lines);

/// <summary>
/// The Content tab: the content types, the filtered entry list, the selected entry and the
/// rules of its draft. One save is one F# upsert, so it is one undo step; adding, duplicating,
/// customizing a built-in and deleting go through the F# document too. The fields themselves
/// are the generated form's (<see cref="IContentDraft"/>): the view builds one whenever
/// <see cref="FormEntity"/> is raised and attaches it. Unsaved fields are never dropped by
/// choosing another type or entry, or by a jump from Problems or search (#87): <see cref="Draft"/>
/// asks first.
/// </summary>
public sealed class ContentEditorViewModel : ObservableObject
{
    private static ContentCategory Of<T>(string name, Func<GameProject, IEnumerable<T>> entries,
        Func<GameProject, T?> create, Func<T, Edit> upsert, Func<string, Edit> remove,
        Func<GameProject, T, T>? duplicate = null) where T : class =>
        new(name, typeof(T), project => entries(project).Cast<object>(), project => create(project),
            entity => upsert((T)entity), remove,
            duplicate is null ? null : (project, entity) => duplicate(project, (T)entity));

    /// <summary>The content types, in the order the type picker lists them.</summary>
    public static IReadOnlyList<ContentCategory> Categories { get; } =
    [
        Of("NPCs", p => p.Npcs, p => Defaults.NewNpc(p, "New NPC"), Edits.UpsertNpc, Edits.RemoveNpc) with
        {
            Art = entity => ((Npc)entity).Visual.OrNull(),
        },
        Of("Dialogue", p => ProjectContent.Dialogues(p), p => p.Npcs.Length > 0 ? Defaults.NewDialogue(p, p.Npcs[0].Id) : null, Edits.UpsertDialogue, Edits.RemoveDialogue),
        Of("Items", p => p.Items, Defaults.NewItem, Edits.UpsertItem, Edits.RemoveItem) with
        {
            Art = entity => ((Item)entity).Visual.OrNull(),
        },
        Of("Crops", p => p.CustomCrops.OrEmpty(), Defaults.NewCrop, Edits.UpsertCrop, Edits.RemoveCrop, Defaults.DuplicateCrop) with
        {
            Art = entity => (entity as CustomCropDefinition)?.Visual.OrNull() ?? (entity as CropDefinition)?.Visual.OrNull(),
            Note = (_, entity, _) => entity switch
            {
                CustomCropDefinition crop => ContentReadouts.CropNote(crop),
                CropDefinition crop => ContentReadouts.BuiltinCropNote(crop),
                _ => "",
            },
            Summary = new ContentSummaryCard("CropSummary", (project, entity) => entity switch
            {
                CustomCropDefinition crop => ContentReadouts.CropSummary(project, crop),
                CropDefinition crop => ContentReadouts.CropSummary(project, crop),
                _ => [],
            }),
            Profit = new ContentProfitLine("CropProfit", (_, entity) => ContentReadouts.CropProfitText((CustomCropDefinition)entity)),
            Builtins = new ContentBuiltins("crop", ContentReadouts.BuiltinCrops, ContentReadouts.IsBuiltinCrop,
                entity => Edits.UpsertCrop(ContentReadouts.CustomizeCrop((CropDefinition)entity))),
        },
        Of("Mines", p => p.Mines.OrEmpty(), Defaults.NewMine, Edits.UpsertMine, Edits.RemoveMine),
        Of("Request boards", p => p.RequestBoards.OrEmpty(), Defaults.NewRequestBoard, Edits.UpsertRequestBoard, Edits.RemoveRequestBoard),
        Of("Quests", p => p.Quests, Defaults.NewQuest, Edits.UpsertQuest, Edits.RemoveQuest),
        Of("Events", p => p.Events, Defaults.NewEvent, Edits.UpsertEvent, Edits.RemoveEvent),
        Of("Cutscenes", p => p.Cutscenes.OrEmpty(), Defaults.NewCutscene, Edits.UpsertCutscene, Edits.RemoveCutscene, Defaults.DuplicateCutscene),
        Of("Heart events", p => p.HeartEvents.OrEmpty(), Defaults.NewHeartEvent, Edits.UpsertHeartEvent, Edits.RemoveHeartEvent, Defaults.DuplicateHeartEvent),
        Of("Mail", p => p.Mail.OrEmpty(), Defaults.NewMail, Edits.UpsertMail, Edits.RemoveMail, Defaults.DuplicateMail),
        Of("Collections", p => p.Collections.OrEmpty(), Defaults.NewCollection, Edits.UpsertCollection, Edits.RemoveCollection, Defaults.DuplicateCollection),
        Of("Bundles", p => p.Bundles.OrEmpty(), Defaults.NewBundle, Edits.UpsertBundle, Edits.RemoveBundle, Defaults.DuplicateBundle),
        Of("Achievements", p => p.Achievements.OrEmpty(), Defaults.NewAchievement, Edits.UpsertAchievement, Edits.RemoveAchievement, Defaults.DuplicateAchievement) with
        {
            Art = entity => ((AchievementDefinition)entity).Visual.OrNull(),
        },
        Of("Shops", p => p.Shops, Defaults.NewShop, Edits.UpsertShop, Edits.RemoveShop),
        Of("Recipes", p => p.Recipes, Defaults.NewRecipe, Edits.UpsertRecipe, Edits.RemoveRecipe) with
        {
            Note = (project, entity, _) => ContentReadouts.RecipeNote(project, (RecipeDefinition)entity),
            Profit = new ContentProfitLine("RecipeProfit", (project, entity) => ContentReadouts.RecipeSummary(project, (RecipeDefinition)entity)),
        },
        Of("Node types", p => p.NodeTypes, Defaults.NewNodeType, Edits.UpsertNodeType, Edits.RemoveNodeType) with
        {
            Art = entity => ((NodeTypeDefinition)entity).Visual.OrNull(),
            Note = (_, entity, builtin) => ContentReadouts.NodeTypeListNote((NodeTypeDefinition)entity, builtin),
            Summary = new ContentSummaryCard("NodeTypeSummary", (project, entity) => ContentReadouts.NodeTypeSummary(project, (NodeTypeDefinition)entity)),
            Builtins = new ContentBuiltins("node type", ContentReadouts.BuiltinNodeTypes, ContentReadouts.IsBuiltinNodeType,
                entity => Edits.UpsertNodeType((NodeTypeDefinition)entity)),
        },
        Of("Machine types", p => p.MachineTypes, Defaults.NewMachineType, Edits.UpsertMachineType, Edits.RemoveMachineType) with
        {
            Art = entity => ((MachineTypeDefinition)entity).Visual.OrNull(),
        },
        Of("Buildings", p => p.BuildingTypes.OrEmpty(), Defaults.NewBuildingType, Edits.UpsertBuildingType, Edits.RemoveBuildingType, Defaults.DuplicateBuildingType) with
        {
            Art = entity => ((BuildingDefinition)entity).Visual.OrNull(),
        },
        Of("Placed buildings", p => p.Buildings.OrEmpty(), Defaults.NewPlacedBuilding, Edits.UpsertPlacedBuilding, Edits.RemovePlacedBuilding) with
        {
            Note = (project, entity, _) => PlacedBuildingNote(project, (BuildingState)entity),
        },
        Of("Animal species", p => p.AnimalSpecies, Defaults.NewAnimalSpecies, Edits.UpsertAnimalSpecies, Edits.RemoveAnimalSpecies) with
        {
            Art = entity => ((AnimalSpeciesDefinition)entity).Visual.OrNull(),
        },
        Of("Monsters", p => p.Monsters.OrEmpty(), Defaults.NewMonster, Edits.UpsertMonster, Edits.RemoveMonster, Defaults.DuplicateMonster) with
        {
            Art = entity => ((MonsterDefinition)entity).Visual.OrNull(),
        },
        Of("Fish tables", p => p.FishTables, Defaults.NewFishTable, Edits.UpsertFishTable, Edits.RemoveFishTable),
        Of("Actions", p => p.Actions, Defaults.NewAction, Edits.UpsertAction, Edits.RemoveAction, Defaults.DuplicateAction),
        Of("Minigames", p => p.Minigames, Defaults.NewMinigame, Edits.UpsertMinigame, Edits.RemoveMinigame, Defaults.DuplicateMinigame),
        Of("Terrains", p => p.Terrains.OrEmpty(), Defaults.NewTerrain, Edits.UpsertTerrain, Edits.RemoveTerrain) with
        {
            Art = entity => ((TerrainDefinition)entity).Visual.OrNull(),
            Note = (_, entity, builtin) => ContentReadouts.TerrainListNote((TerrainDefinition)entity, builtin),
            Builtins = VocabularyBuiltins<TerrainDefinition>("terrain", "terrain", ContentReadouts.BuiltinTerrains, Edits.UpsertTerrain),
        },
        Of("Tools", p => p.Tools.OrEmpty(), Defaults.NewTool, Edits.UpsertTool, Edits.RemoveTool) with
        {
            Note = (_, entity, builtin) => ContentReadouts.ToolListNote((ToolDefinition)entity, builtin),
            Builtins = VocabularyBuiltins<ToolDefinition>("tool", "tool", ContentReadouts.BuiltinTools, Edits.UpsertTool),
        },
        Of("Item kinds", p => p.ItemCategories.OrEmpty(), Defaults.NewItemCategory, Edits.UpsertItemCategory, Edits.RemoveItemCategory) with
        {
            Note = (_, entity, builtin) => ContentReadouts.VocabularyListNote("itemCategory", ((ItemCategoryDefinition)entity).Id, builtin),
            Builtins = VocabularyBuiltins<ItemCategoryDefinition>("item kind", "itemCategory", ContentReadouts.BuiltinItemCategories, Edits.UpsertItemCategory),
        },
        Of("Crop qualities", p => p.CropQualities.OrEmpty(), Defaults.NewCropQuality, Edits.UpsertCropQuality, Edits.RemoveCropQuality) with
        {
            Note = (_, entity, builtin) => ContentReadouts.VocabularyListNote("cropQuality", ((CropQualityDefinition)entity).Id, builtin),
            Builtins = VocabularyBuiltins<CropQualityDefinition>("crop quality", "cropQuality", ContentReadouts.BuiltinCropQualities, Edits.UpsertCropQuality),
        },
        Of("Crop mutations", p => p.CropMutations.OrEmpty(), Defaults.NewCropMutation, Edits.UpsertCropMutation, Edits.RemoveCropMutation) with
        {
            Note = (_, entity, builtin) => ContentReadouts.VocabularyListNote("cropMutation", ((CropMutationDefinition)entity).Id, builtin),
            Builtins = VocabularyBuiltins<CropMutationDefinition>("crop mutation", "cropMutation", ContentReadouts.BuiltinCropMutations, Edits.UpsertCropMutation),
        },
    ];

    /// <summary>"Shed at Farm (4, 6)": what a building on the start map is and where it stands (M5).</summary>
    private static string PlacedBuildingNote(GameProject project, BuildingState building)
    {
        var kind = project.BuildingTypes.OrEmpty().FirstOrDefault(b => b.Id == building.BuildingId)?.Name ?? building.BuildingId;
        var scene = project.Scenes.FirstOrDefault(s => s.Id == building.SceneId)?.Name ?? building.SceneId;
        return EditorStrings.Format("content.placedBuildingNote", kind, scene, building.X, building.Y);
    }

    /// The built-ins of a world vocabulary list (M1): <paramref name="kind"/> names one in
    /// messages, <paramref name="builtinKind"/> is the F# kind; customizing copies it as it is.
    private static ContentBuiltins VocabularyBuiltins<T>(string kind, string builtinKind, Func<GameProject, IEnumerable<T>> entries, Func<T, Edit> upsert)
        where T : class =>
        new(kind, project => entries(project).Cast<object>(), id => ContentReadouts.IsBuiltinVocabulary(builtinKind, id), entity => upsert((T)entity));

    private const string NothingSelected = "Select an entry to edit it.";

    private readonly ProjectWorkspace _workspace;
    private readonly Func<bool> _isShown;
    private ContentCategory _category = Categories[0];
    private string? _selectedId;
    private string _filter = "";
    private IReadOnlyList<ContentListRow> _rows = [];
    private string _message = NothingSelected;
    private bool _canAdd;
    private bool _canDelete;
    private bool _canEdit;
    private bool _showDuplicate;
    private bool _showAddToInventory;
    private bool _showCustomize;
    private string? _deleteTip;
    private string? _customizeTip;
    private object? _editing;
    private object? _builtin;
    private ContentSummary? _summary;
    private string? _profitName;
    private string _profitText = "";

    /// <summary>The category and id the form on show was built for.</summary>
    private (ContentCategory Category, string Id)? _formFor;
    private object? _rowAssets;
    private GameProject? _formProject;
    private static readonly System.Reflection.PropertyInfo[] FormContextFields = typeof(GameProject)
        .GetProperties(System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.Public)
        .Where(field => field.GetIndexParameters().Length == 0 && field.Name is not
            (nameof(GameProject.Scenes) or nameof(GameProject.SelectedTileType) or nameof(GameProject.SelectedTileVisual)))
        .ToArray();

    private static bool SameFormContext(GameProject before, GameProject after) =>
        FormContextFields.All(field => Equals(field.GetValue(before), field.GetValue(after)))
        && before.Scenes.Select(scene => (scene.Id, scene.Name, scene.Width, scene.Height))
            .SequenceEqual(after.Scenes.Select(scene => (scene.Id, scene.Name, scene.Width, scene.Height)));

    private IContentDraft? _draft;

    /// <param name="workspace">The open project.</param>
    /// <param name="isShown">Whether the tab is on show: project changes refresh it only then (opening it refreshes too).</param>
    public ContentEditorViewModel(ProjectWorkspace workspace, Func<bool>? isShown = null)
    {
        _workspace = workspace ?? throw new ArgumentNullException(nameof(workspace));
        _isShown = isShown ?? (() => true);
        Draft = new DraftPrompt(SaveDraft, LoadForm);
        AddCommand = new RelayCommand(() => Navigate(AddNow), () => _canAdd);
        DeleteCommand = new RelayCommand(Delete, () => _canDelete);
        DuplicateCommand = new RelayCommand(() => Navigate(DuplicateNow), () => _canEdit);
        AddToInventoryCommand = new RelayCommand(AddToInventory, () => _canEdit);
        SaveCommand = new RelayCommand(Save, () => _canEdit);
        RevertCommand = new RelayCommand(LoadForm, () => _canEdit);
        CustomizeCommand = new RelayCommand(() => Navigate(CustomizeNow));
        _workspace.ProjectChanged += OnProjectChanged;
        Refresh();
    }

    /// <summary>The content type on show.</summary>
    public ContentCategory SelectedCategory => _category;

    /// <summary>The selected entry's id (null when none).</summary>
    public string? SelectedId
    {
        get => _selectedId;
        private set => SetProperty(ref _selectedId, value);
    }

    /// <summary>Shows only entries whose "Name · id" contains it (the selected entry always shows).</summary>
    public string Filter
    {
        get => _filter;
        set
        {
            if (SetProperty(ref _filter, value ?? "")) RefreshList();
        }
    }

    /// <summary>The selected type's entries, the project's first, then its built-ins.</summary>
    public IReadOnlyList<ContentListRow> Rows
    {
        get => _rows;
        private set => SetProperty(ref _rows, value);
    }

    /// <summary>The status line.</summary>
    public string Message
    {
        get => _message;
        private set => SetProperty(ref _message, value);
    }

    /// <summary>"Duplicate" is offered for this type.</summary>
    public bool ShowDuplicate
    {
        get => _showDuplicate;
        private set => SetProperty(ref _showDuplicate, value);
    }

    /// <summary>"Add to inventory" is offered (items).</summary>
    public bool ShowAddToInventory
    {
        get => _showAddToInventory;
        private set => SetProperty(ref _showAddToInventory, value);
    }

    /// <summary>"Customize" is offered (a built-in is selected).</summary>
    public bool ShowCustomize
    {
        get => _showCustomize;
        private set => SetProperty(ref _showCustomize, value);
    }

    /// <summary>What Delete does when the entry replaces a built-in; null otherwise.</summary>
    public string? DeleteTip
    {
        get => _deleteTip;
        private set => SetProperty(ref _deleteTip, value);
    }

    public string? CustomizeTip
    {
        get => _customizeTip;
        private set => SetProperty(ref _customizeTip, value);
    }

    /// <summary>
    /// The entry the form edits (null: no form). Raised every time the form is to be built again,
    /// even for the same entry: the view then builds a form over <see cref="SelectedCategory"/>'s
    /// type and hands it to <see cref="AttachDraft"/>.
    /// </summary>
    public object? FormEntity => _editing;

    /// <summary>The summary card (null: none).</summary>
    public ContentSummary? Summary
    {
        get => _summary;
        private set => SetProperty(ref _summary, value);
    }

    /// <summary>The name of the profit line under the form (null: none).</summary>
    public string? ProfitName
    {
        get => _profitName;
        private set => SetProperty(ref _profitName, value);
    }

    public string ProfitText
    {
        get => _profitText;
        private set => SetProperty(ref _profitText, value);
    }

    /// <summary>True when the readouts follow the fields while they are edited.</summary>
    public bool FollowsFields => _draft is not null && (_category.Summary is not null || _category.Profit is not null);

    /// <summary>The question asked before unsaved fields would be replaced.</summary>
    public DraftPrompt Draft { get; }

    public RelayCommand AddCommand { get; }

    public RelayCommand DeleteCommand { get; }

    public RelayCommand DuplicateCommand { get; }

    public RelayCommand AddToInventoryCommand { get; }

    public RelayCommand SaveCommand { get; }

    /// <summary>Builds the form again from the saved entry.</summary>
    public RelayCommand RevertCommand { get; }

    /// <summary>Copies the selected built-in into the project.</summary>
    public RelayCommand CustomizeCommand { get; }

    /// <summary>True while the form holds edits that are not saved (#87).</summary>
    public bool HasUnsavedChanges => _draft?.IsDirty == true;

    /// <summary>Stops following the project (the editor that built this tab was replaced).</summary>
    public void Retire() => _workspace.ProjectChanged -= OnProjectChanged;

    /// <summary>The form the view built for <see cref="FormEntity"/>.</summary>
    public void AttachDraft(IContentDraft draft) => _draft = draft;

    /// <summary>A message from the form (a structural change it refused).</summary>
    public void Report(string message) => Message = message;

    /// <summary>Shows another content type, asking first about unsaved fields.</summary>
    public void SelectCategory(string name)
    {
        var category = Categories.First(c => c.Name == name);
        if (ReferenceEquals(category, _category)) return;
        Navigate(() =>
        {
            _category = category;
            SelectedId = null;
            OnPropertyChanged(nameof(SelectedCategory));
            Refresh();
        });
    }

    /// <summary>Opens <paramref name="id"/> of <paramref name="category"/> (Problems, search), asking first about unsaved fields.</summary>
    public void SelectEntry(string category, string id)
    {
        var target = Categories.First(c => c.Name == category);
        Navigate(() =>
        {
            _category = target;
            SelectedId = id;
            Filter = "";
            OnPropertyChanged(nameof(SelectedCategory));
            Refresh();
        });
    }

    /// <summary>The entry chosen in the list (null: none), asking first about unsaved fields.</summary>
    public void SelectRow(string? id)
    {
        if (id == _selectedId) return;
        Navigate(() =>
        {
            SelectedId = id;
            LoadForm();
        });
    }

    /// <summary>
    /// Saves the form's unsaved fields, if any (switching projects, closing the editor). False
    /// when they could not be saved; the message says why.
    /// </summary>
    public bool SaveDraft()
    {
        if (!HasUnsavedChanges) return true;
        Save();
        return !HasUnsavedChanges;
    }

    /// <summary>
    /// Every entry of every type whose name or id contains <paramref name="query"/> (ignoring
    /// case), names that start with it first: the quick-open search (#56).
    /// </summary>
    public static IReadOnlyList<(string Category, string Id, string Label)> Search(GameProject project, string query, int limit = 50)
    {
        ArgumentNullException.ThrowIfNull(project);
        var text = (query ?? "").Trim();
        if (text.Length == 0) return [];
        var found = new List<(string Category, string Id, string Label, int Rank)>();
        foreach (var category in Categories)
        {
            foreach (var entity in category.Entries(project).Concat(category.Builtins?.Entries(project) ?? []))
            {
                var id = IdOf(entity);
                var name = NameOf(entity) ?? "";
                var rank = Rank(name, text) is { } byName ? byName : Rank(id, text) is { } byId ? byId + 2 : (int?)null;
                if (rank is { } r && !found.Any(f => f.Category == category.Name && f.Id == id)) found.Add((category.Name, id, LabelOf(entity), r));
            }
        }

        return found.OrderBy(f => f.Rank).ThenBy(f => f.Label, StringComparer.OrdinalIgnoreCase).Take(limit).Select(f => (f.Category, f.Id, f.Label)).ToList();
    }

    /// <summary>0 when <paramref name="value"/> starts with <paramref name="query"/>, 1 when it contains it, else null.</summary>
    public static int? Rank(string value, string query)
    {
        ArgumentNullException.ThrowIfNull(value);
        return value.StartsWith(query, StringComparison.OrdinalIgnoreCase) ? 0
            : value.Contains(query, StringComparison.OrdinalIgnoreCase) ? 1
            : null;
    }

    private static string IdOf(object entity) => (string)(entity.GetType().GetProperty("Id")?.GetValue(entity) ?? "");

    /// <summary>An entry's name: its <c>Name</c>, or a letter's <c>Subject</c> (M6).</summary>
    private static string? NameOf(object entity) =>
        (entity.GetType().GetProperty("Name") ?? entity.GetType().GetProperty("Subject"))?.GetValue(entity) as string;

    /// <summary>"Name  ·  id", or the id of an entry without a name.</summary>
    public static string LabelOf(object entity)
    {
        ArgumentNullException.ThrowIfNull(entity);
        var name = NameOf(entity);
        return string.IsNullOrWhiteSpace(name) ? IdOf(entity) : $"{name}  ·  {IdOf(entity)}";
    }

    /// <summary>Runs <paramref name="next"/> now, or once the creator saved or discarded the form's unsaved fields.</summary>
    private void Navigate(Action next)
    {
        if (!HasUnsavedChanges)
        {
            Draft.Hide();
            next();
            return;
        }

        Draft.Ask(_editing is null ? "this entry" : LabelOf(_editing), next);
    }

    /// <summary>
    /// Catches up with the project. A form with unsaved fields stays as typed when the tab is shown
    /// again or the project changes elsewhere (undo, a map edit), as long as its entry is still
    /// there (#87).
    /// </summary>
    public void Refresh()
    {
        var project = _workspace.Current;
        RefreshList();
        if (_formFor is { } shown && ReferenceEquals(shown.Category, _category) && shown.Id == _selectedId && project is not null
            && _category.Entries(project).FirstOrDefault(entity => IdOf(entity) == _selectedId) is { } stored && HasUnsavedChanges)
        {
            if (!Equals(stored, _editing))
            {
                Message = $"{LabelOf(stored)} changed elsewhere while you edited it. Save changes keeps what you typed; Revert fields shows the saved entry.";
            }

            return;
        }

        if (_formFor is { } currentForm && ReferenceEquals(currentForm.Category, _category) && currentForm.Id == _selectedId
            && _formProject is { } prior && project is not null && SameFormContext(prior, project))
        {
            return;
        }

        LoadForm();
    }

    /// <summary>The entry list of the selected type, filtered (the selected entry always shows).</summary>
    private void RefreshList()
    {
        var project = _workspace.Current;
        var filter = _filter.Trim();
        bool Shown(object entity) => filter.Length == 0 || IdOf(entity) == _selectedId || Rank(LabelOf(entity), filter) is not null;
        var rows = new List<ContentListRow>();
        if (project is not null)
        {
            rows.AddRange(_category.Entries(project).Where(Shown).Select(entity => Row(project, entity, builtin: false)));
            rows.AddRange((_category.Builtins?.Entries(project) ?? []).Where(Shown).Select(entity => Row(project, entity, builtin: true)));
        }

        if (!rows.Any(row => row.Id == _selectedId)) SelectedId = null;
        var assetsChanged = !ReferenceEquals(_rowAssets, project?.CustomAssets);
        _rowAssets = project?.CustomAssets;
        if (assetsChanged || !Rows.SequenceEqual(rows)) Rows = rows;
        SetCanAdd(project is not null);
        SetCanDelete(_selectedId is not null);
    }

    private ContentListRow Row(GameProject project, object entity, bool builtin) =>
        new(IdOf(entity), LabelOf(entity), _category.Note?.Invoke(project, entity, builtin), _category.Art is not null, _category.Art?.Invoke(entity));

    /// <summary>Fills the form from the selected entry (dropping what was typed), or empties it.</summary>
    private void LoadForm()
    {
        Draft.Hide();
        _formFor = null;
        _formProject = _workspace.Current;
        _draft = null;
        Summary = null;
        ProfitName = null;
        ProfitText = "";
        _editing = null;
        _builtin = null;
        var project = _workspace.Current;
        if (_selectedId is { } id && project is not null)
        {
            _editing = _category.Entries(project).FirstOrDefault(entity => IdOf(entity) == id);
            if (_editing is null) _builtin = _category.Builtins?.Entries(project).FirstOrDefault(entity => IdOf(entity) == id);
        }

        SetCanEdit(_editing is not null);
        SetCanDelete(_editing is not null);
        ShowDuplicate = _category.Duplicate is not null;
        ShowAddToInventory = _category.EntityType == typeof(Item);
        ShowCustomize = _builtin is not null;
        var builtins = _category.Builtins;
        DeleteTip = _editing is not null && builtins is not null && builtins.IsBuiltinId(_selectedId!)
            ? $"Remove your changes and use the built-in {builtins.Kind} again"
            : null;
        if (project is null || _editing is null)
        {
            Message = _builtin is not null && builtins is not null && project is not null
                ? $"This is a built-in {builtins.Kind}. Customize it to change it."
                : NothingSelected;
            if (_builtin is not null && builtins is not null) CustomizeTip = $"Copy this built-in {builtins.Kind} into the project so you can edit it";
            OnPropertyChanged(nameof(FormEntity));
            if (_builtin is not null && project is not null) ShowReadouts(project, _builtin);
            return;
        }

        Message = $"Editing {_category.Name.ToLowerInvariant()} · {_selectedId}";
        _formFor = (_category, _selectedId!);
        OnPropertyChanged(nameof(FormEntity));
        ProfitName = _category.Profit?.Name;
        ShowReadouts(project, _editing);
    }

    /// <summary>The summary card and profit line for <paramref name="entity"/> (saved, built-in or as edited).</summary>
    private void ShowReadouts(GameProject project, object entity)
    {
        if (_category.Summary is { } summary) Summary = new ContentSummary(summary.Name, summary.Lines(project, entity));
        if (_profitName is not null && _category.Profit is { } profit) ProfitText = profit.Text(project, entity);
    }

    /// <summary>Recomputes the readouts from the fields; while one holds something invalid the last readout stays.</summary>
    public void PreviewReadouts()
    {
        if (_draft?.Preview() is { } edited && _workspace.Current is { } project) ShowReadouts(project, edited);
    }

    private void SetCanAdd(bool value)
    {
        if (_canAdd == value) return;
        _canAdd = value;
        AddCommand.NotifyCanExecuteChanged();
    }

    private void SetCanDelete(bool value)
    {
        if (_canDelete == value) return;
        _canDelete = value;
        DeleteCommand.NotifyCanExecuteChanged();
    }

    /// <summary>Whether there is a project entry to save, revert, duplicate or add to the inventory.</summary>
    private void SetCanEdit(bool value)
    {
        if (_canEdit == value) return;
        _canEdit = value;
        DuplicateCommand.NotifyCanExecuteChanged();
        AddToInventoryCommand.NotifyCanExecuteChanged();
        SaveCommand.NotifyCanExecuteChanged();
        RevertCommand.NotifyCanExecuteChanged();
    }

    private void AddNow()
    {
        if (_workspace.Current is not { } project) return;
        var created = _category.Create(project);
        if (created is null)
        {
            Message = "Create an NPC first so this dialogue has an owner.";
            return;
        }

        SelectedId = IdOf(created);
        if (!_workspace.Apply(_category.Upsert(created))) Message = "No changes were made.";
    }

    /// <summary>The selected built-in becomes a project entry with the same id (it replaces the built-in).</summary>
    private void CustomizeNow()
    {
        if (_builtin is not { } builtin || _category.Builtins is not { } builtins) return;
        SelectedId = IdOf(builtin);
        if (_workspace.Apply(builtins.Customize(builtin)))
            Message = $"Customized {LabelOf(builtin)}. Delete it to use the built-in {builtins.Kind} again.";
    }

    private void DuplicateNow()
    {
        if (_editing is null || _category.Duplicate is not { } duplicate || _workspace.Current is not { } project) return;
        var copy = duplicate(project, _editing);
        SelectedId = IdOf(copy);
        if (_workspace.Apply(_category.Upsert(copy))) Message = $"Duplicated as {LabelOf(copy)}.";
    }

    private void AddToInventory()
    {
        if (_editing is not Item item || _workspace.Current is not { } project) return;
        var message = ContentActions.AddToInventoryMessage(project, item.Id);
        _workspace.Apply(Edits.AddToInventory(item.Id));
        Message = message;
    }

    /// <summary>Removes the entry; one that replaces a built-in goes back to the built-in (still selected).</summary>
    private void Delete()
    {
        if (_selectedId is not { } id) return;
        if (_category.Builtins is { } builtins && builtins.IsBuiltinId(id))
        {
            if (_workspace.Apply(_category.Remove(id))) Message = $"Removed your changes: {id} is the built-in {builtins.Kind} again.";
            return;
        }

        SelectedId = null;
        _workspace.Apply(_category.Remove(id));
    }

    private void Save()
    {
        if (_editing is null || _draft is null) return;
        try
        {
            var updated = _draft.Commit();
            if (IdOf(updated) != _selectedId) throw new JsonException("The id cannot be changed here.");
            var saved = _workspace.Apply(_category.Upsert(updated));
            // The fields now match the project: show the saved entry.
            LoadForm();
            if (!saved) Message = "No changes were made.";
        }
        catch (Exception error) when (error is JsonException or FormatException or OverflowException)
        {
            Message = $"Could not save: {error.Message}";
        }
    }

    private void OnProjectChanged(object? sender, ProjectChangedEventArgs e)
    {
        // Another project: its entries are not the form's (the fields were saved, if they could
        // be, before the switch; see EditModeView.SaveDrafts).
        if (e.Kind == ProjectChangeKind.Opened)
        {
            Draft.Hide();
            _formFor = null;
            SelectedId = null;
        }

        if (_isShown()) Refresh();
    }
}
