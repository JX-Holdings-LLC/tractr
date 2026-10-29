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

/// <summary>Generated collection rows and cards over the shared content draft.</summary>
internal sealed partial class ContentForm
{
    private sealed class ListEditor(ContentForm owner)
    {
        // ---- Lists ----

        public JsonArray EnsureArray(JsonObject target, string key)
        {
            if (target[key] is JsonArray array) return array;
            var created = new JsonArray();
            target[key] = created;
            return created;
        }

        public Button SmallButton(string name, string text, string tip, Action action, bool enabled = true)
        {
            var button = Accessible(Ui.Button(text, action, "tool", "small"), tip);
            button.Name = name;
            button.IsEnabled = enabled;
            ToolTip.SetTip(button, tip);
            return button;
        }

        public Control RowButtons(string path, int index, int count, JsonArray array, JsonObject target, string key, bool nullable, string what) =>
            Ui.HStack(4,
                SmallButton($"ContentUp_{path}_{index}", "↑", $"Move {what} up", () => owner.Structural(() => Move(array, index, -1)), index > 0),
                SmallButton($"ContentDown_{path}_{index}", "↓", $"Move {what} down", () => owner.Structural(() => Move(array, index, 1)), index < count - 1),
                SmallButton($"ContentRemove_{path}_{index}", "×", $"Remove {what}", () => owner.Structural(() =>
                {
                    array.RemoveAt(index);
                    if (array.Count == 0 && nullable) target[key] = null;
                })));

        public void AddRecordList(Panel panel, JsonObject target, string key, string path, Type element, bool nullable)
        {
            var array = target[key] as JsonArray;
            var what = Title(element.Name).ToLowerInvariant();
            for (var i = 0; i < (array?.Count ?? 0); i++)
            {
                var body = new StackPanel { Spacing = 6 };
                if (array![i] is JsonObject item) owner.AddProperties(body, item, element, $"{path}_{i}");
                else body.Children.Add(Ui.Wrapped("This entry is not an object; use Edit as JSON.", "muted", "small"));
                var header = RowHeader(Ui.Text($"{i + 1}.", "category"), RowButtons(path, i, array.Count, array, target, key, nullable, what));
                panel.Children.Add(ListRow(header, body));
            }

            panel.Children.Add(AddElementButton(path, $"Add {what}", $"Add a {what}", element, target, key));
        }

        /// <summary>Appends what F# says a new <paramref name="element"/> of this entry starts as.</summary>
        public Button AddElementButton(string path, string text, string tip, Type element, JsonObject target, string key) =>
            SmallButton($"ContentAdd_{path}", text, tip, () => owner.Structural(() =>
            {
                var list = EnsureArray(target, key);
                list.Add(NewElementNode(element, list));
            }));

        public JsonNode NewElementNode(Type element, JsonArray siblings)
        {
            var ids = siblings.OfType<JsonObject>().Select(sibling => StringOf(sibling["id"])).OfType<string>().ToList();
            var created = ContentForms.NewElement(element.Name, owner._project, owner.CurrentEntity(), ids) ?? DefaultOf(element);
            return RecordJson.ToNode(created, element);
        }

        public void AddNested(Panel panel, JsonObject target, string key, string path, Type type, bool nullable, string label)
        {
            var node = target[key] as JsonObject;
            if (nullable)
            {
                var include = new CheckBox { Name = $"ContentInclude_{path}", Content = $"Use {label.ToLowerInvariant()}", IsChecked = node is not null };
                include.IsCheckedChanged += (_, _) => owner.Structural(() =>
                {
                    if (include.IsChecked == true)
                    {
                        var created = ContentForms.NewElement(type.Name, owner._project, owner.CurrentEntity(), []) ?? DefaultOf(type);
                        target[key] = RecordJson.ToNode(created, type);
                    }
                    else target[key] = null;
                });
                panel.Children.Add(include);
            }

            if (node is null) return;
            if (type == typeof(VisualRef)) owner.AddVisual(panel, node, path);
            else owner.AddProperties(panel, node, type, path);
        }

        public void AddTextRows(Panel panel, JsonObject target, string key, string path, string label, bool nullable)
        {
            var array = target[key] as JsonArray;
            for (var i = 0; i < (array?.Count ?? 0); i++)
            {
                var index = i;
                var box = Accessible(new TextBox { Name = $"ContentField_{path}_{i}", Text = StringOf(array![i]) ?? array[i]?.ToJsonString() ?? "", MinWidth = 180 }, $"{label} {i + 1}");
                var original = box.Text;
                var before = array[i]?.DeepClone();
                owner._writers.Add(() =>
                {
                    if (box.Text != original) array[index] = JsonValue.Create(box.Text ?? "");
                    else if (!JsonNode.DeepEquals(array[index], before)) array[index] = before?.DeepClone();
                });
                panel.Children.Add(Ui.Row(box, SmallButton($"ContentRemove_{path}_{i}", "×", "Remove", () => owner.Structural(() =>
                {
                    array.RemoveAt(index);
                    if (array.Count == 0 && nullable) target[key] = null;
                }))));
            }

            panel.Children.Add(SmallButton($"ContentAdd_{path}", "Add", "Add a row", () => owner.Structural(() => EnsureArray(target, key).Add(JsonValue.Create("")))));
        }

        public void AddNumberRows(Panel panel, JsonObject target, string key, string path, string label, bool nullable)
        {
            var array = target[key] as JsonArray;
            var rows = new WrapPanel();
            for (var i = 0; i < (array?.Count ?? 0); i++)
            {
                var index = i;
                var holder = new JsonObject { ["value"] = array![i]?.DeepClone() };
                var box = owner.NumberControl(holder, "value", $"ContentField_{path}_{i}", $"{label} {i + 1}", integer: false, optional: false, min: null, max: null);
                owner._writers.Add(() =>
                {
                    if (!JsonNode.DeepEquals(holder["value"], array[index])) array[index] = holder["value"]?.DeepClone();
                });
                box.Width = 90;
                rows.Children.Add(Ui.HStack(2, box, SmallButton($"ContentRemove_{path}_{i}", "×", "Remove", () => owner.Structural(() =>
                {
                    array.RemoveAt(index);
                    if (array.Count == 0 && nullable) target[key] = null;
                }))));
            }

            panel.Children.Add(rows);
            panel.Children.Add(SmallButton($"ContentAdd_{path}", "Add", "Add a value", () => owner.Structural(() => EnsureArray(target, key).Add(JsonValue.Create(0.0)))));
        }

        /// <summary>Reference lists as chips with a picker that adds the ids not chosen yet.</summary>
        public void AddReferenceChips(Panel panel, JsonObject target, string key, string path, string label, string kind, bool nullable)
        {
            var array = target[key] as JsonArray;
            var ids = array?.Select(node => StringOf(node) ?? "").ToList() ?? [];
            var available = owner.Options(kind);
            var chips = new WrapPanel { Name = $"ContentChips_{path}" };
            var entries = ContentForms.ListEntries(available, ids);
            for (var i = 0; i < entries.Count; i++)
            {
                var index = i;
                var text = Ui.Text(entries[i].Label, entries[i].Missing ? "error" : "small");
                var remove = SmallButton($"ContentChipRemove_{path}_{i}", "×", $"Remove {entries[i].Label}", () => owner.Structural(() =>
                {
                    array!.RemoveAt(index);
                    if (array.Count == 0 && nullable) target[key] = null;
                }));
                chips.Children.Add(new Border { Name = $"ContentChip_{path}_{i}", Child = Ui.HStack(4, text, remove), Margin = new Thickness(0, 0, 6, 6) }.WithClasses("chip"));
            }

            if (entries.Count == 0) chips.Children.Add(Ui.Text(nullable ? "None (no restriction)" : "None", "muted", "small"));
            panel.Children.Add(chips);
            var add = Accessible(new ComboBox { Name = $"ContentChipAdd_{path}", PlaceholderText = "Add…", MinWidth = 200 }, $"Add to {label}");
            foreach (var option in available.Where(option => !ids.Contains(option.Id)))
            {
                var item = new ComboBoxItem { Content = option.Label, Tag = option.Id };
                TextSearch.SetText(item, option.Label);
                add.Items.Add(item);
            }

            add.IsEnabled = add.Items.Count > 0;
            add.SelectionChanged += (_, _) =>
            {
                if (ContentForm.SelectedId(add) is { } id) owner.Structural(() => EnsureArray(target, key).Add(JsonValue.Create(id)));
            };
            panel.Children.Add(add);
        }

        public void AddKeyValues(Panel panel, JsonObject target, string key, string path)
        {
            var entries = (target[key] as JsonObject)?.ToList() ?? [];
            var rows = new List<(TextBox Key, TextBox Value)>();
            var changed = false;
            for (var i = 0; i < entries.Count; i++)
            {
                var index = i;
                var keyBox = Accessible(new TextBox { Name = $"ContentKey_{path}_{i}", Text = entries[i].Key, Width = 140 }, $"Setting {i + 1} name");
                var value = entries[i].Value;
                var valueBox = Accessible(new TextBox { Name = $"ContentValue_{path}_{i}", Text = StringOf(value) ?? value?.ToJsonString() ?? "null", MinWidth = 160 }, $"Setting {i + 1} value");
                var originalKey = keyBox.Text;
                var originalValue = valueBox.Text;
                owner._writers.Add(() => changed |= keyBox.Text != originalKey || valueBox.Text != originalValue);
                rows.Add((keyBox, valueBox));
                panel.Children.Add(Ui.Row(Ui.HStack(6, keyBox, valueBox), SmallButton($"ContentRemove_{path}_{i}", "×", "Remove setting", () => owner.Structural(() =>
                {
                    var next = new JsonObject();
                    var current = (target[key] as JsonObject)?.ToList() ?? [];
                    for (var n = 0; n < current.Count; n++)
                    {
                        if (n != index) next[current[n].Key] = current[n].Value?.DeepClone();
                    }
                    target[key] = next;
                }))));
            }

            owner._writers.Add(() =>
            {
                if (!changed) return;
                var next = new JsonObject();
                foreach (var (keyBox, valueBox) in rows)
                {
                    var name = keyBox.Text?.Trim() ?? "";
                    if (name.Length > 0) next[name] = Literal(valueBox.Text ?? "");
                }
                target[key] = next;
                changed = false;
            });
            panel.Children.Add(SmallButton($"ContentAdd_{path}", "Add setting", "Add a setting", () => owner.Structural(() =>
            {
                if (target[key] is not JsonObject values) target[key] = values = new JsonObject();
                var n = values.Count + 1;
                while (values.ContainsKey($"setting{n}")) n++;
                values[$"setting{n}"] = "";
            })));
        }

        /// <summary>
        /// A built-in minigame kind's settings as typed fields (F# <c>MinigameKinds</c>), with a line on
        /// how the kind plays; a kind a pack defines (with that line) and a kind the game doesn't have
        /// keep the raw setting rows.
        /// </summary>
        public void AddMinigameSettings(Panel panel, JsonObject target, string key, string path)
        {
            var kind = StringOf(target[Key<MinigameDef>(nameof(MinigameDef.Kind))]) ?? "";
            var settings = ContentForms.MinigameSettings(kind);
            var help = ContentForms.MinigameHelp(kind, owner._project);
            if (settings.Count == 0)
            {
                if (help.Length > 0) panel.Children.Add(Ui.Text(help, "muted", "small"));
                AddKeyValues(panel, target, key, path);
                return;
            }

            if (target[key] is not JsonObject config) target[key] = config = new JsonObject();
            panel.Children.Add(Ui.Text(help, "muted", "small"));
            panel.Children.Add(owner._vocabulary.VocabularyFields(config, settings, path));
            // A cleared setting goes away (the game then plays its default) instead of staying as null.
            owner._writers.Add(() =>
            {
                foreach (var name in config.Where(entry => entry.Value is null).Select(entry => entry.Key).ToList()) config.Remove(name);
            });
        }

        // ---- Compact rows and cards (web NPCEditor's schedule and waypoints, ShopEditor's stock) ----

        /// <summary>A whole-number tile coordinate box.</summary>
        public TextBox Coordinate(JsonObject target, string key, string name, string label, string placeholder)
        {
            var box = owner.NumberControl(target, key, name, label, integer: true, optional: false, min: null, max: null);
            box.MinWidth = 0;
            box.Width = 64;
            box.PlaceholderText = placeholder;
            return box;
        }

        /// <summary>
        /// Web NPCEditor's daily schedule: a row per entry with the minute of day and its clock time,
        /// the scene, x and y. New entries come from F# (8:00 AM at the NPC's own scene and tile).
        /// </summary>
        public void AddScheduleRows(Panel panel, JsonObject target, string key, string path, bool nullable)
        {
            var array = target[key] as JsonArray;
            var count = array?.Count ?? 0;
            var sceneField = ContentForms.Field(nameof(NpcScheduleEntry), nameof(NpcScheduleEntry.SceneId), "Scene");
            for (var i = 0; i < count; i++)
            {
                var title = $"Schedule entry {i + 1}";
                var buttons = RowButtons(path, i, count, array!, target, key, nullable, $"schedule entry {i + 1}");
                if (array![i] is not JsonObject entry)
                {
                    panel.Children.Add(NotAnObject(buttons));
                    continue;
                }

                var minute = owner.NumberControl(entry, Key<NpcScheduleEntry>(nameof(NpcScheduleEntry.Minute)), $"ContentField_{path}_{i}_Minute", $"{title} minute",
                    integer: true, optional: false, min: 0, max: 1560);
                minute.MinWidth = 0;
                ToolTip.SetTip(minute, "Minute of day (480 = 8:00 AM)");
                var clock = Ui.Text(ClockText(minute.Text), "muted", "small");
                clock.Name = $"ContentClock_{path}_{i}";
                minute.TextChanged += (_, _) => clock.Text = ClockText(minute.Text);
                var scene = owner.StringControl(entry, Key<NpcScheduleEntry>(nameof(NpcScheduleEntry.SceneId)), $"ContentField_{path}_{i}_SceneId", $"{title} scene",
                    sceneField, nullable: false, multiline: false, rebuildOnChange: false);
                scene.MinWidth = 120;
                var x = Coordinate(entry, Key<NpcScheduleEntry>(nameof(NpcScheduleEntry.X)), $"ContentField_{path}_{i}_X", $"{title} x", "x");
                var y = Coordinate(entry, Key<NpcScheduleEntry>(nameof(NpcScheduleEntry.Y)), $"ContentField_{path}_{i}_Y", $"{title} y", "y");
                string Scene() => scene switch
                {
                    ComboBox picker => ContentForm.SelectedId(picker) ?? "",
                    TextBox box => box.Text ?? "",
                    _ => "",
                };
                var pick = owner._vocabulary.PickButton($"ContentPick_{path}_{i}", $"where schedule entry {i + 1} goes", Scene, x, y);
                var row = pick is null
                    ? Columns("72,118,*,Auto,Auto,Auto", minute, clock, scene, x, y, buttons)
                    : Columns("72,118,*,Auto,Auto,Auto,Auto", minute, clock, scene, x, y, pick, buttons);
                row.Name = $"ContentRow_{path}_{i}";
                panel.Children.Add(row);
            }

            if (count == 0) panel.Children.Add(EmptyNote(path, "No schedule — the NPC stays put (or wanders/patrols)."));
            panel.Children.Add(AddElementButton(path, "Add schedule entry", "Add schedule entry", typeof(NpcScheduleEntry), target, key));
        }

        /// <summary>Web NPCEditor's patrol waypoints: "1. x y" rows; new points start at the NPC's tile (F#).</summary>
        public void AddWaypointRows(Panel panel, JsonObject target, string key, string path, bool nullable)
        {
            var array = target[key] as JsonArray;
            var count = array?.Count ?? 0;
            for (var i = 0; i < count; i++)
            {
                var title = $"Waypoint {i + 1}";
                var buttons = RowButtons(path, i, count, array!, target, key, nullable, $"waypoint {i + 1}");
                if (array![i] is not JsonObject point)
                {
                    panel.Children.Add(NotAnObject(buttons));
                    continue;
                }

                var number = Ui.Text($"{i + 1}.", "muted", "small");
                number.MinWidth = 18;
                var x = Coordinate(point, Key<GridPoint>(nameof(GridPoint.X)), $"ContentField_{path}_{i}_X", $"{title} x", "x");
                var y = Coordinate(point, Key<GridPoint>(nameof(GridPoint.Y)), $"ContentField_{path}_{i}_Y", $"{title} y", "y");
                var row = Ui.HStack(6, number, x, y, buttons);
                if (owner._vocabulary.PickButton($"ContentPick_{path}_{i}", $"waypoint {i + 1}", () => StringOf(owner.Draft["sceneId"]) ?? "", x, y) is { } pick)
                {
                    row.Children.Insert(3, pick);
                }
                row.Name = $"ContentRow_{path}_{i}";
                panel.Children.Add(row);
            }

            if (count == 0) panel.Children.Add(EmptyNote(path, "No waypoints yet."));
            panel.Children.Add(AddElementButton(path, "Add waypoint", "Add waypoint", typeof(GridPoint), target, key));
        }

        /// <summary>
        /// Web ShopEditor's stock: a card per entry with the item (its base value in the label), a price
        /// override over that base, a daily limit and the calendar's seasons as checkboxes. Keys the form
        /// does not know stay in the entry.
        /// </summary>
        public void AddStockCards(Panel panel, JsonObject target, string key, string path, bool nullable)
        {
            var array = target[key] as JsonArray;
            var count = array?.Count ?? 0;
            var itemField = ContentForms.Field(nameof(ShopStockEntry), nameof(ShopStockEntry.ItemId), "Item");
            var itemKey = Key<ShopStockEntry>(nameof(ShopStockEntry.ItemId));
            for (var i = 0; i < count; i++)
            {
                var title = $"Stock {i + 1}";
                var buttons = RowButtons(path, i, count, array!, target, key, nullable, $"stock entry {i + 1}");
                if (array![i] is not JsonObject entry)
                {
                    panel.Children.Add(NotAnObject(buttons));
                    continue;
                }

                var item = owner.StringControl(entry, itemKey, $"ContentField_{path}_{i}_ItemId", $"{title} item", itemField, nullable: false, multiline: false, rebuildOnChange: false,
                    optionLabel: StockLabel);
                var price = owner.NumberControl(entry, Key<ShopStockEntry>(nameof(ShopStockEntry.Price)), $"ContentField_{path}_{i}_Price", $"{title} price override",
                    integer: false, optional: true, min: null, max: null);
                price.PlaceholderText = BaseValueText(StringOf(entry[itemKey]));
                if (item is ComboBox picker) picker.SelectionChanged += (_, _) => price.PlaceholderText = BaseValueText(ContentForm.SelectedId(picker));
                var limit = LimitControl(entry, Key<ShopStockEntry>(nameof(ShopStockEntry.DailyLimit)), $"ContentField_{path}_{i}_DailyLimit", $"{title} daily limit");
                var seasons = SeasonChecks(entry, Key<ShopStockEntry>(nameof(ShopStockEntry.Seasons)), $"{path}_{i}", title);
                var card = Ui.VStack(8,
                    Columns("*,Auto", item, buttons),
                    Columns("*,*", Labeled("Price override", price), Labeled("Daily limit", limit)),
                    Ui.VStack(4, Label("Seasons (none = all seasons)"), seasons));
                panel.Children.Add(new Border { Name = $"ContentCard_{path}_{i}", Child = card }.WithClasses("row"));
            }

            if (count == 0) panel.Children.Add(EmptyNote(path, "No stock entries. Add one to sell items."));
            panel.Children.Add(AddElementButton(path, "Add stock entry", "Add stock entry", typeof(ShopStockEntry), target, key));
        }

        /// <summary>The effective items (the project's, or the built-in catalog) by id.</summary>
        public Dictionary<string, Item> ItemsById() =>
            owner._items ??= ProjectContent.Compile(owner._project).Items.DistinctBy(item => item.Id).ToDictionary(item => item.Id);

        /// <summary>"Wheat ($25)": an item with its base value (web ShopEditor's picker).</summary>
        public string StockLabel(PickerOption option) =>
            ItemsById().TryGetValue(option.Id, out var item)
                ? $"{(string.IsNullOrWhiteSpace(item.Name) ? item.Id : item.Name)} ({ContentReadouts.Money(item.Value)})"
                : option.Label;

        /// <summary>The price placeholder: "25 (base)", the item's value when there is no override.</summary>
        public string BaseValueText(string? itemId) =>
            itemId is not null && ItemsById().TryGetValue(itemId, out var item) ? $"{Ui.Num(item.Value)} (base)" : "Base value";

        /// <summary>A whole-number limit; empty, zero or less removes it (web "Unlimited").</summary>
        public TextBox LimitControl(JsonObject target, string key, string name, string label)
        {
            var box = Accessible(new TextBox { Name = name, Text = NumberText(target[key]), MinWidth = 90, PlaceholderText = "Unlimited" }, label);
            var original = box.Text;
            owner.Writer(target, key, () => (box.Text ?? "").Trim() == original, () =>
            {
                var raw = (box.Text ?? "").Trim();
                if (raw.Length == 0)
                {
                    target[key] = null;
                    return;
                }

                if (!double.TryParse(raw, NumberStyles.Float, CultureInfo.InvariantCulture, out var value) || !double.IsFinite(value) || value != Math.Floor(value))
                {
                    owner._errors.Add($"{label} must be a whole number.");
                    return;
                }

                target[key] = value > 0 ? JsonValue.Create(value) : null;
            });
            return box;
        }

        /// <summary>
        /// A season filter as a checkbox per calendar season: none checked means every season and is
        /// stored as absent. Ids the calendar no longer has stay listed as "(missing: id)" until unchecked.
        /// </summary>
        public WrapPanel SeasonChecks(JsonObject target, string key, string path, string title)
        {
            var ids = (target[key] as JsonArray)?.Select(node => StringOf(node) ?? "").ToList() ?? [];
            var calendar = owner.Options("season");
            var seasons = calendar.Concat(ContentForms.ListEntries(calendar, ids).Where(entry => entry.Missing)).DistinctBy(entry => entry.Id).ToList();
            var wrap = new WrapPanel { Name = $"ContentSeasons_{path}" };
            var checks = new List<(string Id, CheckBox Check)>();
            for (var i = 0; i < seasons.Count; i++)
            {
                var check = new CheckBox
                {
                    Name = $"ContentSeason_{path}_{i}",
                    Content = seasons[i].Label,
                    IsChecked = ids.Contains(seasons[i].Id),
                    Margin = new Thickness(0, 0, 14, 0),
                };
                checks.Add((seasons[i].Id, Accessible(check, $"{title} season {seasons[i].Label}")));
                wrap.Children.Add(check);
            }

            var original = checks.Select(entry => entry.Check.IsChecked).ToList();
            owner.Writer(target, key, () => checks.Select(entry => entry.Check.IsChecked).SequenceEqual(original), () =>
            {
                var chosen = checks.Where(entry => entry.Check.IsChecked == true).Select(entry => (JsonNode?)JsonValue.Create(entry.Id)).ToArray();
                target[key] = chosen.Length == 0 ? null : new JsonArray(chosen);
            });
            return wrap;
        }
    }
}
