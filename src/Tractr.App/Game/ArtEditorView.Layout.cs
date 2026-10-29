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

public sealed partial class ArtEditorView
{
    private void BuildLayout()
    {
        var left = new StackPanel { Spacing = 10, Margin = new Thickness(0, 0, 20, 0) };
        left.Children.Add(Ui.Text("ARTWORK", "section"));
        left.Children.Add(Ui.Wrapped("Import PNG, JPEG, WebP, GIF, BMP or SVG. Animated images and SVGs become a still PNG; add clips below.", "muted", "small"));
        var import = Ui.AsyncButton("Import images", PickImagesAsync, error => _model.Message = $"Could not import: {error.Message}", "accent");
        import.Name = "ImportArtButton";
        ToolTip.SetTip(import, "Choose one or more images; they are added as one undo step.");
        left.Children.Add(import);
        var svgSize = Ui.HStack(6, Ui.Text("SVG size (longest side, px)", "muted", "small"), _svgSize);
        ToolTip.SetTip(svgSize, "Leave empty to use the SVG's own size.");
        left.Children.Add(svgSize);
        left.Children.Add(_assets);
        var removeUnused = Ui.Button("Remove unused art…", ConfirmRemoveUnused, "tool");
        removeUnused.Name = "RemoveUnusedArtButton";
        ToolTip.SetTip(removeUnused, "Delete every asset nothing in the game uses");
        left.Children.Add(removeUnused);
        left.Children.Add(_unusedConfirm);
        left.Children.Add(_pixelArt);
        _pixelArt.Click += (_, _) => _model.SetPixelArt(_pixelArt.IsChecked == true);
        left.Children.Add(_message);
        left.Children.Add(Ui.Text(EditorStrings.Get("art.sounds"), "section"));
        left.Children.Add(Ui.Wrapped(EditorStrings.Get("art.soundsHelp"), "muted", "small"));
        var importSounds = Ui.AsyncButton(EditorStrings.Get("art.importSounds"), () => PickSoundsAsync(CustomAssetTypes.Sound), error => _model.Message = EditorStrings.Format("art.couldNotImport", error.Message), "tool");
        importSounds.Name = "ImportSoundButton";
        ToolTip.SetTip(importSounds, EditorStrings.Get("art.importSoundsTip"));
        var importMusic = Ui.AsyncButton(EditorStrings.Get("art.importMusic"), () => PickSoundsAsync(CustomAssetTypes.Music), error => _model.Message = EditorStrings.Format("art.couldNotImport", error.Message), "tool");
        importMusic.Name = "ImportMusicButton";
        ToolTip.SetTip(importMusic, EditorStrings.Get("art.importMusicTip"));
        left.Children.Add(Ui.HStack(8, importSounds, importMusic));
        left.Children.Add(_sounds);
        var renameSound = Ui.Button("Rename", RenameSound, "tool");
        renameSound.Name = "RenameSoundButton";
        ToolTip.SetTip(renameSound, "Rename the selected sound or music; everything that plays it still does");
        left.Children.Add(Ui.HStack(8, _soundName, renameSound));
        _playSound = Ui.Button(Ui.IconLabel("IconPlay", "Play"), () => _ = ToggleSoundAsync(), "tool");
        _playSound.Name = "PlaySoundButton";
        ToolTip.SetShowOnDisabled(_playSound, true);
        var removeSound = Ui.Button(EditorStrings.Get("art.removeSound"), RemoveSound, "tool");
        removeSound.Name = "RemoveSoundButton";
        ToolTip.SetTip(removeSound, EditorStrings.Get("art.removeSoundTip"));
        left.Children.Add(Ui.HStack(8, _playSound, removeSound));

        var right = new StackPanel { Spacing = 10 };
        right.Children.Add(_preview);
        _play = Ui.Button("Pause preview", TogglePreview, "tool", "small");
        _play.Name = "ArtPreviewPlayButton";
        _play.HorizontalAlignment = HorizontalAlignment.Center;
        right.Children.Add(_play);
        right.Children.Add(Ui.Text("Asset name", "muted", "small"));
        right.Children.Add(_assetName);
        var rename = Ui.Button("Rename", RenameAsset, "tool");
        rename.Name = "RenameArtButton";
        var remove = Ui.Button("Remove asset", RemoveAsset, "tool");
        remove.Name = "RemoveArtButton";
        right.Children.Add(Ui.HStack(8, rename, remove));
        right.Children.Add(Ui.Text("ANIMATION CLIPS", "section"));
        right.Children.Add(_clips);
        right.Children.Add(Ui.HStack(8, Ui.Text("Name", "muted", "small"), _clipName, _loop));
        right.Children.Add(Ui.HStack(8, Ui.Text("Frame", "muted", "small"), _frameWidth, Ui.Text("×"), _frameHeight,
            Ui.Text("at", "muted", "small"), _frameX, Ui.Text(","), _frameY, Ui.Text("ticks", "muted", "small"), _frameTicks));
        right.Children.Add(Ui.Wrapped("Click a cell of the sheet to add it to the clip, or type its position and use Append frame.", "muted", "small"));
        right.Children.Add(new ScrollViewer
        {
            Name = "ArtSheetScroll",
            Content = _sheet,
            MaxHeight = 300,
            HorizontalAlignment = HorizontalAlignment.Left,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        });
        var slice = Ui.Button("Slice entire sheet", SliceSheet, "tool");
        slice.Name = "SliceArtButton";
        var append = Ui.Button("Append frame", AppendFrame, "tool");
        append.Name = "AppendArtFrameButton";
        var removeClip = Ui.Button("Remove clip", RemoveClip, "tool");
        right.Children.Add(Ui.HStack(8, slice, append, removeClip));
        _addImageFrame = Ui.Button("Add image frame", AddImageFrame, "tool");
        _addImageFrame.Name = "ArtAddImageFrameButton";
        _addImageFrame.IsEnabled = false;
        _frameSource.SelectionChanged += (_, _) => _addImageFrame.IsEnabled = _frameSource.SelectedItem is not null;
        ToolTip.SetTip(_addImageFrame, "Append the whole chosen image as the next frame: one animation can use frames of several images.");
        AutomationProperties.SetName(_frameSource, "Separate image to add as a frame");
        right.Children.Add(Ui.HStack(8, Ui.Text("Separate image", "muted", "small"), _frameSource, _addImageFrame));
        var allTicks = Ui.Button("Set every frame to these ticks", SetAllFrameTicks, "tool");
        allTicks.Name = "ArtAllFrameTicksButton";
        right.Children.Add(Ui.HStack(8, allTicks, Ui.Text("20 ticks = 1 second", "muted", "small")));
        right.Children.Add(_frames);
        right.Children.Add(Ui.Text("ASSIGN ARTWORK", "section"));
        right.Children.Add(Ui.HStack(8, Ui.Text("Target", "muted", "small"), _target));
        right.Children.Add(Ui.HStack(8, Ui.Text("Asset", "muted", "small"), _bindingAsset, Ui.Text("Clip", "muted", "small"), _bindingClip));
        _singleCell.Click += (_, _) => RefreshCellControls();
        right.Children.Add(Ui.HStack(8, _singleCell, Ui.Text("size", "muted", "small"), _cellSize,
            Ui.Text("column", "muted", "small"), _cellColumn, Ui.Text("row", "muted", "small"), _cellRow));
        right.Children.Add(Ui.HStack(8, Ui.Text("Map tile behavior", "muted", "small"), _tileType));
        var bind = Ui.Button("Assign artwork", Bind, "accent");
        bind.Name = "BindArtButton";
        right.Children.Add(bind);
        var grid = new Grid { ColumnDefinitions = new ColumnDefinitions("250,*"), Margin = new Thickness(20) };
        grid.Children.Add(left);
        var scroll = new ScrollViewer { Content = right };
        Grid.SetColumn(scroll, 1);
        grid.Children.Add(scroll);
        Content = grid;
    }
}
