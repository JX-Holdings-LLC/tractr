using System.Globalization;
using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using Tractr.App.Game;
using Tractr.App.Localization;
using Tractr.App.Mvvm;
using Tractr.App.Projects;

namespace Tractr.App.ViewModels.EditMode;

/// <summary>The art studio's selection, imports, animation edits and bindings, independent of controls.
/// Decoding imports runs on a worker; edits return to the caller's synchronization context.</summary>
public sealed class ArtEditorViewModel(ProjectWorkspace workspace) : ObservableObject
{
    private bool _retired;
    private string? _selectedAssetId;
    private string? _selectedSoundId;
    private string _message = "Import an image to start.";
    public string? SelectedAssetId { get => _selectedAssetId; set => SetProperty(ref _selectedAssetId, value); }
    public string? SelectedSoundId { get => _selectedSoundId; set => SetProperty(ref _selectedSoundId, value); }
    public string Message { get => _message; set => SetProperty(ref _message, value); }
    public CustomAsset? SelectedAsset => Find(SelectedAssetId);
    public CustomAsset? Find(string? id) => workspace.Current?.CustomAssets.FirstOrDefault(asset => asset.Id == id);
    public IReadOnlyList<CustomAsset> Unused => workspace.Current is { } project ? ArtLibrary.Unused(project) : [];

    public static string SizeText(CustomAsset asset) =>
        asset.Width.OrNullable() is { } width && asset.Height.OrNullable() is { } height
            ? $"{DisplayFormat.Number(width)}×{DisplayFormat.Number(height)}" : "?×?";

    public void Retire() => _retired = true;

    public void SetPixelArt(bool enabled) => workspace.Apply(Edits.SetGraphics(GraphicsSettings.Default.WithPixelArt(enabled)));

    private static int? SvgSide(string? text)
    {
        if (string.IsNullOrWhiteSpace(text)) return null;
        if (!int.TryParse(text.Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out var side) || side <= 0)
            throw new FormatException("The SVG size must be a positive whole number of pixels.");
        return side;
    }

    private sealed record Decoded(GameProject Project, List<CustomAsset> Assets, List<string> Errors, string Type);

    private static Decoded Decode(GameProject project, IReadOnlyList<(string FileName, byte[] Bytes)> files,
        string type, int? svgSide, IReadOnlyList<string> readErrors)
    {
        var errors = new List<string>(readErrors);
        var assets = new List<CustomAsset>();
        foreach (var (fileName, bytes) in files)
        {
            try
            {
                assets.Add(type == CustomAssetTypes.Art
                    ? ArtImport.FromBytes(project, fileName, bytes, svgSide)
                    : AudioImport.FromBytes(project, fileName, bytes, type));
            }
            catch (ArgumentException error) { errors.Add($"{fileName}: {error.Message}"); }
        }
        return new Decoded(project, assets, errors, type);
    }

    private void ApplyImport(Decoded decoded)
    {
        if (_retired) return;
        if (workspace.Current is not { } project || project.Id != decoded.Project.Id)
        {
            Message = "The project changed while importing; choose the files again in the intended project.";
            return;
        }
        var imported = new List<CustomAsset>();
        if (decoded.Assets.Count > 0)
        {
            var before = project.CustomAssets.Select(asset => asset.Id).ToHashSet();
            // Rebase on current content: edits made while decoding must not be overwritten.
            workspace.Apply(ArtLibrary.Import(project, decoded.Assets));
            imported = workspace.Current!.CustomAssets.Where(asset => !before.Contains(asset.Id)).ToList();
            if (imported.Count > 0)
            {
                if (decoded.Type == CustomAssetTypes.Art) SelectedAssetId = imported[^1].Id;
                else SelectedSoundId = imported[^1].Id;
            }
        }
        var music = decoded.Type == CustomAssetTypes.Music;
        var summary = decoded.Type == CustomAssetTypes.Art
            ? imported.Count switch { 0 => "", 1 => $"Imported {imported[0].Name} ({SizeText(imported[0])}).", _ => $"Imported {imported.Count} images." }
            : imported.Count switch
            {
                0 => "",
                1 => EditorStrings.Format(music ? "art.importedMusic" : "art.importedSound", imported[0].Name, DisplayFormat.FileSize(AudioImport.Size(imported[0]))),
                _ => EditorStrings.Format(music ? "art.importedMusicTracks" : "art.importedSounds", imported.Count),
            };
        var errorText = decoded.Type == CustomAssetTypes.Art
            ? $"Could not import {string.Join("; ", decoded.Errors)}"
            : EditorStrings.Format("art.couldNotImportFiles", string.Join("; ", decoded.Errors));
        Message = decoded.Errors.Count == 0 ? summary : string.Join(" ", new[] { summary, errorText }.Where(part => part.Length > 0));
    }

    public void Import(IReadOnlyList<(string FileName, byte[] Bytes)> files, string type, string? svgSize = null, IReadOnlyList<string>? errors = null)
    {
        if (workspace.Current is not { } project) return;
        try { ApplyImport(Decode(project, files, type, SvgSide(svgSize), errors ?? [])); }
        catch (FormatException error) { Message = error.Message; }
    }

    public async Task ImportAsync(IReadOnlyList<(string FileName, byte[] Bytes)> files, string type, string? svgSize = null, IReadOnlyList<string>? errors = null)
    {
        if (workspace.Current is not { } project) return;
        try
        {
            var side = SvgSide(svgSize);
            var decoded = await Task.Run(() => Decode(project, files, type, side, errors ?? []));
            ApplyImport(decoded);
        }
        catch (FormatException error) { Message = error.Message; }
    }

    public void Rename(string? id, string? name)
    {
        if (id is null || string.IsNullOrWhiteSpace(name)) return;
        if (workspace.Apply(Edits.RenameAsset(id, name))) Message = $"Renamed to {name.Trim()}.";
    }

    public void Remove(string? id, bool art)
    {
        if (Find(id) is not { } asset) return;
        if (SelectedAssetId == id) SelectedAssetId = null;
        if (SelectedSoundId == id) SelectedSoundId = null;
        if (workspace.Apply(Edits.RemoveAsset(asset.Id)))
            Message = art ? "Asset removed; bindings now use their default look." : $"Removed {asset.Name}.";
    }

    public void RemoveUnused()
    {
        if (workspace.Current is not { } project) return;
        var count = ArtLibrary.Unused(project).Count;
        if (!workspace.Apply(ArtLibrary.RemoveUnused(project))) return;
        if (Find(SelectedAssetId) is null) SelectedAssetId = null;
        Message = $"Removed {count} unused asset{(count == 1 ? "" : "s")}.";
    }

    public void SaveClip(AnimationClip clip)
    {
        if (SelectedAsset is not { } asset) return;
        var clips = asset.Animations.OrEmpty().Where(existing => existing.Name != clip.Name).Append(clip).ToList();
        workspace.Apply(Edits.UpsertAsset(asset.WithAnimations(clips)));
    }

    public AnimationClip Slice(int width, int height, int ticks, string? clipName, bool loop)
    {
        var asset = SelectedAsset ?? throw new FormatException("Choose an image.");
        if (width <= 0 || height <= 0 || ticks <= 0) throw new FormatException("Frame sizes and ticks must be positive.");
        var name = ClipName(clipName);
        var frames = new List<ArtFrame>();
        for (double y = 0; y + height <= asset.Height.Or(0) && frames.Count < 1024; y += height)
            for (double x = 0; x + width <= asset.Width.Or(0) && frames.Count < 1024; x += width)
                frames.Add(ArtFrame.Default.WithX(x).WithY(y).WithWidth(width).WithHeight(height).WithTicks(ticks));
        if (frames.Count == 0) throw new FormatException("No complete frames fit the image.");
        var clip = AnimationClip.Default.WithName(name).WithLoop(loop).WithFrames(frames);
        SaveClip(clip);
        Message = $"Sliced {frames.Count} frames into {name}.";
        return clip;
    }

    private static string ClipName(string? name) => string.IsNullOrWhiteSpace(name) ? throw new FormatException("Give the clip a name.") : name.Trim();

    public AnimationClip AddFrame(ArtFrame frame, string? clipName, bool loop)
    {
        var asset = SelectedAsset ?? throw new FormatException("Choose an image.");
        var name = ClipName(clipName);
        var frames = asset.Animations.OrEmpty().FirstOrDefault(clip => clip.Name == name)?.Frames.ToList() ?? [];
        if (frames.Count >= 1024) throw new FormatException("A clip may contain at most 1024 frames.");
        frames.Add(frame);
        var clip = AnimationClip.Default.WithName(name).WithLoop(loop).WithFrames(frames);
        SaveClip(clip);
        Message = $"Added frame {frames.Count} to {name}.";
        return clip;
    }

    public static ArtFrame Frame(double x, double y, int width, int height, int ticks, double imageWidth, double imageHeight, bool snap)
    {
        if (width <= 0 || height <= 0 || ticks <= 0) throw new FormatException("Frame sizes and ticks must be positive.");
        if (snap) { x = Math.Floor(x / width) * width; y = Math.Floor(y / height) * height; }
        if (!double.IsFinite(x) || !double.IsFinite(y) || x < 0 || y < 0 || x + width > imageWidth || y + height > imageHeight)
            throw new FormatException("That frame extends beyond the image.");
        return ArtFrame.Default.WithX(x).WithY(y).WithWidth(width).WithHeight(height).WithTicks(ticks);
    }

    public void SetFrameTicks(string clip, int index, int ticks)
    {
        if (SelectedAssetId is { } id) workspace.Apply(Edits.SetFrameTicks(id, clip, index, ticks));
    }
    public void DuplicateFrame(string clip, int index)
    {
        if (SelectedAssetId is { } id) workspace.Apply(Edits.DuplicateFrame(id, clip, index));
    }
    public void SetAllFrameTicks(string clip, int ticks)
    {
        if (SelectedAssetId is not { } id) return;
        workspace.Apply(Edits.SetAllFrameTicks(id, clip, ticks));
        Message = $"Every frame of {clip} now lasts {ticks} ticks.";
    }
    public void RemoveClip(string name)
    {
        if (SelectedAsset is { } asset)
            workspace.Apply(Edits.UpsertAsset(asset.WithAnimations(asset.Animations.OrEmpty().Where(clip => clip.Name != name).ToList())));
    }
    public void MoveFrame(AnimationClip clip, int index)
    {
        if (index <= 0 || index >= clip.Frames.Length) return;
        var frames = clip.Frames.ToList();
        (frames[index - 1], frames[index]) = (frames[index], frames[index - 1]);
        SaveClip(clip.WithFrames(frames));
    }
    public void RemoveFrame(AnimationClip clip, int index)
    {
        var frames = clip.Frames.Where((_, n) => n != index).ToList();
        if (frames.Count == 0) RemoveClip(clip.Name);
        else SaveClip(clip.WithFrames(frames));
    }

    public VisualRef Visual(string assetId, string? clip, int? size, int column, int row)
    {
        var asset = Find(assetId) ?? throw new FormatException("Choose an image.");
        ArtFrame? frame = size is { } cell
            ? Frame((double)column * cell, (double)row * cell, cell, cell, 6, asset.Width.Or(0), asset.Height.Or(0), false) : null;
        return VisualRef.Default.WithAssetId(assetId).WithAnimation(string.IsNullOrEmpty(clip) ? null : clip).WithFrame(frame);
    }
    public void Bind(string kind, string id, VisualRef visual)
    {
        var edit = kind switch
        {
            "player" => Edits.BindPlayerVisual(visual),
            "npc" => Edits.BindNpcVisual(id, visual),
            "item" => Edits.BindItemVisual(id, visual),
            "crop" => Edits.BindCropVisual(id, visual),
            "node" => Edits.BindNodeTypeVisual(id, visual),
            "animal" => Edits.BindAnimalSpeciesVisual(id, visual),
            "machine" => Edits.BindMachineTypeVisual(id, visual),
            _ => null,
        };
        if (edit is not null) workspace.Apply(edit);
        Message = "Artwork assigned.";
    }
}
