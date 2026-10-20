using System.ComponentModel;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Data;
using FarmEngine.Authoring;
using FarmingRpgMaker.App.Projects;
using FarmingRpgMaker.App.ViewModels.EditMode;

namespace FarmingRpgMaker.App.Game;

/// <summary>The Workshop tab: shows and collects the pattern form of its <see cref="WorkshopViewModel"/>.</summary>
public sealed class WorkshopView : UserControl, IRetirable
{
    private readonly ComboBox _patterns = new() { Name = "WorkshopPattern", MinWidth = 240 };
    private readonly ComboBox _npc = new() { Name = "WorkshopNpc", MinWidth = 180 };
    private bool _showingCharacters;

    /// <param name="workspace">The open project.</param>
    /// <param name="mapScene">The scene the Map tab shows: the patterns put their tile there (#47).</param>
    /// <param name="pickOnMap">Lets the creator click the pattern's tile on the map (scene id, prompt, what to do with the tile).</param>
    /// <param name="openEditor">Opens an editor tab by its web tab key.</param>
    public WorkshopView(ProjectWorkspace workspace, Func<string?> mapScene, Action<string, string, Action<int, int>> pickOnMap, Action<string> openEditor)
    {
        ViewModel = new WorkshopViewModel(workspace, mapScene, pickOnMap, openEditor);
        DataContext = ViewModel;
        Name = "WorkshopView";
        var name = Bound(new TextBox { Name = "WorkshopName", PlaceholderText = "Name" }, nameof(WorkshopViewModel.Name));
        var text = Bound(new TextBox
        {
            Name = "WorkshopText",
            PlaceholderText = "Story text or description",
            AcceptsReturn = true,
            MinHeight = 80,
            TextWrapping = Avalonia.Media.TextWrapping.Wrap,
        }, nameof(WorkshopViewModel.Text));
        var x = Bound(new TextBox { Name = "WorkshopX", Width = 65 }, nameof(WorkshopViewModel.X));
        var y = Bound(new TextBox { Name = "WorkshopY", Width = 65 }, nameof(WorkshopViewModel.Y));
        var day = Bound(new TextBox { Name = "WorkshopDay", Width = 65 }, nameof(WorkshopViewModel.Day));
        var friendship = Bound(new TextBox { Name = "WorkshopFriendship", Width = 75 }, nameof(WorkshopViewModel.Friendship));
        var consequences = new CheckBox { Name = "WorkshopConsequences", Content = "Remember the player's promise" };
        consequences.Bind(ToggleButton.IsCheckedProperty, new Binding(nameof(WorkshopViewModel.RememberPromise)) { Mode = BindingMode.TwoWay });
        var help = Shown(Ui.Wrapped("", "muted", "small"), nameof(WorkshopViewModel.Help));
        var message = Shown(Ui.Wrapped("", "muted", "small"), nameof(WorkshopViewModel.Message));
        message.Name = "WorkshopMessage";
        var scene = Shown(Ui.Wrapped("", "muted", "small"), nameof(WorkshopViewModel.SceneText));
        scene.Name = "WorkshopScene";
        Ui.Label((_patterns, "Pattern"), (name, "Name"), (text, "Story text or description"), (x, "Scene tile X"), (y, "Scene tile Y"),
            (day, "Delivery day"), (friendship, "Friendship"), (_npc, "Character"));
        foreach (var pattern in WorkshopViewModel.AllPatterns) _patterns.Items.Add(new ComboBoxItem { Content = pattern.Name, Tag = pattern });
        _patterns.SelectedItem = _patterns.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, ViewModel.SelectedPattern));
        _patterns.SelectionChanged += (_, _) => ViewModel.SelectedPattern = (_patterns.SelectedItem as ComboBoxItem)?.Tag as PatternInfo;
        _npc.SelectionChanged += (_, _) =>
        {
            if (!_showingCharacters && _npc.SelectedItem is ComboBoxItem { Tag: string id }) ViewModel.CharacterId = id;
        };
        ShowCharacters();

        var form = new StackPanel { Spacing = 12, Margin = new Thickness(20), MaxWidth = 760 };
        form.Children.Add(Ui.Text("CREATOR WORKSHOP", "section"));
        form.Children.Add(Ui.Wrapped("Start with a working interaction. Each pattern creates editable content and undoes as one step.", "muted", "small"));
        form.Children.Add(_patterns);
        form.Children.Add(help);
        form.Children.Add(Ui.Text("Name", "muted", "small"));
        form.Children.Add(name);
        form.Children.Add(Ui.Text("Story text or description", "muted", "small"));
        form.Children.Add(text);
        var pick = Ui.CommandButton("Pick on map", ViewModel.PickTileCommand, "tool", "small");
        pick.Name = "WorkshopPickOnMap";
        Avalonia.Automation.AutomationProperties.SetName(pick, "Pick the scene tile on the map");
        form.Children.Add(Ui.HStack(8, Ui.Text("Scene tile", "muted", "small"), x, Ui.Text(","), y, pick));
        form.Children.Add(scene);
        form.Children.Add(Ui.HStack(8, Ui.Text("Delivery day", "muted", "small"), day,
            Ui.Text("Friendship", "muted", "small"), friendship));
        form.Children.Add(Ui.HStack(8, Ui.Text("Character", "muted", "small"), _npc));
        form.Children.Add(consequences);
        var create = Ui.CommandButton("Create pattern", ViewModel.CreateCommand, "accent");
        create.Name = "CreatePatternButton";
        var open = Ui.CommandButton("Open its editor", ViewModel.OpenEditorCommand, "tool");
        form.Children.Add(Ui.HStack(8, create, open));
        form.Children.Add(message);

        form.Children.Add(Ui.Text("BUILD YOUR GAME", "section"));
        var links = new WrapPanel { Name = "WorkshopLinks" };
        foreach (var (tab, label) in WorkshopViewModel.QuickLinks)
        {
            var link = Ui.Button(label, () => ViewModel.OpenLink(tab), "tool");
            link.Name = $"WorkshopLink_{tab}";
            link.Margin = new Thickness(0, 0, 6, 6);
            links.Children.Add(link);
        }
        form.Children.Add(links);
        Content = new ScrollViewer { Content = form };
        ViewModel.PropertyChanged += OnViewModelChanged;
    }

    public WorkshopViewModel ViewModel { get; }

    /// <summary>Stops following the project (the editor that built this view was replaced).</summary>
    public void Retire() => ViewModel.Retire();

    public void Refresh() => ViewModel.Refresh();

    private static TextBox Bound(TextBox box, string property)
    {
        box.Bind(TextBox.TextProperty, new Binding(property) { Mode = BindingMode.TwoWay });
        return box;
    }

    private static TextBlock Shown(TextBlock block, string property)
    {
        block.Bind(TextBlock.TextProperty, new Binding(property));
        return block;
    }

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(WorkshopViewModel.Characters)) ShowCharacters();
        if (e.PropertyName == nameof(WorkshopViewModel.CharacterId)) SelectCharacter();
    }

    /// <summary>The character picker: the view model's characters, with its choice selected.</summary>
    private void ShowCharacters()
    {
        _showingCharacters = true;
        try
        {
            _npc.SelectedItem = null;
            _npc.Items.Clear();
            foreach (var character in ViewModel.Characters) _npc.Items.Add(new ComboBoxItem { Content = character.Name, Tag = character.Id });
        }
        finally { _showingCharacters = false; }
        SelectCharacter();
    }

    private void SelectCharacter()
    {
        _showingCharacters = true;
        try
        {
            _npc.SelectedItem = _npc.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, ViewModel.CharacterId)) ?? _npc.Items[0];
        }
        finally { _showingCharacters = false; }
    }
}
