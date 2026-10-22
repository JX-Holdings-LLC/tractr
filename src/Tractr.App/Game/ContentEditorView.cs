using System.ComponentModel;
using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Data;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;
using FarmEngine.Schemas;
using Tractr.App.Projects;
using Tractr.App.ViewModels.EditMode;

namespace Tractr.App.Game;

/// <summary>
/// The Content tab: shows its <see cref="ContentEditorViewModel"/> (the types, the entry list,
/// the readouts) and builds the generated <see cref="ContentForm"/> for the entry it edits: nested
/// records, lists, reference pickers and the condition/outcome editors, each with an "Edit as
/// JSON" box. The lists show each entry's art (drawn by the Rust renderer); recipe and crop
/// profits and the crop and node type summary cards follow the fields while they are edited.
/// </summary>
public sealed class ContentEditorView : UserControl, IRetirable
{
    private const double ThumbnailSize = 28;

    /// <summary>The entry list's width until the creator drags the splitter.</summary>
    public const double DefaultListWidth = 230;

    /// <summary>What the view shows of its view model, in the order it is first shown.</summary>
    private static readonly string[] Shown =
    [
        nameof(ContentEditorViewModel.SelectedCategory), nameof(ContentEditorViewModel.Rows), nameof(ContentEditorViewModel.FormEntity),
        nameof(ContentEditorViewModel.Summary), nameof(ContentEditorViewModel.ProfitName), nameof(ContentEditorViewModel.ShowDuplicate),
        nameof(ContentEditorViewModel.ShowAddToInventory), nameof(ContentEditorViewModel.ShowCustomize), nameof(ContentEditorViewModel.DeleteTip),
        nameof(ContentEditorViewModel.CustomizeTip),
    ];

    private readonly ProjectWorkspace _workspace;
    private readonly ComboBox _category = new() { Name = "ContentCategory", MinWidth = 175 };
    private readonly ListBox _entities = new() { Name = "ContentEntities", MinHeight = 250 };
    private readonly StackPanel _form = new() { Name = "ContentFields", Spacing = 10 };
    private readonly Border _summary = new() { Name = "ContentSummary", IsVisible = false };
    private readonly StackPanel _readouts = new() { Name = "ContentReadouts", Spacing = 6 };
    private readonly Button _duplicate;
    private readonly Button _addToInventory;
    private readonly Button _delete;
    private readonly Button _customize;
    private readonly VisualPreview _art = new();
    private readonly List<(Border Holder, VisualRef Visual)> _thumbnails = [];
    private bool _refreshing;
    private bool _attached;
    private bool _readoutsQueued;
    private ContentForm? _contentForm;
    private TextBlock? _profit;

    private readonly Action<string, string, Action<int, int>>? _pickOnMap;

    /// <param name="workspace">The open project.</param>
    /// <param name="pickOnMap">"Pick on map" for the forms' tile coordinates (scene id, prompt, what to do with the tile); null hides it.</param>
    public ContentEditorView(ProjectWorkspace workspace, Action<string, string, Action<int, int>>? pickOnMap = null)
    {
        _workspace = workspace;
        _pickOnMap = pickOnMap;
        ViewModel = new ContentEditorViewModel(workspace, () => IsEffectivelyVisible);
        DataContext = ViewModel;
        Name = "ContentEditorView";
        var message = Ui.Wrapped("", "muted", "small");
        message.Name = "ContentMessage";
        message.Bind(TextBlock.TextProperty, new Binding(nameof(ContentEditorViewModel.Message)));
        var filter = new TextBox { Name = "ContentFilter", PlaceholderText = "Filter by name or id" };
        filter.Bind(TextBox.TextProperty, new Binding(nameof(ContentEditorViewModel.Filter)) { Mode = BindingMode.TwoWay });
        Ui.Label((_category, "Content type"), (_entities, "Entries"));
        foreach (var category in ContentEditorViewModel.Categories)
        {
            _category.Items.Add(new ComboBoxItem { Content = category.Name, Tag = category });
        }

        Ui.Label((filter, "Filter entries"));
        var draftBar = new DraftBar("Content", ViewModel.Draft);
        _category.SelectionChanged += (_, _) =>
        {
            if (_refreshing || _category.SelectedItem is not ComboBoxItem { Tag: ContentCategory category } || ReferenceEquals(category, ViewModel.SelectedCategory)) return;
            // The picker shows the type the fields are about until the view model moves on.
            ShowCategory();
            ViewModel.SelectCategory(category.Name);
        };
        _entities.SelectionChanged += (_, _) =>
        {
            if (_refreshing) return;
            var id = (_entities.SelectedItem as ListBoxItem)?.Tag as string;
            if (id == ViewModel.SelectedId) return;
            ShowSelection();
            ViewModel.SelectRow(id);
        };
        // Readouts follow the fields: any edit inside the form (typing, a picker, a checkbox)
        // bubbles up here. The refresh is posted, so it runs after a rebuild the edit starts.
        _form.AddHandler(TextBox.TextChangedEvent, (_, _) => QueueReadouts(), handledEventsToo: true);
        _form.AddHandler(SelectingItemsControl.SelectionChangedEvent, (_, _) => QueueReadouts(), handledEventsToo: true);
        _form.AddHandler(ToggleButton.IsCheckedChangedEvent, (_, _) => QueueReadouts(), handledEventsToo: true);
        var add = Ui.CommandButton("New", ViewModel.AddCommand, "accent");
        add.Name = "AddContentButton";
        _delete = Ui.CommandButton("Delete", ViewModel.DeleteCommand, "tool");
        _delete.Name = "DeleteContentButton";
        _duplicate = Ui.CommandButton("Duplicate", ViewModel.DuplicateCommand, "tool");
        _duplicate.Name = "DuplicateContentButton";
        _addToInventory = Ui.CommandButton("Add to inventory", ViewModel.AddToInventoryCommand, "tool");
        _addToInventory.Name = "AddToInventoryButton";
        ToolTip.SetTip(_addToInventory, "Give the player one of this item at the start of the game");
        var save = Ui.CommandButton("Save changes", ViewModel.SaveCommand, "accent");
        save.Name = "SaveContentButton";
        var revert = Ui.CommandButton("Revert fields", ViewModel.RevertCommand, "tool");
        revert.Name = "RevertContentButton";
        _customize = Ui.CommandButton("Customize", ViewModel.CustomizeCommand, "accent");
        _customize.Name = "CustomizeBuiltinButton";
        _customize.IsVisible = false;

        var listSide = new StackPanel { Spacing = 10, Margin = new Thickness(0, 0, 16, 0) };
        listSide.Children.Add(Ui.Text("CONTENT", "section"));
        listSide.Children.Add(_category);
        listSide.Children.Add(filter);
        listSide.Children.Add(_entities);
        listSide.Children.Add(Ui.HStack(8, add, _delete));
        listSide.Children.Add(Ui.HStack(8, _duplicate, _addToInventory));
        listSide.Children.Add(Ui.Wrapped("Select a type, then edit its fields. Every nested field also has an Edit as JSON box.", "muted", "small"));
        var editor = new StackPanel { Spacing = 12 };
        editor.Children.Add(Ui.Text("DETAILS", "section"));
        editor.Children.Add(draftBar);
        editor.Children.Add(message);
        editor.Children.Add(_summary);
        editor.Children.Add(_form);
        editor.Children.Add(_readouts);
        editor.Children.Add(Ui.HStack(8, _customize, save, revert));
        // The list is as wide as the creator dragged it last (#56); the splitter remembers it.
        var listWidth = Math.Clamp(_workspace.Settings.Load().ContentListWidth ?? DefaultListWidth, 160, 640);
        var layout = new Grid { Name = "ContentLayout", Margin = new Thickness(20) };
        layout.ColumnDefinitions.Add(new ColumnDefinition(listWidth, GridUnitType.Pixel) { MinWidth = 160, MaxWidth = 640 });
        layout.ColumnDefinitions.Add(new ColumnDefinition(GridLength.Auto));
        layout.ColumnDefinitions.Add(new ColumnDefinition(1, GridUnitType.Star));
        layout.Children.Add(listSide);
        listSide.Margin = new Thickness(0, 0, 8, 0);
        var splitter = new GridSplitter { Name = "ContentSplitter", Width = 6, ResizeDirection = GridResizeDirection.Columns, Background = Brushes.Transparent, Margin = new Thickness(0, 0, 10, 0) };
        AutomationProperties.SetName(splitter, "Resize the entry list");
        splitter.DragCompleted += (_, _) =>
        {
            var width = layout.ColumnDefinitions[0].ActualWidth;
            if (width > 0) _workspace.Settings.TryUpdate(settings => settings with { ContentListWidth = Math.Round(width) });
        };
        Grid.SetColumn(splitter, 1);
        layout.Children.Add(splitter);
        var formScroll = new ScrollViewer { Content = editor, HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled };
        Grid.SetColumn(formScroll, 2);
        layout.Children.Add(formScroll);
        Content = layout;
        ViewModel.PropertyChanged += OnViewModelChanged;
        foreach (var property in Shown) OnViewModelChanged(ViewModel, new PropertyChangedEventArgs(property));
    }

    public ContentEditorViewModel ViewModel { get; }

    /// <summary>Stops following the project (the editor that built this view was replaced).</summary>
    public void Retire() => ViewModel.Retire();

    public void Refresh() => ViewModel.Refresh();

    protected override void OnAttachedToVisualTree(VisualTreeAttachmentEventArgs e)
    {
        base.OnAttachedToVisualTree(e);
        _attached = true;
        DrawThumbnails();
    }

    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        _attached = false;
        _art.Dispose();
        base.OnDetachedFromVisualTree(e);
    }

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs e)
    {
        switch (e.PropertyName)
        {
            case nameof(ContentEditorViewModel.SelectedCategory): ShowCategory(); break;
            case nameof(ContentEditorViewModel.SelectedId): ShowSelection(); break;
            case nameof(ContentEditorViewModel.Rows): ShowRows(); break;
            case nameof(ContentEditorViewModel.FormEntity): ShowForm(); break;
            case nameof(ContentEditorViewModel.Summary): ShowSummary(); break;
            case nameof(ContentEditorViewModel.ProfitName): ShowProfit(); break;
            case nameof(ContentEditorViewModel.ProfitText) when _profit is not null: _profit.Text = ViewModel.ProfitText; break;
            case nameof(ContentEditorViewModel.ShowDuplicate): _duplicate.IsVisible = ViewModel.ShowDuplicate; break;
            case nameof(ContentEditorViewModel.ShowAddToInventory): _addToInventory.IsVisible = ViewModel.ShowAddToInventory; break;
            case nameof(ContentEditorViewModel.ShowCustomize): _customize.IsVisible = ViewModel.ShowCustomize; break;
            case nameof(ContentEditorViewModel.DeleteTip): ToolTip.SetTip(_delete, ViewModel.DeleteTip); break;
            case nameof(ContentEditorViewModel.CustomizeTip): ToolTip.SetTip(_customize, ViewModel.CustomizeTip); break;
        }
    }

    /// <summary>Shows the view model's type in the type picker without reacting to it.</summary>
    private void ShowCategory()
    {
        _refreshing = true;
        try
        {
            _category.SelectedItem = _category.Items.OfType<ComboBoxItem>().FirstOrDefault(item => ReferenceEquals(item.Tag, ViewModel.SelectedCategory));
        }
        finally
        {
            _refreshing = false;
        }
    }

    /// <summary>Shows the selected entry in the list without reacting to it.</summary>
    private void ShowSelection()
    {
        _refreshing = true;
        try
        {
            _entities.SelectedItem = _entities.Items.OfType<ListBoxItem>().FirstOrDefault(item => Equals(item.Tag, ViewModel.SelectedId));
        }
        finally
        {
            _refreshing = false;
        }
    }

    /// <summary>The entry list, with the selected entry selected.</summary>
    private void ShowRows()
    {
        _refreshing = true;
        try
        {
            _entities.SelectedItem = null;
            _entities.Items.Clear();
            _thumbnails.Clear();
            foreach (var row in ViewModel.Rows) _entities.Items.Add(ListRow(row));
            _entities.SelectedItem = _entities.Items.OfType<ListBoxItem>().FirstOrDefault(item => Equals(item.Tag, ViewModel.SelectedId));
        }
        finally
        {
            _refreshing = false;
        }

        DrawThumbnails();
    }

    /// <summary>
    /// One list entry: its art (or an empty slot, so names line up), "Name · id" and the muted
    /// line. <c>Tag</c> is the id; screen readers read the "Name · id" label and the line as help.
    /// </summary>
    private ListBoxItem ListRow(ContentListRow entry)
    {
        var text = Ui.VStack(1, Trimmed(Ui.Text(entry.Label)));
        text.VerticalAlignment = VerticalAlignment.Center;
        if (!string.IsNullOrEmpty(entry.Note)) text.Children.Add(Trimmed(Ui.Text(entry.Note, "muted", "small")));
        var row = new Grid { ColumnDefinitions = new ColumnDefinitions("Auto,*") };
        if (entry.HasArtSlot)
        {
            var holder = new Border
            {
                Name = $"ContentThumb_{entry.Id}",
                Width = ThumbnailSize,
                Height = ThumbnailSize,
                Margin = new Thickness(0, 0, 8, 0),
                VerticalAlignment = VerticalAlignment.Center,
            };
            if (entry.Art is { } visual) _thumbnails.Add((holder, visual));
            row.Children.Add(holder);
        }

        Grid.SetColumn(text, 1);
        row.Children.Add(text);
        var item = new ListBoxItem { Content = row, Tag = entry.Id };
        AutomationProperties.SetName(item, entry.Label);
        if (!string.IsNullOrEmpty(entry.Note)) AutomationProperties.SetHelpText(item, entry.Note);
        // The narrow list trims long lines; the tooltip has them whole.
        ToolTip.SetTip(item, string.IsNullOrEmpty(entry.Note) ? entry.Label : $"{entry.Label}\n{entry.Note}");
        return item;
    }

    private static TextBlock Trimmed(TextBlock block)
    {
        block.TextTrimming = TextTrimming.CharacterEllipsis;
        return block;
    }

    /// <summary>Draws the listed entries' art with the Rust renderer, once the view is on screen.</summary>
    private void DrawThumbnails()
    {
        if (!_attached || _workspace.Current is not { } project) return;
        foreach (var (holder, visual) in _thumbnails)
        {
            holder.Child ??= _art.Render(project, visual, 0, ThumbnailSize);
        }
    }

    /// <summary>Builds the form again for the entry the view model edits (none: the form is empty).</summary>
    private void ShowForm()
    {
        _form.Children.Clear();
        _contentForm?.ReleaseImages();
        _contentForm = null;
        var entity = ViewModel.FormEntity;
        _form.IsVisible = entity is not null;
        if (entity is null || _workspace.Current is not { } project) return;
        _contentForm = new ContentForm(project, entity, ViewModel.SelectedCategory.EntityType, _form, ViewModel.Report, _pickOnMap);
        ViewModel.AttachDraft(_contentForm);
    }

    private void ShowSummary()
    {
        _summary.Child = ViewModel.Summary is { } summary ? ContentReadoutCard.Card(summary.Name, summary.Lines) : null;
        _summary.IsVisible = ViewModel.Summary is not null;
    }

    private void ShowProfit()
    {
        _readouts.Children.Clear();
        _profit = null;
        if (ViewModel.ProfitName is { } name)
        {
            var (box, text) = ContentReadoutCard.Line(name);
            text.Text = ViewModel.ProfitText;
            _profit = text;
            _readouts.Children.Add(box);
        }

        _readouts.IsVisible = _profit is not null;
    }

    /// <summary>
    /// Recomputes the readouts from the fields after the current input settles. While a field
    /// holds something invalid the last readout stays.
    /// </summary>
    private void QueueReadouts()
    {
        if (_readoutsQueued || !ViewModel.FollowsFields) return;
        _readoutsQueued = true;
        Dispatcher.UIThread.Post(() =>
        {
            _readoutsQueued = false;
            ViewModel.PreviewReadouts();
        });
    }
}
