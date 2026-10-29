using Avalonia;
using Avalonia.Controls;
using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using Tractr.App.Localization;

namespace Tractr.App.Game;

public sealed partial class EditModeView
{
    private void EditForageSpawns()
    {
        if (_workspace.Current is not { } project || CurrentScene() is not { } scene || TopLevel.GetTopLevel(this) is not Window owner) return;
        var window = new Window { Title = EditorStrings.Get("scene.forage"), Width = 680, Height = 720, WindowStartupLocation = WindowStartupLocation.CenterOwner };
        var root = new StackPanel { Spacing = 12, Margin = new Thickness(16) };
        var rows = new StackPanel { Spacing = 12 };
        var status = Ui.Wrapped("", "muted", "small");
        var forms = new List<ContentForm>();
        void Add(ForageSpawnTable table)
        {
            var row = new StackPanel { Spacing = 8 };
            var fields = new StackPanel { Spacing = 8 };
            var preview = Ui.Wrapped("", "muted", "small");
            var form = new ContentForm(project, table, typeof(ForageSpawnTable), fields, message => status.Text = message);
            forms.Add(form);
            void Preview()
            {
                try
                {
                    var points = SettingsForm.ForageEligibleTiles(project, scene, (ForageSpawnTable)form.Commit());
                    preview.Text = $"{points.Count}: " + string.Join(", ", points.Take(80).Select(p => $"({p.X}, {p.Y})"));
                }
                catch (Exception error) when (error is FormatException or System.Text.Json.JsonException) { status.Text = error.Message; }
            }
            row.Children.Add(fields);
            row.Children.Add(Ui.HStack(8,
                Ui.Button(EditorStrings.Get("scene.foragePreview"), Preview),
                Ui.Button(EditorStrings.Get("scene.forageRemove"), () => { forms.Remove(form); rows.Children.Remove(row); })));
            row.Children.Add(preview);
            rows.Children.Add(row);
            Preview();
        }
        foreach (var table in scene.ForageSpawns.OrEmpty()) Add(table);
        root.Children.Add(rows);
        root.Children.Add(Ui.Button(EditorStrings.Get("scene.forageAdd"), () => Add(ForageSpawnTable.Default)));
        root.Children.Add(status);
        root.Children.Add(Ui.Button(EditorStrings.Get("scene.saveSurroundings"), () =>
        {
            try
            {
                var tables = forms.Select(form => (ForageSpawnTable)form.Commit()).ToArray();
                ViewModel.Apply(Edits.SetSceneForageSpawns(scene.Id, tables));
                window.Close();
            }
            catch (Exception error) when (error is FormatException or System.Text.Json.JsonException) { status.Text = error.Message; }
        }, "primary"));
        window.Content = new ScrollViewer { Content = root };
        _ = window.ShowDialog(owner);
    }
}
