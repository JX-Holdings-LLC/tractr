using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Input.Platform;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;
using FarmEngine.Authoring;
using Tractr.App.Projects;
using Tractr.App.ViewModels.EditMode;

namespace Tractr.App.Game;

/// <summary>Shared frame of the authoring tool windows: sized, closable with Escape.</summary>
public abstract class AuthoringToolWindow : Window
{
    protected AuthoringToolWindow(string name, string title)
    {
        Name = name;
        Title = title;
        Width = 900;
        Height = 640;
        MinWidth = 520;
        MinHeight = 360;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        ShowInTaskbar = false;
    }

    protected override void OnKeyDown(KeyEventArgs e)
    {
        base.OnKeyDown(e);
        if (!e.Handled && e.Key == Key.Escape)
        {
            e.Handled = true;
            Close();
        }
    }

    protected static TextBox MessageBox(string name) => new()
    {
        Name = name,
        IsReadOnly = true,
        TextWrapping = TextWrapping.Wrap,
        AcceptsReturn = true,
        MaxHeight = 160,
    };
}

/// <summary>
/// The dialogue script editor (#225): an NPC's dialogues and lines, or the project's cutscenes,
/// as text (docs/AUTHORING-AT-SCALE.md). Check reports mistakes by line; Apply writes the script
/// into the project as one undo step.
/// </summary>
public sealed class ScriptEditorWindow : AuthoringToolWindow
{
    public ScriptEditorWindow(ProjectWorkspace workspace, string? npcId)
        : base("ScriptEditorWindow", "")
    {
        ViewModel = new ScriptEditorViewModel(workspace, npcId);
        Title = ViewModel.Title;
        var text = new TextBox
        {
            Name = "ScriptText",
            AcceptsReturn = true,
            AcceptsTab = false,
            TextWrapping = TextWrapping.NoWrap,
            Text = ViewModel.Text,
        };
        Ui.Label((text, "Script"));
        text.TextChanged += (_, _) => ViewModel.Text = text.Text ?? "";
        var messages = MessageBox("ScriptMessages");
        Ui.Label((messages, "Script problems"));
        void ShowMessages() => messages.Text = string.Join("\n", ViewModel.Messages);
        ShowMessages();
        var check = Ui.Button("Check", () =>
        {
            ViewModel.Check();
            ShowMessages();
        }, "tool");
        check.Name = "CheckScriptButton";
        var reload = Ui.Button("Reload", () =>
        {
            ViewModel.Reload();
            text.Text = ViewModel.Text;
            ShowMessages();
        }, "tool");
        reload.Name = "ReloadScriptButton";
        var apply = Ui.Button("Apply script", () =>
        {
            if (ViewModel.Apply()) text.Text = ViewModel.Text;
            ShowMessages();
        }, "accent");
        apply.Name = "ApplyScriptButton";
        var layout = new DockPanel { Margin = new Thickness(16) };
        var top = Ui.Wrapped("=== DIALOGUE-ID starts a dialogue, * an answer, ~ its effect, [CONDITIONS] a line's conditions. Help → Creator Guide links the full format.", "muted", "small");
        DockPanel.SetDock(top, Dock.Top);
        var bottom = Ui.VStack(8, messages, Ui.HStack(8, apply, check, reload));
        bottom.Margin = new Thickness(0, 10, 0, 0);
        DockPanel.SetDock(bottom, Dock.Bottom);
        layout.Children.Add(top);
        layout.Children.Add(bottom);
        layout.Children.Add(new ScrollViewer { Content = text, Margin = new Thickness(0, 8, 0, 0) });
        Content = layout;
    }

    public ScriptEditorViewModel ViewModel { get; }
}

/// <summary>
/// Content tables (#226, #227): one category's entries as rows of cells (computed columns
/// read-only), filtered by text or tag; a cell commits when it loses focus, a paste of
/// tab-separated text fills many rows; CSV export and import with a diff; bulk edits and tags
/// over the shown rows, previewed before they apply. Every change is one undo step.
/// </summary>
public sealed class TableEditorWindow : AuthoringToolWindow
{
    private const double CellWidth = 130;
    private readonly StackPanel _grid = new() { Name = "TableRows", Spacing = 2 };
    private readonly TextBox _message = MessageBox("TableMessage");
    private readonly ComboBox _column = new() { Name = "BulkColumn", MinWidth = 150 };
    private readonly Button _apply;

    public TableEditorWindow(ProjectWorkspace workspace)
        : base("TableEditorWindow", "Content tables")
    {
        ViewModel = new TableEditorViewModel(workspace);
        var category = new ComboBox { Name = "TableCategory", MinWidth = 160 };
        foreach (var id in TableEditorViewModel.Categories) category.Items.Add(new ComboBoxItem { Content = AuthoringTools.TableName(id), Tag = id });
        category.SelectedIndex = 0;
        category.SelectionChanged += (_, _) =>
        {
            if (category.SelectedItem is ComboBoxItem { Tag: string id }) ViewModel.Category = id;
            Show();
        };
        var filter = new TextBox { Name = "TableFilter", PlaceholderText = "Filter rows", MinWidth = 160 };
        filter.TextChanged += (_, _) =>
        {
            ViewModel.Filter = filter.Text ?? "";
            ShowRows();
        };
        var tag = new TextBox { Name = "TableTag", PlaceholderText = "Tag", MinWidth = 100 };
        tag.TextChanged += (_, _) =>
        {
            ViewModel.Tag = tag.Text ?? "";
            ShowRows();
        };
        Ui.Label((category, "Table"), (filter, "Filter rows"), (tag, "Only rows with tag"), (_column, "Column"), (_message, "Table messages"));

        var operation = new TextBox { Name = "BulkOperation", PlaceholderText = "= 50, × 1.1, + 1, + summer", MinWidth = 170 };
        Ui.Label((operation, "Bulk edit"));
        var preview = Ui.Button("Preview", () =>
        {
            if (_column.SelectedItem is ComboBoxItem { Tag: string key }) ViewModel.PreviewBulk(key, operation.Text ?? "");
            ShowMessage();
        }, "tool");
        preview.Name = "PreviewBulkButton";
        var addTag = Ui.Button("Tag shown rows", () =>
        {
            ViewModel.TagShown(operation.Text ?? "", false);
            ShowMessage();
            ShowRows();
        }, "tool");
        addTag.Name = "TagRowsButton";
        _apply = Ui.Button("Apply", () =>
        {
            ViewModel.ApplyPending();
            ShowMessage();
            ShowRows();
        }, "accent");
        _apply.Name = "ApplyTableButton";
        var exportCsv = Ui.AsyncButton("Copy CSV", async () =>
        {
            if (Clipboard is { } clipboard) await clipboard.SetTextAsync(ViewModel.ExportCsv());
            _message.Text = "The table is on the clipboard as CSV.";
        }, error => _message.Text = error.Message, "tool");
        exportCsv.Name = "ExportCsvButton";
        var importCsv = Ui.AsyncButton("Import CSV from clipboard", async () =>
        {
            var text = Clipboard is { } clipboard ? await clipboard.TryGetTextAsync() : null;
            ViewModel.PreviewImport(text ?? "");
            ShowMessage();
        }, error => _message.Text = error.Message, "tool");
        importCsv.Name = "ImportCsvButton";

        var top = Ui.VStack(8, Ui.HStack(8, category, filter, tag), Ui.HStack(8, _column, operation, preview, addTag), Ui.HStack(8, exportCsv, importCsv, _apply));
        DockPanel.SetDock(top, Dock.Top);
        var bottom = _message;
        bottom.Margin = new Thickness(0, 10, 0, 0);
        DockPanel.SetDock(bottom, Dock.Bottom);
        var layout = new DockPanel { Margin = new Thickness(16) };
        layout.Children.Add(top);
        layout.Children.Add(bottom);
        layout.Children.Add(new ScrollViewer
        {
            Content = _grid,
            HorizontalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Auto,
            Margin = new Thickness(0, 10, 0, 0),
        });
        Content = layout;
        Show();
    }

    public TableEditorViewModel ViewModel { get; }

    private new void Show()
    {
        _column.Items.Clear();
        foreach (var column in ViewModel.Columns)
        {
            if (!column.ReadOnly) _column.Items.Add(new ComboBoxItem { Content = column.Header, Tag = column.Key });
        }

        _column.SelectedIndex = _column.Items.Count > 1 ? 1 : 0;
        ShowRows();
        ShowMessage();
    }

    private void ShowMessage()
    {
        _message.Text = ViewModel.Message;
        _apply.IsEnabled = ViewModel.HasPending;
    }

    private void ShowRows()
    {
        _grid.Children.Clear();
        var columns = ViewModel.Columns;
        var header = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 2 };
        foreach (var column in columns) header.Children.Add(new TextBlock { Text = column.Header, Width = CellWidth, FontWeight = FontWeight.Bold });
        _grid.Children.Add(header);
        foreach (var row in ViewModel.Rows)
        {
            var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 2 };
            var cells = row.Cells.ToList();
            for (var k = 0; k < columns.Count && k < cells.Count; k++)
            {
                var column = columns[k];
                var rowId = row.Id;
                var shown = cells[k];
                var cell = new TextBox { Text = shown, Width = CellWidth, IsReadOnly = column.ReadOnly };
                Ui.Label((cell, $"{column.Header} of {rowId}"));
                cell.LostFocus += (_, _) =>
                {
                    var text = cell.Text ?? "";
                    if (text == shown) return;
                    if (text.Contains('\t') || text.Contains('\n'))
                    {
                        ViewModel.Paste(rowId, column.Key, text);
                    }
                    else
                    {
                        ViewModel.SetCell(rowId, column.Key, text);
                    }

                    ShowMessage();
                    Dispatcher.UIThread.Post(ShowRows);
                };
                line.Children.Add(cell);
            }

            _grid.Children.Add(line);
        }
    }
}
