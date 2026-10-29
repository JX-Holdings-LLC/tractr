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

/// <summary>Generated condition and outcome fields over the shared content draft.</summary>
internal sealed partial class ContentForm
{
    private sealed class VocabularyEditor(ContentForm owner)
    {
        // ---- Condition and outcome vocabulary ----

        public ComboBox TypePicker(string name, IReadOnlyList<PickerOption> types, string current, string placeholder)
        {
            var picker = Accessible(new ComboBox { Name = name, MinWidth = 200, PlaceholderText = placeholder }, placeholder.TrimStart('+', ' '));
            foreach (var option in types)
            {
                picker.Items.Add(new ComboBoxItem { Content = option.Label, Tag = option.Id });
            }

            if (current.Length > 0 && !types.Any(option => option.Id == current))
                picker.Items.Insert(0, new ComboBoxItem { Content = $"(legacy: {current})", Tag = current });
            picker.SelectedItem = picker.Items.OfType<ComboBoxItem>().FirstOrDefault(item => (string)item.Tag! == current);
            return picker;
        }

        public void AddConditions(Panel panel, JsonObject target, string key, string path)
        {
            var array = target[key] as JsonArray;
            for (var i = 0; i < (array?.Count ?? 0); i++)
            {
                var index = i;
                var condition = array![i] as JsonObject ?? new JsonObject();
                var type = StringOf(condition["type"]) ?? "";
                var picker = TypePicker($"ContentField_{path}_{i}_Type", ContentForms.ConditionTypes, type, "Condition type");
                picker.SelectionChanged += (_, _) =>
                {
                    if (ContentForm.SelectedId(picker) is { } next && next != type)
                        owner.Structural(() => array[index] = RecordJson.ToNode(ContentForms.DefaultCondition(next, owner._project), typeof(EventCondition)));
                };
                var header = RowHeader(picker, owner._lists.RowButtons(path, i, array.Count, array, target, key, nullable: false, "condition"));
                panel.Children.Add(ListRow(header, VocabularyFields(condition, ContentForms.ConditionFields(type), $"{path}_{i}")));
            }

            var add = TypePicker($"ContentAdd_{path}", ContentForms.ConditionTypes, "", "+ Add condition");
            add.SelectionChanged += (_, _) =>
            {
                if (ContentForm.SelectedId(add) is { } type)
                    owner.Structural(() => owner._lists.EnsureArray(target, key).Add(RecordJson.ToNode(ContentForms.DefaultCondition(type, owner._project), typeof(EventCondition))));
            };
            panel.Children.Add(add);
        }

        public void AddOutcomes(Panel panel, JsonObject target, string key, string path)
        {
            var array = target[key] as JsonArray;
            for (var i = 0; i < (array?.Count ?? 0); i++)
            {
                var index = i;
                var outcome = array![i] as JsonObject ?? new JsonObject();
                var type = StringOf(outcome["type"]) ?? "";
                var picker = TypePicker($"ContentField_{path}_{i}_Type", ContentForms.OutcomeTypes, type, "Outcome type");
                picker.SelectionChanged += (_, _) =>
                {
                    if (ContentForm.SelectedId(picker) is { } next && next != type)
                        owner.Structural(() => array[index] = RecordJson.ToNode(ContentForms.DefaultOutcome(next)));
                };
                var title = Ui.HStack(8, Ui.Text($"{i + 1}.", "category"), picker);
                var header = RowHeader(title, owner._lists.RowButtons(path, i, array.Count, array, target, key, nullable: false, "outcome"));
                if (type == "applyBuff")
                {
                    var buffFields = new StackPanel { Spacing = 6 };
                    if (outcome["buff"] is not JsonObject buff)
                    {
                        buff = (JsonObject)RecordJson.ToNode(BuffDefinition.Default)!;
                        outcome["buff"] = buff;
                    }
                    owner.AddProperties(buffFields, buff, typeof(BuffDefinition), $"{path}_{i}_Buff");
                    panel.Children.Add(ListRow(header, buffFields));
                }
                else panel.Children.Add(ListRow(header, VocabularyFields(outcome, ContentForms.OutcomeFields(type), $"{path}_{i}")));
            }

            var add = TypePicker($"ContentAdd_{path}", ContentForms.OutcomeTypes, "", "+ Add outcome");
            add.SelectionChanged += (_, _) =>
            {
                if (ContentForm.SelectedId(add) is { } type)
                    owner.Structural(() => owner._lists.EnsureArray(target, key).Add(RecordJson.ToNode(ContentForms.DefaultOutcome(type))));
            };
            panel.Children.Add(add);
        }

        /// <summary>The fields F# lists for one condition or outcome type, laid out in a wrap.</summary>
        public Control VocabularyFields(JsonObject target, IReadOnlyList<FormField> fields, string path)
        {
            var wrap = new WrapPanel();
            var stack = new StackPanel { Spacing = 6 };
            var numbers = new Dictionary<string, TextBox>(StringComparer.Ordinal);
            foreach (var field in fields)
            {
                var name = $"ContentField_{path}_{Pascal(field.Key)}";
                switch (field.Kind)
                {
                    case "integer" or "number":
                        var number = owner.NumberControl(target, field.Key, name, field.Label, field.Kind == "integer", field.Optional,
                            field.HasMin ? field.Min : null, field.HasMax ? field.Max : null);
                        numbers[field.Key] = number;
                        number.PlaceholderText = field.Optional ? (field.Placeholder.Length > 0 ? field.Placeholder : "optional") : null;
                        wrap.Children.Add(Labeled(field.Label, number, 130));
                        break;
                    case "bool":
                        wrap.Children.Add(owner.BoolControl(target, field.Key, name, field.OnLabel, whenAbsent: true));
                        break;
                    case "choice" or "reference":
                        wrap.Children.Add(Labeled(field.Label, owner.StringControl(target, field.Key, name, field.Label, field, field.Optional, multiline: false, rebuildOnChange: false), 240));
                        break;
                    case "conditions":
                        // A condition group (M3): its own list of conditions, nested.
                        var group = new StackPanel { Spacing = 4, Margin = new Thickness(12, 0, 0, 0) };
                        group.Children.Add(Label(field.Label));
                        AddConditions(group, target, field.Key, $"{path}_{Pascal(field.Key)}");
                        stack.Children.Add(group);
                        break;
                    case "referenceList":
                        var chips = new StackPanel { Spacing = 4 };
                        chips.Children.Add(Label(field.Label));
                        owner._lists.AddReferenceChips(chips, target, field.Key, $"{path}_{Pascal(field.Key)}", field.Label, field.Reference, nullable: false);
                        stack.Children.Add(chips);
                        break;
                    default:
                        var text = (TextBox)owner.StringControl(target, field.Key, name, field.Label, null, field.Optional, multiline: false, rebuildOnChange: false);
                        text.PlaceholderText = field.Placeholder;
                        text.MinWidth = 240;
                        wrap.Children.Add(Labeled(field.Label, text));
                        break;
                }
            }

            // Tile coordinates can be clicked on the map: the outcome's own scene, else the event's.
            string Scene() => StringOf(target["sceneId"]) is { Length: > 0 } own ? own : StringOf(owner.Draft["sceneId"]) ?? "";
            foreach (var (xKey, yKey, what) in new[] { ("x", "y", "the tile"), ("tileX", "tileY", "the tile"), ("x2", "y2", "the far corner") })
            {
                if (numbers.TryGetValue(xKey, out var x) && numbers.TryGetValue(yKey, out var y)
                    && PickButton($"ContentPick_{path}_{Pascal(xKey)}", what, Scene, x, y) is { } pick)
                {
                    wrap.Children.Add(pick);
                }
            }

            foreach (var child in wrap.Children.OfType<Control>()) child.Margin = new Thickness(0, 0, 10, 6);
            if (wrap.Children.Count > 0) stack.Children.Insert(0, wrap);
            return stack;
        }

        /// <summary>A "Pick on map" button that fills <paramref name="x"/> and <paramref name="y"/> with a clicked tile (#47).</summary>
        public Button? PickButton(string name, string what, Func<string> scene, TextBox x, TextBox y)
        {
            if (owner._pickOnMap is not { } pickOnMap) return null;
            var button = owner._lists.SmallButton(name, "Pick on map", $"Pick {what} on the map", () => pickOnMap(scene(), $"Click {what}.", (tileX, tileY) =>
            {
                x.Text = tileX.ToString(CultureInfo.InvariantCulture);
                y.Text = tileY.ToString(CultureInfo.InvariantCulture);
                owner._report($"Tile ({tileX}, {tileY}) chosen. Save changes to keep it.");
            }));
            button.VerticalAlignment = VerticalAlignment.Bottom;
            return button;
        }
    }
}
