using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Platform.Storage;
using FarmEngine.Authoring;
using FarmEngine.Schemas;
using Tractr.App.Projects;

namespace Tractr.App.Game;

/// <summary>
/// The Interface tab's skin section (#236): the game's fonts, colours, panel art, icons, layout
/// and title screen. <see cref="SkinForm"/> builds the skin; Save skin applies it as one undo
/// step and Reset to built-in look removes it.
/// </summary>
public sealed class SkinEditorPanel : StackPanel
{
    private static readonly FilePickerFileType FontFiles = new("Fonts") { Patterns = ["*.ttf", "*.otf"] };
    private static readonly string[] Roles = ["text", "muted", "accent", "onAccent", "accentText", "warning", "panel", "panelBorder", "hud", "button", "buttonHover", "primary", "secondary"];
    private readonly ProjectWorkspace _workspace;
    private readonly Dictionary<string, ComboBox> _fonts = [];
    private readonly Dictionary<string, TextBox> _colors = [];
    private readonly Dictionary<string, ComboBox> _panels = [];
    private readonly Dictionary<string, ComboBox> _icons = [];
    private readonly TextBox _edge = new() { Name = "SkinPanelEdge", Text = "4", Width = 50 };
    private readonly TextBox _scale = new() { Name = "SkinPanelScale", Text = "1", Width = 50 };
    private readonly ComboBox _hud = Choice("SkinHudPosition", UiHudPositions.All);
    private readonly CheckBox _toolbar = new() { Name = "SkinToolbar", Content = "Toolbar buttons", IsChecked = true };
    private readonly ComboBox _clock = Choice("SkinClockStyle", UiClockStyles.All);
    private readonly ComboBox _dialogue = Choice("SkinDialoguePosition", UiDialoguePositions.All);
    private readonly TextBox _portrait = new() { Name = "SkinPortraitSize", Text = "48", Width = 50 };
    private readonly ComboBox _titleBackground = new() { Name = "SkinTitleBackground", MinWidth = 150 };
    private readonly ComboBox _titleLogo = new() { Name = "SkinTitleLogo", MinWidth = 150 };
    private readonly ComboBox _titleScene = new() { Name = "SkinTitleScene", MinWidth = 150 };
    private readonly ComboBox _menu = Choice("SkinMenuPosition", UiMenuPositions.All);
    private readonly TextBlock _message = Ui.Wrapped("", "muted", "small");

    public SkinEditorPanel(ProjectWorkspace workspace)
    {
        _workspace = workspace;
        Name = "SkinEditor";
        Spacing = 8;
        _message.Name = "SkinMessage";
        AutomationProperties.SetName(_edge, "Panel edge (pixels)");
        AutomationProperties.SetName(_scale, "Panel scale");
        AutomationProperties.SetName(_portrait, "Portrait size");
        AutomationProperties.SetName(_titleBackground, "Title background art");
        AutomationProperties.SetName(_titleLogo, "Title logo");
        AutomationProperties.SetName(_titleScene, "Title scene");
        Children.Add(Ui.Text("INTERFACE SKIN", "section"));
        Children.Add(Ui.Wrapped("Your game's own look: fonts, colours, panel art (drawn in nine parts so corners keep their size), icons and layout. Empty fields keep the built-in look; the player's text size and readable font still apply.", "muted", "small"));

        Children.Add(Ui.Text("Fonts", "muted", "small"));
        foreach (var role in new[] { "heading", "body", "number" })
        {
            var box = new ComboBox { Name = "SkinFont-" + role, MinWidth = 150 };
            AutomationProperties.SetName(box, $"{role} font");
            _fonts[role] = box;
            Children.Add(Ui.HStack(6, Ui.Text(role, "small"), box));
        }
        var importFont = Ui.AsyncButton("Import font…", PickFontAsync, error => _message.Text = $"Could not import: {error.Message}", "tool");
        importFont.Name = "ImportSkinFontButton";
        Children.Add(importFont);

        Children.Add(Ui.Text("Colours (#rrggbb or #rrggbbaa)", "muted", "small"));
        var colors = new WrapPanel();
        foreach (var role in Roles)
        {
            var box = new TextBox { Name = "SkinColor-" + role, Width = 90, PlaceholderText = role };
            AutomationProperties.SetName(box, $"{role} colour");
            _colors[role] = box;
            colors.Children.Add(Ui.HStack(4, Ui.Text(role, "small"), box));
        }
        Children.Add(colors);

        Children.Add(Ui.Text("Panel art", "muted", "small"));
        Children.Add(Ui.HStack(6, Ui.Text("Edge (source px)", "small"), _edge, Ui.Text("Scale", "small"), _scale));
        foreach (var part in UiSkinParts.All)
        {
            var box = new ComboBox { Name = "SkinPanel-" + part, MinWidth = 150 };
            AutomationProperties.SetName(box, $"{part} art");
            _panels[part] = box;
            Children.Add(Ui.HStack(6, Ui.Text(part, "small"), box));
        }

        Children.Add(Ui.Text("Icons", "muted", "small"));
        var icons = new WrapPanel();
        foreach (var name in UiSkinIcons.All)
        {
            var box = new ComboBox { Name = "SkinIcon-" + name, MinWidth = 110 };
            AutomationProperties.SetName(box, $"{name} icon");
            _icons[name] = box;
            icons.Children.Add(Ui.HStack(4, Ui.Text(name, "small"), box));
        }
        Children.Add(icons);

        Children.Add(Ui.Text("Layout", "muted", "small"));
        Children.Add(Ui.HStack(6, Ui.Text("HUD", "small"), _hud, _toolbar));
        Children.Add(Ui.HStack(6, Ui.Text("Clock", "small"), _clock, Ui.Text("Dialogue box", "small"), _dialogue));
        Children.Add(Ui.HStack(6, Ui.Text("Portrait size (32–160)", "small"), _portrait));

        Children.Add(Ui.Text("Title screen", "muted", "small"));
        Children.Add(Ui.HStack(6, Ui.Text("Background", "small"), _titleBackground, Ui.Text("Logo", "small"), _titleLogo));
        Children.Add(Ui.HStack(6, Ui.Text("Scene", "small"), _titleScene, Ui.Text("Menu", "small"), _menu));

        var save = Ui.Button("Save skin", Save, "accent");
        save.Name = "SaveSkinButton";
        var reset = Ui.Button("Reset to built-in look", Reset, "tool");
        reset.Name = "ResetSkinButton";
        Children.Add(Ui.HStack(8, save, reset));
        Children.Add(_message);
        Load();
    }

    private static ComboBox Choice(string name, IEnumerable<string> values)
    {
        var box = new ComboBox { Name = name, MinWidth = 90 };
        foreach (var value in values) box.Items.Add(new ComboBoxItem { Content = value, Tag = value });
        box.SelectedIndex = 0;
        AutomationProperties.SetName(box, name);
        return box;
    }

    private static void Fill(ComboBox box, IEnumerable<(string Id, string Name)> options, string selected)
    {
        box.Items.Clear();
        box.Items.Add(new ComboBoxItem { Content = "(built-in)", Tag = "" });
        foreach (var (id, name) in options) box.Items.Add(new ComboBoxItem { Content = name, Tag = id });
        box.SelectedItem = box.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, selected)) ?? box.Items[0];
    }

    private static void Pick(ComboBox box, string? value) =>
        box.SelectedItem = box.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, value ?? "")) ?? box.Items[0];

    private static string TagOf(ComboBox box) => (box.SelectedItem as ComboBoxItem)?.Tag as string ?? "";

    private static double Number(TextBox box, double fallback) =>
        double.TryParse(box.Text, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var value) ? value : fallback;

    /// <summary>Shows the project's skin in the controls.</summary>
    public void Load()
    {
        if (_workspace.Current is not { } project) return;
        var skin = SkinForm.Current(project);
        var fonts = SkinForm.Fonts(project).Select(asset => (asset.Id, asset.Name)).ToList();
        var art = project.CustomAssets
            .Where(asset => asset.Type != CustomAssetTypes.Font && asset.Type != CustomAssetTypes.Sound && asset.Type != CustomAssetTypes.Music)
            .Select(asset => (asset.Id, asset.Name)).ToList();
        Fill(_fonts["heading"], fonts, skin.HeadingFont?.Value ?? "");
        Fill(_fonts["body"], fonts, skin.BodyFont?.Value ?? "");
        Fill(_fonts["number"], fonts, skin.NumberFont?.Value ?? "");
        foreach (var (role, box) in _colors) box.Text = SkinForm.Color(skin, role);
        foreach (var (part, box) in _panels) Fill(box, art, SkinForm.PanelAsset(skin, part));
        foreach (var (name, box) in _icons) Fill(box, art, SkinForm.IconAsset(skin, name));
        var firstSlice = skin.Panels?.Value?.FirstOrDefault();
        if (firstSlice is { } slice)
        {
            _edge.Text = slice.Item2.Left.ToString(System.Globalization.CultureInfo.InvariantCulture);
            _scale.Text = (slice.Item2.Scale?.Value ?? 1.0).ToString(System.Globalization.CultureInfo.InvariantCulture);
        }
        var layout = skin.Layout?.Value;
        Pick(_hud, layout?.HudPosition?.Value ?? UiHudPositions.Top);
        _toolbar.IsChecked = layout?.Toolbar?.Value ?? true;
        Pick(_clock, layout?.ClockStyle?.Value ?? UiClockStyles.TwelveHour);
        Pick(_dialogue, layout?.DialoguePosition?.Value ?? UiDialoguePositions.Bottom);
        _portrait.Text = (layout?.PortraitSize?.Value ?? 48.0).ToString(System.Globalization.CultureInfo.InvariantCulture);
        var title = skin.Title?.Value;
        Fill(_titleBackground, art, title?.Background?.Value?.AssetId ?? "");
        Fill(_titleLogo, art, title?.Logo?.Value?.AssetId ?? "");
        Fill(_titleScene, project.Scenes.Select(scene => (scene.Id, scene.Name)), title?.Scene?.Value ?? "");
        Pick(_menu, title?.MenuPosition?.Value ?? UiMenuPositions.Center);
    }

    /// <summary>Builds the skin from the controls and applies it (one undo step).</summary>
    public void Save()
    {
        if (_workspace.Current is null) return;
        var skin = UiSkin.Default;
        foreach (var (role, box) in _fonts) skin = SkinForm.WithFont(skin, role, TagOf(box));
        foreach (var (role, box) in _colors) skin = SkinForm.WithColor(skin, role, box.Text);
        var edge = Math.Max(0, Number(_edge, 4));
        var scale = Math.Clamp(Number(_scale, 1), 1, 8);
        foreach (var (part, box) in _panels) skin = SkinForm.WithPanel(skin, part, TagOf(box), edge, scale);
        foreach (var (name, box) in _icons) skin = SkinForm.WithIcon(skin, name, TagOf(box));
        skin = SkinForm.WithLayout(skin, TagOf(_hud), _toolbar.IsChecked != false, TagOf(_clock), TagOf(_dialogue), Number(_portrait, 48));
        skin = SkinForm.WithTitle(skin, TagOf(_titleBackground), TagOf(_titleLogo), TagOf(_titleScene), TagOf(_menu));
        var changed = SkinForm.IsEmpty(skin) ? _workspace.Apply(Edits.ResetUiSkin()) : _workspace.Apply(Edits.SetUiSkin(skin));
        _message.Text = changed ? "Skin saved. Play the game to see it; Problems lists any part that keeps the built-in look." : "No changes to save.";
    }

    /// <summary>Removes the skin: the game draws the built-in look again.</summary>
    public void Reset()
    {
        _message.Text = _workspace.Apply(Edits.ResetUiSkin()) ? "Back to the built-in look." : "The game already has the built-in look.";
        Load();
    }

    private async Task PickFontAsync()
    {
        if (TopLevel.GetTopLevel(this)?.StorageProvider is not { CanOpen: true } storage || _workspace.Current is not { } project) return;
        var picked = await storage.OpenFilePickerAsync(new FilePickerOpenOptions { Title = "Import a font", FileTypeFilter = [FontFiles] });
        if (picked.Count == 0) return;
        var bytes = await PickedFiles.ReadBytesAsync(picked[0], FontFileModule.MaxBytes, "fonts");
        ImportFont(project, picked[0].Name, bytes);
    }

    /// <summary>Adds a font asset from a font file's bytes.</summary>
    public void ImportFont(GameProject project, string fileName, byte[] bytes)
    {
        var asset = SkinForm.ReadFont(project, fileName, bytes);
        _workspace.Apply(Edits.UpsertAsset(asset));
        _message.Text = $"Imported {fileName}: choose it as a heading, body or number font.";
        Load();
    }
}
