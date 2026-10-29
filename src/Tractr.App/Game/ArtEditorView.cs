using System.Globalization;
using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Data;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Platform.Storage;
using Avalonia.Threading;
using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using Tractr.App.Localization;
using Tractr.App.Projects;
using Tractr.App.ViewModels.EditMode;

namespace Tractr.App.Game;

/// <summary>Import, animate and bind creator art through F# project edits.</summary>
public sealed partial class ArtEditorView : UserControl, IRetirable
{
    private static readonly FilePickerFileType ArtFiles = new("Artwork") { Patterns = ArtImport.FilePatterns };
    private static readonly FilePickerFileType SoundFiles = new(EditorStrings.Get("art.audioFiles")) { Patterns = AudioImport.FilePatterns };
    private readonly ProjectWorkspace _workspace;
    private readonly Action<string, VisualRef> _useMapBrush;
    private readonly DispatcherTimer _previewTimer = new() { Interval = TimeSpan.FromMilliseconds(50) };
    private readonly ListBox _assets = new() { Name = "ArtAssets", MinHeight = 130 };
    private readonly ListBox _sounds = new() { Name = "SoundAssets", MinHeight = 60 };
    private readonly TextBox _soundName = new() { Name = "SoundAssetName", Width = 150 };
    private Button _playSound = null!;
    /// <summary>Watches the preview while it plays, to turn Stop back into Play when it ends.</summary>
    private readonly DispatcherTimer _soundTimer = new() { Interval = TimeSpan.FromMilliseconds(200) };
    private ISoundPreview _soundPreview = RustSoundPreview.Instance;
    /// <summary>The sound this view is previewing, or null.</summary>
    private string? _playingSoundId;
    private bool _startingSound;
    private readonly Border _preview = new() { Name = "ArtPreview", Width = 140, Height = 140 };
    private readonly VisualPreview _art;
    private readonly TextBlock _message = Ui.Wrapped("Import an image to start.", "muted", "small");
    private readonly TextBox _assetName = new() { Name = "ArtAssetName" };
    private readonly ComboBox _clips = new() { Name = "ArtClips" };
    private readonly TextBox _clipName = new() { Name = "ArtClipName", Text = "idle" };
    private readonly TextBox _frameWidth = new() { Name = "ArtFrameWidth", Text = "16", Width = 58 };
    private readonly TextBox _frameHeight = new() { Name = "ArtFrameHeight", Text = "16", Width = 58 };
    private readonly TextBox _frameX = new() { Name = "ArtFrameX", Text = "0", Width = 58 };
    private readonly TextBox _frameY = new() { Name = "ArtFrameY", Text = "0", Width = 58 };
    private readonly TextBox _frameTicks = new() { Name = "ArtFrameTicks", Text = "6", Width = 58 };
    private readonly CheckBox _loop = new() { Name = "ArtLoop", Content = "Loop", IsChecked = true };
    private readonly StackPanel _frames = new() { Name = "ArtFrames", Spacing = 4 };
    private readonly SpriteSheetView _sheet = new();
    private readonly ComboBox _frameSource = new() { Name = "ArtFrameSource", MinWidth = 160, PlaceholderText = "Choose image" };
    private Button _addImageFrame = null!;
    private Button _play = null!;
    private readonly ComboBox _target = new() { Name = "ArtTarget" };
    private readonly ComboBox _bindingAsset = new() { Name = "ArtBindingAsset" };
    private readonly ComboBox _bindingClip = new() { Name = "ArtBindingClip" };
    private readonly ComboBox _tileType = new() { Name = "ArtTileType" };
    private readonly CheckBox _singleCell = new() { Name = "ArtSingleCell", Content = "Use one cell" };
    private readonly TextBox _cellSize = new() { Name = "ArtCellSize", Text = "16", Width = 55 };
    private readonly TextBox _cellColumn = new() { Name = "ArtCellColumn", Text = "0", Width = 55 };
    private readonly TextBox _cellRow = new() { Name = "ArtCellRow", Text = "0", Width = 55 };
    private readonly CheckBox _pixelArt = new() { Name = "ArtPixelArt", Content = "Crisp pixel art" };
    private readonly TextBox _svgSize = new() { Name = "ArtSvgSize", Width = 70, PlaceholderText = "own" };
    private readonly Border _unusedConfirm = new() { Name = "UnusedArtConfirm", IsVisible = false };
    /// <summary>Asset list thumbnails by asset id, with the data URL they were decoded from.</summary>
    private readonly Dictionary<string, (string DataUrl, Bitmap? Bitmap)> _thumbnails = [];
    /// <summary>The asset (and data URL) the sprite sheet shows.</summary>
    private (string AssetId, string DataUrl)? _sheetKey;
    private readonly ArtEditorViewModel _model;
    private bool _refreshing;
    private bool _playing = true;
    private double _tick;

    public ArtEditorView(ProjectWorkspace workspace, Action<string, VisualRef> useMapBrush, EditorPreview? preview = null)
    {
        _workspace = workspace;
        _model = new ArtEditorViewModel(workspace);
        DataContext = _model;
        _message.Bind(TextBlock.TextProperty, new Binding(nameof(ArtEditorViewModel.Message)));
        _art = new VisualPreview(preview);
        _useMapBrush = useMapBrush;
        Ui.Label((_assets, "Artwork"), (_svgSize, "SVG size (longest side, px)"), (_assetName, "Asset name"), (_clips, "Animation clip"),
            (_clipName, "Clip name"), (_frameWidth, "Frame width"), (_frameHeight, "Frame height"), (_frameX, "Frame X"), (_frameY, "Frame Y"),
            (_frameTicks, "Ticks per frame"), (_target, "Assign to"), (_bindingAsset, "Artwork to assign"), (_bindingClip, "Clip to assign"),
            (_cellSize, "Cell size"), (_cellColumn, "Cell column"), (_cellRow, "Cell row"), (_tileType, "Map tile behavior"), (_sounds, EditorStrings.Get("art.soundsName")), (_soundName, "Sound name"));
        Name = "ArtEditorView";
        _message.Name = "ArtMessage";
        _previewTimer.Tick += (_, _) => AdvancePreview();
        _sheet.CellClicked += AppendFrameAt;
        _frameWidth.TextChanged += (_, _) => RefreshSheetGrid();
        _frameHeight.TextChanged += (_, _) => RefreshSheetGrid();
        _assets.SelectionChanged += (_, _) =>
        {
            if (_refreshing) return;
            _model.SelectedAssetId = (_assets.SelectedItem as ListBoxItem)?.Tag as string;
            RefreshSelected();
        };
        _clips.SelectionChanged += (_, _) =>
        {
            if (_refreshing) return;
            var clip = SelectedClip();
            if (clip is null) return;
            _clipName.Text = clip.Name;
            _loop.IsChecked = clip.Loop;
            if (clip.Frames.FirstOrDefault() is { } frame)
            {
                _frameWidth.Text = Number(frame.Width);
                _frameHeight.Text = Number(frame.Height);
                _frameTicks.Text = Number(frame.Ticks);
            }
            RefreshFrames();
            DrawPreview();
            UpdatePreviewTimer();
        };
        _bindingAsset.SelectionChanged += (_, _) => RefreshBindingClips();
        _sounds.SelectionChanged += (_, _) =>
        {
            if (_refreshing) return;
            _model.SelectedSoundId = SelectedSoundId;
            ShowSelectedSound();
        };
        _soundTimer.Tick += (_, _) => WatchSound();

        BuildLayout();
        _workspace.ProjectChanged += OnProjectChanged;
        RefreshCellControls();
        Refresh();
    }

    /// <summary>Stops following the project (the editor that built this view was replaced).</summary>
    public void Retire()
    {
        _workspace.ProjectChanged -= OnProjectChanged;
        _model.Retire();
        StopSound();
    }

    private void OnProjectChanged(object? sender, ProjectChangedEventArgs e)
    {
        if (IsEffectivelyVisible) Refresh();
    }

    protected override void OnAttachedToVisualTree(VisualTreeAttachmentEventArgs e)
    {
        base.OnAttachedToVisualTree(e);
        UpdatePreviewTimer();
    }

    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        _previewTimer.Stop();
        _art.Dispose();
        StopSound();
        base.OnDetachedFromVisualTree(e);
    }

    private static string Number(double value) => DisplayFormat.Number(value);
    /// <summary>"32×16", or "?×?" for legacy art imported without its size.</summary>
    private static string SizeText(CustomAsset asset) => ArtEditorViewModel.SizeText(asset);
    private static int Positive(TextBox box) => int.TryParse(box.Text, NumberStyles.Integer, CultureInfo.InvariantCulture, out var value) && value > 0 ? value : throw new FormatException($"{FieldName(box)} must be a positive whole number.");
    private static int Nonnegative(TextBox box) => int.TryParse(box.Text, NumberStyles.Integer, CultureInfo.InvariantCulture, out var value) && value >= 0 ? value : throw new FormatException($"{FieldName(box)} must be zero or more.");
    /// <summary>A field as the creator knows it (its label, which screen readers announce too), not the control's name.</summary>
    private static string FieldName(TextBox box) => AutomationProperties.GetName(box) is { Length: > 0 } label ? label : "This field";
    private CustomAsset? SelectedAsset() => _model.SelectedAsset;
    private AnimationClip? SelectedClip() => SelectedAsset()?.Animations.OrEmpty().FirstOrDefault(clip => clip.Name == (_clips.SelectedItem as ComboBoxItem)?.Tag as string);

    /// <summary>The asset being edited, or null.</summary>
    public string? SelectedAssetId => _model.SelectedAssetId;

    /// <summary>Whether the animation preview is playing ("Pause preview" holds its current frame).</summary>
    public bool PreviewPlaying => _playing;

    /// <summary>The animation tick the preview shows.</summary>
    public double PreviewTick => _tick;

    /// <summary>Whether the preview timer runs (only while an animation with several frames plays on screen).</summary>
    internal bool PreviewTimerRunning => _previewTimer.IsEnabled;

    /// <summary>One step of the preview timer: the next tick is drawn unless the preview is paused or hidden.</summary>
    internal void AdvancePreview()
    {
        if (!_playing || !IsEffectivelyVisible) return;
        _tick++;
        DrawPreview();
    }

    /// <summary>
    /// Runs the preview timer only while it changes something: playing, on screen, and a clip
    /// with more than one frame selected. A still image is drawn once, not 20 times a second.
    /// </summary>
    private void UpdatePreviewTimer()
    {
        var animated = SelectedClip() is { } clip && clip.Frames.Length > 1;
        if (_playing && animated && TopLevel.GetTopLevel(this) is not null)
        {
            if (!_previewTimer.IsEnabled) _previewTimer.Start();
        }
        else
        {
            _previewTimer.Stop();
        }
    }

    private void TogglePreview()
    {
        _playing = !_playing;
        _play.Content = _playing ? "Pause preview" : "Play preview";
        UpdatePreviewTimer();
    }

    /// <summary>Opens an asset by id (Problems "Go to"): art in the art list, a sound in the sound list.</summary>
    public void SelectAsset(string assetId)
    {
        if (_workspace.Current?.CustomAssets.FirstOrDefault(asset => asset.Id == assetId) is { } asset && AudioImport.IsAudio(asset))
        {
            Refresh();
            _sounds.SelectedItem = _sounds.Items.OfType<ListBoxItem>().FirstOrDefault(item => Equals(item.Tag, assetId));
            return;
        }

        _model.SelectedAssetId = assetId;
        Refresh();
    }

    /// <summary>The selected sound or music asset's id, or null.</summary>
    public string? SelectedSoundId => (_sounds.SelectedItem as ListBoxItem)?.Tag as string;

    /// <summary>
    /// Imports OGG and WAV files as sound or music assets (<paramref name="type"/>) as ONE undo
    /// step; files that fail are reported by name and the rest are still imported.
    /// </summary>
    public void ImportSounds(IReadOnlyList<(string FileName, byte[] Bytes)> files, string type) => ImportSounds(files, type, []);

    private void ImportSounds(IReadOnlyList<(string FileName, byte[] Bytes)> files, string type, IReadOnlyList<string> readErrors)
    {
        _model.Import(files, type, errors: readErrors);
        RefreshImportedSound();
    }

    private void RefreshImportedSound()
    {
        Refresh();
        _sounds.SelectedItem = _sounds.Items.OfType<ListBoxItem>().FirstOrDefault(item => Equals(item.Tag, _model.SelectedSoundId));
    }

    private async Task PickSoundsAsync(string type)
    {
        if (TopLevel.GetTopLevel(this)?.StorageProvider is not { CanOpen: true } storage) return;
        var title = EditorStrings.Get(type == CustomAssetTypes.Music ? "art.importMusic" : "art.importSounds");
        var picked = await storage.OpenFilePickerAsync(new FilePickerOpenOptions { Title = title, AllowMultiple = true, FileTypeFilter = [SoundFiles, FilePickerFileTypes.All] });
        if (picked.Count == 0) return;
        var files = new List<(string, byte[])>();
        var errors = new List<string>();
        foreach (var file in picked)
        {
            try
            {
                files.Add((file.Name, await PickedFiles.ReadBytesAsync(file, PickedFiles.MaxAudioBytes, "sounds")));
            }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException)
            {
                errors.Add($"{file.Name}: {error.Message}");
            }
        }

        await _model.ImportAsync(files, type, errors: errors);
        RefreshImportedSound();
    }

    /// <summary>The preview the play button uses (tests swap in a fake).</summary>
    internal ISoundPreview SoundPreview
    {
        get => _soundPreview;
        set
        {
            StopSound();
            _soundPreview = value ?? throw new ArgumentNullException(nameof(value));
            UpdateSoundControls();
        }
    }

    /// <summary>The sound or music the play button is playing, or null.</summary>
    public string? PlayingSoundId => _playingSoundId;

    /// <summary>The play button: plays the selected sound or music once, or stops it.</summary>
    internal async Task ToggleSoundAsync()
    {
        if (_playingSoundId is not null)
        {
            StopSound();
            return;
        }

        if (_startingSound || SelectedSoundId is not { } id || _workspace.Current?.CustomAssets.FirstOrDefault(asset => asset.Id == id) is not { } asset) return;
        if (AudioFiles.dataUrlBytes(asset.DataUrl).OrNull() is not { } file)
        {
            _model.Message = $"Could not play {asset.Name}: its file is not a base64 data: URL.";
            return;
        }

        _startingSound = true;
        UpdateSoundControls();
        try
        {
            await _soundPreview.PlayAsync(file).ConfigureAwait(true);
            _playingSoundId = id;
            _model.Message = $"Playing {asset.Name}.";
            _soundTimer.Start();
        }
        catch (FarmEngine.Interop.FarmFfiException error)
        {
            _model.Message = $"Could not play {asset.Name}: {error.Message}";
        }
        finally
        {
            _startingSound = false;
            UpdateSoundControls();
        }
    }

    /// <summary>Stops the preview this view started (nothing happens otherwise).</summary>
    private void StopSound()
    {
        _soundTimer.Stop();
        if (_playingSoundId is null) return;
        _playingSoundId = null;
        _soundPreview.Stop();
        UpdateSoundControls();
    }

    /// <summary>One look at the preview while it plays: Stop turns back into Play when it ended.</summary>
    internal void WatchSound()
    {
        if (_playingSoundId is not null && !_soundPreview.IsPlaying)
        {
            _playingSoundId = null;
            _soundTimer.Stop();
            UpdateSoundControls();
        }
    }

    /// <summary>
    /// Play turns into Stop while a preview plays. Without sound output (an editor built without
    /// it, or no Rust library) the button is off and its tooltip says why.
    /// </summary>
    private void UpdateSoundControls()
    {
        var reason = _soundPreview.UnavailableReason;
        var playing = _playingSoundId is not null;
        _playSound.Content = Ui.IconLabel(playing ? "IconClose" : "IconPlay", playing ? "Stop" : "Play");
        AutomationProperties.SetName(_playSound, playing ? "Stop sound" : "Play sound");
        _playSound.IsEnabled = reason is null && !_startingSound && (playing || SelectedSoundId is not null);
        ToolTip.SetTip(_playSound, reason is not null ? $"Sound preview is unavailable: {reason}"
            : playing ? "Stop playing" : "Play the selected sound or music once");
    }

    /// <summary>The selected sound's name in the rename field, and the play button's state.</summary>
    private void ShowSelectedSound()
    {
        _soundName.Text = _workspace.Current?.CustomAssets.FirstOrDefault(asset => asset.Id == SelectedSoundId)?.Name ?? "";
        UpdateSoundControls();
    }

    /// <summary>Renames the selected sound or music (its id, which everything that plays it names, stays).</summary>
    private void RenameSound() => _model.Rename(SelectedSoundId, _soundName.Text);

    private void RemoveSound() => _model.Remove(SelectedSoundId, art: false);

    public void ImportBytes(string fileName, byte[] bytes) => ImportFiles([(fileName, bytes)]);

    /// <summary>
    /// Imports several images as ONE undo step (web AssetManager multi-file upload). Files that
    /// fail are reported by name; the rest are still imported.
    /// </summary>
    public void ImportFiles(IReadOnlyList<(string FileName, byte[] Bytes)> files) => ImportFiles(files, []);

    private void ImportFiles(IReadOnlyList<(string FileName, byte[] Bytes)> files, IReadOnlyList<string> readErrors)
    {
        _model.Import(files, CustomAssetTypes.Art, _svgSize.Text, readErrors);
        Refresh();
    }

    private async Task PickImagesAsync()
    {
        if (TopLevel.GetTopLevel(this)?.StorageProvider is not { CanOpen: true } storage) return;
        var picked = await storage.OpenFilePickerAsync(new FilePickerOpenOptions { Title = "Import artwork", AllowMultiple = true, FileTypeFilter = [ArtFiles, FilePickerFileTypes.All] });
        if (picked.Count == 0) return;
        var files = new List<(string, byte[])>();
        var errors = new List<string>();
        foreach (var file in picked)
        {
            try
            {
                files.Add((file.Name, await PickedFiles.ReadBytesAsync(file, ArtImport.MaxBytes, "images")));
            }
            catch (Exception error) when (error is IOException or ArgumentException or UnauthorizedAccessException)
            {
                errors.Add($"{file.Name}: {error.Message}");
            }
        }

        await _model.ImportAsync(files, CustomAssetTypes.Art, _svgSize.Text, errors);
        Refresh();
    }

    /// <summary>Shows what "Remove unused art" would delete, with Remove and Cancel.</summary>
    public void ConfirmRemoveUnused()
    {
        if (_workspace.Current is not { } project) return;
        var unused = _model.Unused;
        if (unused.Count == 0)
        {
            _unusedConfirm.IsVisible = false;
            _model.Message = "All art is in use; nothing to remove.";
            return;
        }

        var list = new StackPanel { Spacing = 2 };
        list.Children.Add(Ui.Wrapped($"Remove {unused.Count} unused asset{(unused.Count == 1 ? "" : "s")}? Nothing in the game uses {(unused.Count == 1 ? "it" : "them")}. Undo brings {(unused.Count == 1 ? "it" : "them")} back.", "small"));
        var names = new StackPanel { Name = "UnusedArtList", Spacing = 1 };
        foreach (var asset in unused) names.Children.Add(Ui.Text($"• {asset.Name} · {asset.Id}", "muted", "small"));
        list.Children.Add(new ScrollViewer { Content = names, MaxHeight = 160 });
        var confirm = Ui.Button($"Remove {unused.Count}", RemoveUnused, "accent");
        confirm.Name = "ConfirmRemoveUnusedArtButton";
        var cancel = Ui.Button("Cancel", () => _unusedConfirm.IsVisible = false, "tool");
        cancel.Name = "CancelRemoveUnusedArtButton";
        list.Children.Add(Ui.HStack(8, confirm, cancel));
        _unusedConfirm.Child = list;
        _unusedConfirm.IsVisible = true;
    }

    private void RemoveUnused()
    {
        _unusedConfirm.IsVisible = false;
        _model.RemoveUnused();
        Refresh();
    }

    public void Refresh()
    {
        if (_workspace.Current is not { } project) return;
        _unusedConfirm.IsVisible = false;
        _refreshing = true;
        try
        {
            _assets.SelectedItem = null;
            _assets.Items.Clear();
            var pixelArt = project.Graphics.OrNull()?.PixelArt != false;
            foreach (var asset in project.CustomAssets.Where(asset => !AudioImport.IsAudio(asset)))
            {
                var text = $"{asset.Name} · {SizeText(asset)}";
                var thumbnail = new Image { Width = 32, Height = 32, Stretch = Stretch.Uniform, Source = Thumbnail(asset) };
                RenderOptions.SetBitmapInterpolationMode(thumbnail, pixelArt ? BitmapInterpolationMode.None : BitmapInterpolationMode.HighQuality);
                var item = new ListBoxItem { Tag = asset.Id, Content = Ui.HStack(8, thumbnail, Ui.Text(text)) };
                AutomationProperties.SetName(item, text);
                _assets.Items.Add(item);
            }
            foreach (var stale in _thumbnails.Keys.Where(id => !project.CustomAssets.Any(asset => asset.Id == id)).ToList())
            {
                _thumbnails[stale].Bitmap?.Dispose();
                _thumbnails.Remove(stale);
            }
            _assets.SelectedItem = _assets.Items.OfType<ListBoxItem>().FirstOrDefault(item => Equals(item.Tag, _model.SelectedAssetId));
            var previousSound = SelectedSoundId;
            _sounds.SelectedItem = null;
            _sounds.Items.Clear();
            foreach (var asset in project.CustomAssets.Where(AudioImport.IsAudio))
            {
                var text = $"{asset.Name} · {EditorStrings.Get(asset.Type == CustomAssetTypes.Music ? "art.music" : "art.sound")} · {DisplayFormat.FileSize(AudioImport.Size(asset))}";
                var item = new ListBoxItem { Tag = asset.Id, Content = Ui.Text(text, "small") };
                AutomationProperties.SetName(item, text);
                _sounds.Items.Add(item);
            }
            _sounds.SelectedItem = _sounds.Items.OfType<ListBoxItem>().FirstOrDefault(item => Equals(item.Tag, previousSound));
            if (_sounds.Items.Count == 0) _sounds.Items.Add(new ListBoxItem { Content = Ui.Text(EditorStrings.Get("art.noSounds"), "muted", "small"), IsEnabled = false });
            if (_playingSoundId is { } playing && !project.CustomAssets.Any(asset => asset.Id == playing)) StopSound();
            var previousTile = (_tileType.SelectedItem as ComboBoxItem)?.Tag as string;
            _tileType.SelectedItem = null;
            _tileType.Items.Clear();
            foreach (var terrain in ContentReadouts.Terrains(project))
            {
                _tileType.Items.Add(new ComboBoxItem { Content = string.IsNullOrWhiteSpace(terrain.Name) ? terrain.Id : terrain.Name, Tag = terrain.Id });
            }
            _tileType.SelectedItem = _tileType.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, previousTile)) ?? _tileType.Items.OfType<ComboBoxItem>().FirstOrDefault();
            _pixelArt.IsChecked = pixelArt;
            var previousTarget = (_target.SelectedItem as ComboBoxItem)?.Tag;
            _target.Items.Clear();
            void Target(string label, string kind, string id) => _target.Items.Add(new ComboBoxItem { Content = label, Tag = (kind, id) });
            Target("Player", "player", "");
            Target("Map brush", "tiles", "");
            foreach (var npc in project.Npcs) Target($"NPC · {npc.Name}", "npc", npc.Id);
            foreach (var item in project.Items) Target($"Item · {item.Name}", "item", item.Id);
            foreach (var crop in project.CustomCrops.OrEmpty()) Target($"Crop · {crop.Name}", "crop", crop.Id);
            foreach (var node in project.NodeTypes) Target($"Node · {node.Name}", "node", node.Id);
            foreach (var animal in project.AnimalSpecies) Target($"Animal · {animal.Name}", "animal", animal.Id);
            foreach (var machine in project.MachineTypes) Target($"Machine · {machine.Name}", "machine", machine.Id);
            _target.SelectedItem = _target.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, previousTarget)) ?? _target.Items[0];
            var previousBinding = (_bindingAsset.SelectedItem as ComboBoxItem)?.Tag as string;
            _bindingAsset.Items.Clear();
            foreach (var asset in project.CustomAssets.Where(asset => !AudioImport.IsAudio(asset))) _bindingAsset.Items.Add(new ComboBoxItem { Tag = asset.Id, Content = asset.Name });
            _bindingAsset.SelectedItem = _bindingAsset.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, previousBinding))
                ?? _bindingAsset.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, _model.SelectedAssetId));
            var previousSource = (_frameSource.SelectedItem as ComboBoxItem)?.Tag as string;
            _frameSource.SelectedItem = null;
            _frameSource.Items.Clear();
            foreach (var asset in project.CustomAssets.Where(asset => !AudioImport.IsAudio(asset))) _frameSource.Items.Add(new ComboBoxItem { Tag = asset.Id, Content = asset.Name });
            _frameSource.SelectedItem = _frameSource.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, previousSource));
        }
        finally
        {
            _refreshing = false;
        }
        RefreshSelected();
        RefreshBindingClips();
        ShowSelectedSound();
    }

    private void RefreshSelected()
    {
        var asset = SelectedAsset();
        _assetName.Text = asset?.Name ?? "";
        var previousClip = (_clips.SelectedItem as ComboBoxItem)?.Tag as string;
        _refreshing = true;
        try
        {
            _clips.SelectedItem = null;
            _clips.Items.Clear();
            foreach (var clip in asset?.Animations.OrEmpty() ?? []) _clips.Items.Add(new ComboBoxItem { Tag = clip.Name, Content = clip.Name });
            _clips.SelectedItem = _clips.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, previousClip)) ?? _clips.Items.OfType<ComboBoxItem>().FirstOrDefault();
        }
        finally
        {
            _refreshing = false;
        }
        RefreshSheet(asset);
        RefreshFrames();
        DrawPreview();
        UpdatePreviewTimer();
    }

    /// <summary>The asset's list thumbnail (at most 64px, for crisp 32px display), decoded once per image.</summary>
    private Bitmap? Thumbnail(CustomAsset asset)
    {
        if (_thumbnails.TryGetValue(asset.Id, out var cached))
        {
            if (cached.DataUrl == asset.DataUrl) return cached.Bitmap;
            cached.Bitmap?.Dispose();
        }

        var bitmap = ArtBitmaps.Thumbnail(asset, 64);
        _thumbnails[asset.Id] = (asset.DataUrl, bitmap);
        return bitmap;
    }

    /// <summary>Shows the selected asset's whole image on the sprite sheet (decoded again only when it changes).</summary>
    private void RefreshSheet(CustomAsset? asset)
    {
        if (asset is not null && _sheetKey is { } key && key.AssetId == asset.Id && key.DataUrl == asset.DataUrl) return;
        var previous = _sheet.Image;
        _sheet.SetImage(asset is null ? null : ArtBitmaps.Decode(asset.DataUrl));
        previous?.Dispose();
        _sheetKey = asset is null ? null : (asset.Id, asset.DataUrl);
        RefreshSheetGrid();
    }

    private void RefreshSheetGrid()
    {
        static int Cell(TextBox box) => int.TryParse(box.Text, NumberStyles.Integer, CultureInfo.InvariantCulture, out var value) && value > 0 ? value : 0;
        _sheet.SetCell(Cell(_frameWidth), Cell(_frameHeight));
    }

    private void DrawPreview()
    {
        var asset = SelectedAsset();
        var project = _workspace.Current;
        if (asset is null || project is null) { VisualPreview.Release(_preview.Child as Image); _preview.Child = null; return; }
        var visual = VisualRef.Default.WithAssetId(asset.Id).WithAnimation((_clips.SelectedItem as ComboBoxItem)?.Tag as string);
        // The Rust renderer resolves and draws the frame, exactly as the game will show it, into
        // the bitmap already on show when the size is the same.
        _preview.Child = _art.Draw(_preview.Child as Image, project, visual, _tick, _preview.Width);
    }

    /// <summary>
    /// "1. 16,0 · 16×16" for a cell of the asset's own image, "2. from Carrot.png · 16×16" for a
    /// frame drawn from another image.
    /// </summary>
    private string FrameLabel(CustomAsset asset, int index, ArtFrame frame)
    {
        var size = $"{Number(frame.Width)}×{Number(frame.Height)}";
        var position = $"{Number(frame.X)},{Number(frame.Y)}";
        if (frame.AssetId.OrNull() is not { } sourceId || sourceId == asset.Id) return $"{index + 1}. {position} · {size}";
        var source = _workspace.Current?.CustomAssets.FirstOrDefault(other => other.Id == sourceId);
        var at = frame.X == 0 && frame.Y == 0 ? "" : $" at {position}";
        return $"{index + 1}. from {source?.Name ?? $"(missing: {sourceId})"}{at} · {size}";
    }

    private void RefreshFrames()
    {
        _frames.Children.Clear();
        var clip = SelectedClip();
        if (clip is null || SelectedAsset() is not { } owner) return;
        for (var index = 0; index < clip.Frames.Length; index++)
        {
            var i = index;
            var frame = clip.Frames[i];
            var label = Ui.Text(FrameLabel(owner, i, frame), "muted", "small");
            label.Name = $"ArtFrameLabel_{i}";
            var ticks = new TextBox { Name = $"ArtFrameTicks_{i}", Text = Number(frame.Ticks), Width = 52 };
            ToolTip.SetTip(ticks, $"Frame {i + 1} duration in ticks");
            Ui.Label((ticks, $"Frame {i + 1} duration in ticks"));
            var setTicks = Ui.Button("Set", () =>
            {
                if (SelectedAsset() is not { } asset) return;
                try
                {
                    _model.SetFrameTicks(clip.Name, i, Positive(ticks));
                }
                catch (FormatException error) { _model.Message = error.Message; }
            }, "tool", "small");
            setTicks.Name = $"ArtFrameTicksSet_{i}";
            Ui.Label((setTicks, $"Set frame {i + 1} duration"));
            var duplicate = Ui.Button("Copy", () =>
            {
                if (SelectedAsset() is { } asset) _model.DuplicateFrame(clip.Name, i);
            }, "tool", "small");
            duplicate.Name = $"ArtFrameDuplicate_{i}";
            duplicate.IsEnabled = clip.Frames.Length < 1024;
            ToolTip.SetTip(duplicate, "Duplicate this frame");
            Ui.Label((duplicate, $"Duplicate frame {i + 1}"));
            var up = Ui.Button("↑", () =>
            {
                _model.MoveFrame(clip, i);
                SelectClip(clip.Name);
            }, "tool", "small");
            up.IsEnabled = i > 0;
            AutomationProperties.SetName(up, $"Move frame {i + 1} earlier");
            var remove = Ui.Button("×", () =>
            {
                _model.RemoveFrame(clip, i);
                SelectClip(clip.Name);
            }, "tool", "small");
            AutomationProperties.SetName(remove, $"Remove frame {i + 1}");
            _frames.Children.Add(Ui.Row(label, ticks, setTicks, duplicate, up, remove));
        }
    }

    private void SelectClip(string name) => _clips.SelectedItem = _clips.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, name));

    private void SliceSheet()
    {
        if (SelectedAsset() is not { Width: not null, Height: not null }) return;
        try
        {
            var clip = _model.Slice(Positive(_frameWidth), Positive(_frameHeight), Positive(_frameTicks), _clipName.Text, _loop.IsChecked == true);
            SelectClip(clip.Name);
        }
        catch (FormatException error) { _model.Message = error.Message; }
    }

    private void AppendFrame()
    {
        if (SelectedAsset() is not { } asset) return;
        try
        {
            var width = Positive(_frameWidth);
            var height = Positive(_frameHeight);
            var x = Nonnegative(_frameX);
            var y = Nonnegative(_frameY);
            var ticks = Positive(_frameTicks);
            var (imageWidth, imageHeight) = ImageSize(asset);
            AddFrame(asset, ArtEditorViewModel.Frame(x, y, width, height, ticks, imageWidth, imageHeight, snap: false));
        }
        catch (FormatException error) { _model.Message = error.Message; }
    }

    /// <summary>
    /// Adds the sprite sheet cell under image pixel (<paramref name="x"/>, <paramref name="y"/>)
    /// of the selected asset to the clip named in the Clip name box, creating the clip when needed
    /// (web AssetManager sheet click): a frame width × height cell with the current ticks, as one
    /// undo step.
    /// </summary>
    public void AppendFrameAt(double x, double y)
    {
        if (SelectedAsset() is not { } asset) return;
        try
        {
            var width = Positive(_frameWidth);
            var height = Positive(_frameHeight);
            var ticks = Positive(_frameTicks);
            var (imageWidth, imageHeight) = ImageSize(asset);
            AddFrame(asset, ArtEditorViewModel.Frame(x, y, width, height, ticks, imageWidth, imageHeight, snap: true));
        }
        catch (FormatException error) { _model.Message = error.Message; }
    }

    /// <summary>
    /// Adds the whole image picked under "Separate image" as the next frame of the clip (web
    /// AssetManager "Add image frame"), so one animation can use frames of several images. The
    /// renderer draws a frame with an <c>AssetId</c> from that asset.
    /// </summary>
    private void AddImageFrame()
    {
        if (SelectedAsset() is not { } asset) return;
        var sourceId = (_frameSource.SelectedItem as ComboBoxItem)?.Tag as string;
        if (_workspace.Current?.CustomAssets.FirstOrDefault(other => other.Id == sourceId) is not { } source)
        {
            _model.Message = "Choose the image to add.";
            return;
        }

        try
        {
            if (source.Width.OrNullable() is not { } width || source.Height.OrNullable() is not { } height) throw new FormatException("Reimport this art to read its size.");
            var ticks = Positive(_frameTicks);
            // A frame of the clip's own asset needs no AssetId.
            AddFrame(asset, ArtFrame.Default.WithAssetId(source.Id == asset.Id ? null : source.Id).WithWidth(width).WithHeight(height).WithTicks(ticks));
        }
        catch (FormatException error) { _model.Message = error.Message; }
    }

    /// <summary>The asset's size in pixels: the size recorded at import, else the sheet's decoded image (legacy art).</summary>
    private (double Width, double Height) ImageSize(CustomAsset asset)
    {
        if (asset.Width.OrNullable() is { } width && asset.Height.OrNullable() is { } height) return (width, height);
        if (_sheetKey?.AssetId == asset.Id && _sheet.Image is { } image) return (image.PixelSize.Width, image.PixelSize.Height);
        throw new FormatException("Reimport this art to read its size.");
    }

    /// <summary>
    /// Appends <paramref name="frame"/> to the clip named in the Clip name box, creating the clip
    /// when it does not exist yet: one F# edit (one undo step).
    /// </summary>
    private void AddFrame(CustomAsset asset, ArtFrame frame)
    {
        var clip = _model.AddFrame(frame, _clipName.Text, _loop.IsChecked == true);
        SelectClip(clip.Name);
    }

    private void SetAllFrameTicks()
    {
        if (SelectedClip() is not { } clip) return;
        try { _model.SetAllFrameTicks(clip.Name, Positive(_frameTicks)); }
        catch (FormatException error) { _model.Message = error.Message; }
    }

    private void RemoveClip()
    {
        if (SelectedClip() is { } clip) _model.RemoveClip(clip.Name);
    }

    private void RenameAsset() => _model.Rename(_model.SelectedAssetId, _assetName.Text);

    private void RemoveAsset() => _model.Remove(_model.SelectedAssetId, art: true);

    private void RefreshBindingClips()
    {
        var id = (_bindingAsset.SelectedItem as ComboBoxItem)?.Tag as string;
        var asset = _workspace.Current?.CustomAssets.FirstOrDefault(item => item.Id == id);
        var previous = (_bindingClip.SelectedItem as ComboBoxItem)?.Tag as string;
        _bindingClip.SelectedItem = null;
        _bindingClip.Items.Clear();
        _bindingClip.Items.Add(new ComboBoxItem { Content = "Automatic", Tag = "" });
        foreach (var clip in asset?.Animations.OrEmpty() ?? []) _bindingClip.Items.Add(new ComboBoxItem { Content = clip.Name, Tag = clip.Name });
        _bindingClip.SelectedItem = _bindingClip.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, previous)) ?? _bindingClip.Items[0];
    }

    private void RefreshCellControls()
    {
        _cellSize.IsEnabled = _singleCell.IsChecked == true;
        _cellColumn.IsEnabled = _singleCell.IsChecked == true;
        _cellRow.IsEnabled = _singleCell.IsChecked == true;
    }

    private void Bind()
    {
        if (_workspace.Current is not { } project || _target.SelectedItem is not ComboBoxItem { Tag: ValueTuple<string, string> target }
            || _bindingAsset.SelectedItem is not ComboBoxItem { Tag: string assetId }) return;
        var asset = project.CustomAssets.FirstOrDefault(item => item.Id == assetId);
        if (asset is null) return;
        try
        {
            var clip = (_bindingClip.SelectedItem as ComboBoxItem)?.Tag as string;
            var cell = _singleCell.IsChecked == true;
            var visual = _model.Visual(assetId, clip, cell ? Positive(_cellSize) : null,
                cell ? Nonnegative(_cellColumn) : 0, cell ? Nonnegative(_cellRow) : 0);
            if (target.Item1 == "tiles")
            {
                if (_tileType.SelectedItem is ComboBoxItem { Tag: string type }) _useMapBrush(type, visual);
            }
            else _model.Bind(target.Item1, target.Item2, visual);
            _model.Message = "Artwork assigned.";
        }
        catch (FormatException error) { _model.Message = error.Message; }
    }
}
