using System.Globalization;
using System.Reflection;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Interop;
using FarmEngine.Schemas;
using Microsoft.FSharp.Collections;
using Microsoft.FSharp.Core;
using Microsoft.FSharp.Reflection;
using Tractr.App.ViewModels.EditMode;
using static Tractr.App.Game.ContentFormLayout;
using static Tractr.App.Game.ContentFormSchema;

namespace Tractr.App.Game;

/// <summary>
/// A generated form over a JSON draft of one content entry. Property types pick the controls:
/// nested records become grouped sub-forms, lists get add/remove/move rows, string lists become
/// chips or rows. F# <see cref="ContentForms"/> says what the strings mean (references, choices),
/// which condition and outcome fields each type shows, and what new rows start as. Every nested
/// field also has a collapsed "Edit as JSON" box. Controls write into the draft; the owner turns
/// it back into the record with <see cref="Commit"/> and applies one F# upsert.
/// Where the web has its own layout the form follows it: tabs F# lists for the record (crops),
/// compact schedule and waypoint rows (NPCs), stock cards (shops), and art previews beside visual
/// bindings.
/// </summary>
internal sealed partial class ContentForm : IContentDraft
{
    private readonly GameProject _project;
    private readonly object _entity;
    private readonly Type _entityType;
    private readonly StackPanel _root;
    private readonly Action<string> _report;
    private readonly ContentDraftViewModel _draft;
    private readonly VocabularyEditor _vocabulary;
    private readonly ListEditor _lists;
    private List<Action> _writers => _draft.Writers;
    private List<string> _errors => _draft.Errors;
    private readonly Dictionary<string, IReadOnlyList<PickerOption>> _options = [];
    private Dictionary<string, Item>? _items;
    private VisualPreview? _art;
    private readonly EditorPreview? _sharedPreview;
    private int _tab;

    /// <summary>"Pick on map" for tile coordinates (scene id, prompt, what to do with the tile); null hides the buttons (#47).</summary>
    private readonly Action<string, string, Action<int, int>>? _pickOnMap;

    public ContentForm(GameProject project, object entity, Type entityType, StackPanel root, Action<string> report, Action<string, string, Action<int, int>>? pickOnMap = null, EditorPreview? preview = null)
    {
        _pickOnMap = pickOnMap;
        _sharedPreview = preview;
        _project = project;
        _entity = entity;
        _entityType = entityType;
        _root = root;
        _report = report;
        _draft = new ContentDraftViewModel(entity, entityType);
        _vocabulary = new VocabularyEditor(this);
        _lists = new ListEditor(this);
        Rebuild();
        _draft.AcceptInitial();
    }

    public bool IsDirty => _draft.IsDirty;

    public JsonObject Draft => _draft.Draft;

    public object Commit() => _draft.Commit();

    public object? Preview() => _draft.Preview();

    private List<string> Flush() => _draft.Flush();

    /// <summary>A structural change (add, remove, move, retype): write the fields, change, rebuild.</summary>
    private void Structural(Action change)
    {
        var errors = Flush();
        if (errors.Count > 0)
        {
            _report("Fix these first: " + string.Join(" ", errors));
            return;
        }

        change();
        Rebuild();
    }

    private void Rebuild()
    {
        _root.Children.Clear();
        _draft.ClearBindings();
        _options.Clear();
        AddForm();
    }

    private object CurrentEntity()
    {
        try
        {
            return RecordJson.FromNode(Draft, _entityType);
        }
        catch (FormatException)
        {
            return _entity;
        }
    }

    private IReadOnlyList<PickerOption> Options(string kind)
    {
        if (!_options.TryGetValue(kind, out var options))
        {
            options = ContentForms.Options(kind, _project);
            _options[kind] = options;
        }

        return options;
    }

    // ---- Record forms (reflection over the schema records: ContentFormSchema) ----

    /// <summary>
    /// The entry itself: one form, or a tab per F# <see cref="FormTab"/> (web CropEditor's Basic /
    /// Growth / Asset). Every tab's controls exist, so the writers and <see cref="Commit"/> do not
    /// depend on the tab showing. The art previews of one build share a renderer that is freed
    /// when the build is done; the images keep their own bitmaps.
    /// </summary>
    private void AddForm()
    {
        _art = new VisualPreview(_sharedPreview);
        try
        {
            var tabs = ContentReadouts.FormTabs(_entityType.Name);
            if (tabs.Count == 0) AddProperties(_root, Draft, _entityType, "");
            else _root.Children.Add(Tabs(tabs));
        }
        finally
        {
            _art.Dispose();
            _art = null;
        }
    }

    /// <summary>The tabs in F# order; properties no tab lists go on the last one. A rebuild keeps the selected tab.</summary>
    private TabControl Tabs(IReadOnlyList<FormTab> tabs)
    {
        var properties = VisibleProperties(Draft, _entityType).ToList();
        var listed = tabs.SelectMany(ContentReadouts.TabProperties).ToHashSet();
        var control = new TabControl { Name = "ContentTabs" };
        for (var i = 0; i < tabs.Count; i++)
        {
            var panel = new StackPanel { Spacing = 10, Margin = new Thickness(0, 12, 0, 0) };
            var shown = ContentReadouts.TabProperties(tabs[i])
                .Select(name => properties.FirstOrDefault(property => property.Name == name))
                .OfType<PropertyInfo>();
            if (i == tabs.Count - 1) shown = shown.Concat(properties.Where(property => !listed.Contains(property.Name)));
            foreach (var property in shown) AddProperty(panel, Draft, _entityType, property, property.Name);
            var title = tabs[i].Title;
            control.Items.Add(Accessible(new TabItem { Name = "ContentTab_" + string.Concat(title.Where(char.IsLetterOrDigit)), Header = title, Content = panel }, title));
        }

        control.SelectedIndex = Math.Clamp(_tab, 0, tabs.Count - 1);
        control.SelectionChanged += (_, e) =>
        {
            // Pickers inside the tabs raise the same (bubbling) event.
            if (ReferenceEquals(e.Source, control) && control.SelectedIndex >= 0) _tab = control.SelectedIndex;
        };
        return control;
    }

    private void AddProperties(Panel panel, JsonObject target, Type type, string path)
    {
        foreach (var property in VisibleProperties(target, type))
        {
            AddProperty(panel, target, type, property, Join(path, property.Name));
        }

        // A building on the start map (M5) takes its top-left tile from the map.
        if (type == typeof(BuildingState) && path.Length == 0
            && panel.Children.OfType<TextBox>().FirstOrDefault(box => box.Name == "ContentField_X") is { } x
            && panel.Children.OfType<TextBox>().FirstOrDefault(box => box.Name == "ContentField_Y") is { } y
            && _vocabulary.PickButton("ContentPick_X", "the building's top-left tile", () => StringOf(target["sceneId"]) ?? "", x, y) is { } pick)
        {
            panel.Children.Insert(panel.Children.IndexOf(y) + 1, pick);
        }
    }

    private void AddProperty(Panel panel, JsonObject target, Type owner, PropertyInfo property, string path)
    {
        var key = JsonKey(owner, property);
        var label = Title(property.Name);
        if (owner == typeof(QuestObjective) && property.Name == nameof(QuestObjective.TargetItemQuantity))
        {
            label = StringOf(target["type"]) switch
            {
                "hearts" => "Hearts",
                "mineFloor" => "Floor",
                _ => label,
            };
        }
        var name = "ContentField_" + path;
        var type = Unwrap(property.PropertyType);
        var nullable = IsNullable(property);
        // A record with a type picks some fields' meaning by it (a cutscene step's sound or music).
        var field = Discriminator(target, owner) is { } kind
            ? ContentForms.Field(owner.Name, property.Name, label, _project, kind)
            : ContentForms.Field(owner.Name, property.Name, label, _project);
        if (field is { Kind: "reference" or "referenceList" }) label = TrimId(label);

        // A record's own id is shown, not edited; an optional "Id" names something else (a neighbour's crop).
        if (property.Name == "Id" && property.PropertyType == typeof(string))
        {
            panel.Children.Add(Label(label));
            panel.Children.Add(Accessible(new TextBox { Name = name, Text = StringOf(target[key]) ?? "", IsReadOnly = true }, label));
            return;
        }

        if (type == typeof(string))
        {
            panel.Children.Add(Label(label));
            var control = StringControl(target, key, name, label, field, nullable,
                multiline: property.Name is "Description" or "Text" or "Message" or "FailMessage",
                rebuildOnChange: property.Name == "Type" || (owner == typeof(MinigameDef) && property.Name == "Kind")
                    || (owner == typeof(GameEvent) && property.Name == "Trigger"));
            panel.Children.Add(control);
            return;
        }

        if (type == typeof(bool))
        {
            panel.Children.Add(BoolControl(target, key, name, label, nullable ? null : false));
            return;
        }

        if (IsNumber(type) || IsOptionalNullableNumber(type))
        {
            panel.Children.Add(Label(label));
            panel.Children.Add(NumberControl(target, key, name, label, type == typeof(int) || type == typeof(long),
                optional: nullable || IsOptionalNullableNumber(type), min: null, max: null));
            return;
        }

        var body = new StackPanel { Spacing = 8 };
        if (IsJsonDictionary(type) && owner == typeof(MinigameDef)) _lists.AddMinigameSettings(body, target, key, path);
        else if (IsJsonDictionary(type)) _lists.AddKeyValues(body, target, key, path);
        else if (ListElement(type) is { } element)
        {
            if (element == typeof(string))
            {
                if (field is { Kind: "referenceList" }) _lists.AddReferenceChips(body, target, key, path, label, field.Reference, nullable);
                else _lists.AddTextRows(body, target, key, path, label, nullable);
            }
            else if (IsNumber(element)) _lists.AddNumberRows(body, target, key, path, label, nullable);
            else if (element == typeof(EventCondition)) _vocabulary.AddConditions(body, target, key, path);
            else if (element == typeof(EventOutcome)) _vocabulary.AddOutcomes(body, target, key, path);
            else if (element == typeof(NpcScheduleEntry)) _lists.AddScheduleRows(body, target, key, path, nullable);
            else if (element == typeof(GridPoint)) _lists.AddWaypointRows(body, target, key, path, nullable);
            else if (element == typeof(ShopStockEntry)) _lists.AddStockCards(body, target, key, path, nullable);
            else if (IsRecord(element)) _lists.AddRecordList(body, target, key, path, element, nullable);
        }
        else if (IsRecord(type)) _lists.AddNested(body, target, key, path, type, nullable, label);

        AddJsonEscape(body, target, key, path, label, nullable);
        var group = new StackPanel { Spacing = 6 };
        group.Children.Add(Ui.Text(label.ToUpperInvariant(), "category"));
        group.Children.Add(body);
        panel.Children.Add(new Border { Name = "ContentGroup_" + path, Child = group }.WithClasses("form-group"));
    }

    // ---- Scalars ----

    private Control StringControl(JsonObject target, string key, string name, string label, FormField? field, bool nullable, bool multiline, bool rebuildOnChange,
        Func<PickerOption, string>? optionLabel = null)
    {
        var current = StringOf(target[key]);
        if (field is { Kind: "reference" or "choice" })
        {
            var picker = Accessible(Picker(name, field, current, optionLabel), label);
            var initial = SelectedId(picker);
            Writer(target, key, () => SelectedId(picker) is not { } id || id == initial, () =>
            {
                var id = SelectedId(picker)!;
                target[key] = id.Length == 0 && nullable ? null : JsonValue.Create(id);
            });
            if (rebuildOnChange) picker.SelectionChanged += (_, _) => { if (SelectedId(picker) is { } id && id != initial) Structural(() => { }); };
            return picker;
        }

        var box = Accessible(new TextBox
        {
            Name = name,
            Text = current ?? "",
            AcceptsReturn = multiline,
            TextWrapping = multiline ? TextWrapping.Wrap : TextWrapping.NoWrap,
            MinHeight = multiline ? 54 : 30,
        }, label);
        if (field is { Kind: "plain" } && field.Reason.Length > 0) ToolTip.SetTip(box, Ui.Capitalize(field.Reason) + ".");
        var original = box.Text;
        Writer(target, key, () => (box.Text ?? "") == original, () =>
        {
            var text = box.Text ?? "";
            target[key] = nullable && text.Length == 0 ? null : JsonValue.Create(text);
        });
        return box;
    }

    private CheckBox BoolControl(JsonObject target, string key, string name, string label, bool? whenAbsent)
    {
        var value = target[key] is JsonValue node && node.TryGetValue<bool>(out var flag) ? flag : whenAbsent;
        var check = Accessible(new CheckBox { Name = name, Content = label, IsChecked = value, IsThreeState = whenAbsent is null }, label);
        var original = check.IsChecked;
        Writer(target, key, () => check.IsChecked == original, () => target[key] = check.IsChecked is { } on ? JsonValue.Create(on) : null);
        return check;
    }

    /// <summary>
    /// A number box. Saving never changes what was typed behind the creator's back (#46): an
    /// emptied required field and a value outside <paramref name="min"/>–<paramref name="max"/>
    /// are reported by name instead of being reset or clamped.
    /// </summary>
    private TextBox NumberControl(JsonObject target, string key, string name, string label, bool integer, bool optional, double? min, double? max)
    {
        var box = Accessible(new TextBox { Name = name, Text = NumberText(target[key]), MinWidth = 90 }, label);
        var original = box.Text;
        Writer(target, key, () => (box.Text ?? "").Trim() == original,
            () => _draft.WriteNumber(target, key, label, box.Text, integer, optional, min, max));
        return box;
    }

    /// <summary>
    /// Registers the writer of the field <paramref name="key"/>: <paramref name="write"/> runs when
    /// the control changed. Unchanged, the value the field had when the form was built goes back
    /// into the draft, since an earlier <see cref="Preview"/> may have written a value the creator
    /// then typed back: the draft always follows the controls.
    /// </summary>
    private void Writer(JsonObject target, string key, Func<bool> unchanged, Action write) => _draft.RegisterWriter(target, key, unchanged, write);

    /// <summary>
    /// A picker of <paramref name="field"/>'s choices or references, keeping an unknown current id
    /// as "(missing: id)"; <paramref name="optionLabel"/> relabels the known entries.
    /// </summary>
    private ComboBox Picker(string name, FormField field, string? current, Func<PickerOption, string>? optionLabel = null)
    {
        var available = field.Kind == "choice" ? field.Choices : Options(field.Reference);
        var picker = new ComboBox
        {
            Name = name,
            MinWidth = 200,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            PlaceholderText = "Pick…",
            IsEditable = field.Kind == "reference",
            MaxDropDownHeight = 320,
        };
        foreach (var option in ContentForms.Entries(field, available, current))
        {
            var label = optionLabel is not null && !option.Missing ? optionLabel(option) : option.Label;
            var item = new ComboBoxItem { Content = label, Tag = option.Id };
            TextSearch.SetText(item, label);
            if (option.Missing) item.Classes.Add("missing");
            picker.Items.Add(item);
        }

        var selected = current ?? "";
        picker.SelectedItem = picker.Items.OfType<ComboBoxItem>().FirstOrDefault(item => (string)item.Tag! == selected);
        return picker;
    }

    private static string? SelectedId(ComboBox picker) => (picker.SelectedItem as ComboBoxItem)?.Tag as string;

    // ---- Art previews ----

    private const double ArtSize = 64;

    /// <summary>
    /// A visual binding's fields beside a small preview of its art, drawn by the Rust renderer as
    /// the game draws it; picking another asset redraws it.
    /// </summary>
    private void AddVisual(Panel panel, JsonObject node, string path)
    {
        var fields = new StackPanel { Spacing = 8 };
        AddProperties(fields, node, typeof(VisualRef), path);
        var preview = Accessible(new Border
        {
            Name = $"ContentArt_{path}",
            Width = ArtSize + 10,
            Height = ArtSize + 10,
            Padding = new Thickness(4),
            VerticalAlignment = VerticalAlignment.Top,
        }.WithClasses("row"), "Art preview");
        Show(preview, ArtImage(node));
        var assetKey = Key<VisualRef>(nameof(VisualRef.AssetId));
        var pickerName = $"ContentField_{Join(path, nameof(VisualRef.AssetId))}";
        if (fields.Children.OfType<ComboBox>().FirstOrDefault(picker => picker.Name == pickerName) is { } picker)
        {
            picker.SelectionChanged += (_, _) =>
            {
                if (SelectedId(picker) is not { } id) return;
                var chosen = (JsonObject)node.DeepClone();
                chosen[assetKey] = id;
                Show(preview, ArtImage(chosen));
            };
        }

        panel.Children.Add(Columns("*,Auto", fields, preview));
    }

    /// <summary>A preview frame holds an image, or hides while there is none.</summary>
    private static void Show(Border frame, Image? image)
    {
        frame.Child = image;
        frame.IsVisible = image is not null;
    }

    /// <summary>The art a visual binding draws, or null when it draws nothing.</summary>
    private Image? ArtImage(JsonNode node)
    {
        VisualRef visual;
        try
        {
            visual = RecordJson.FromNode<VisualRef>(node);
        }
        catch (FormatException)
        {
            return null;
        }

        if (visual.AssetId.Length == 0) return null;
        if (_art is not null) return _art.Render(_project, visual, 0, ArtSize);
        using var art = new VisualPreview(_sharedPreview);
        return art.Render(_project, visual, 0, ArtSize);
    }

    // ---- Escape hatch ----

    private void AddJsonEscape(Panel panel, JsonObject target, string key, string path, string label, bool nullable)
    {
        var text = target[key]?.ToJsonString(InteropJson.Indented) ?? "null";
        var box = Accessible(new TextBox
        {
            Name = $"ContentJson_{path}",
            Text = text,
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
            MinHeight = 70,
            FontFamily = new FontFamily("avares://Tractr/Assets/Fonts#Courier Prime, fonts:Inter#Inter"),
        }, $"{label} as JSON");
        var apply = Accessible(_lists.SmallButton($"ContentJsonApply_{path}", "Apply JSON", "Replace this field with the JSON above", () => Structural(() => { })), $"Apply {label} JSON");
        _draft.JsonFields.Add(new ContentDraftViewModel.JsonField(label, () => box.Text, text, nullable, node => target[key] = node));
        panel.Children.Add(new Expander
        {
            Name = $"ContentJsonExpander_{path}",
            Header = "Edit as JSON",
            IsExpanded = false,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Content = Ui.VStack(6, box, apply),
        });
    }
}
