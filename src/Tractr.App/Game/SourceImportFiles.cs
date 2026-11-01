using FarmEngine.Authoring;
using FarmEngine.Schemas;

namespace Tractr.App.Game;

/// <summary>
/// The file side of the Aseprite, Tiled and LDtk importers (#232, #233): reads the picked file
/// and the files beside it with a size limit, and imports every image through
/// <see cref="ArtImport"/> (its header checks and re-encoding, docs/dev/IMAGE-SAFETY.md). The
/// F# importers (<see cref="SourceImports"/>) do the parsing, mapping and merging.
/// </summary>
internal static class SourceImportFiles
{
    /// <summary>The largest map, tileset or sprite file read.</summary>
    public const int MaxBytes = 16 * 1024 * 1024;

    /// <summary>The files beside a map that are read: tilesets and LDtk levels.</summary>
    private static readonly string[] SideFiles = [".tsx", ".tsj", ".json", ".ldtkl", ".ldtk"];

    /// <summary>A sprite sheet ready to import: what the file holds and its image.</summary>
    public sealed record SpriteSheetFile(string Path, SpriteSheetSource Sheet, string DataUrl, int Width, int Height);

    public static byte[] ReadLimited(string path)
    {
        var info = new FileInfo(path);
        if (!info.Exists) throw new FileNotFoundException($"{System.IO.Path.GetFileName(path)} was not found.", path);
        if (info.Length > MaxBytes) throw new ArgumentException($"{info.Name} is over {MaxBytes / (1024 * 1024)} MB.");
        return File.ReadAllBytes(path);
    }

    /// <summary>
    /// Reads an Aseprite file (its frames drawn onto one sheet) or a sheet export's JSON with the
    /// image it names (beside it, else the JSON's own name with <c>.png</c>).
    /// </summary>
    public static SpriteSheetFile LoadSpriteSheet(GameProject project, string path)
    {
        var sheet = SourceImports.ReadSpriteSheet(path, ReadLimited(path));
        if (SourceImports.SheetDataUrl(sheet) is { } url)
            return new SpriteSheetFile(path, sheet, url, sheet.SheetWidth, sheet.SheetHeight);
        var directory = System.IO.Path.GetDirectoryName(path) ?? ".";
        var image = sheet.Image is { Value: { Length: > 0 } name }
            ? System.IO.Path.GetFullPath(System.IO.Path.Combine(directory, name))
            : System.IO.Path.ChangeExtension(path, ".png");
        var asset = ArtImport.FromBytes(project, System.IO.Path.GetFileName(image), ReadLimited(image));
        return new SpriteSheetFile(path, sheet, asset.DataUrl, (int)(asset.Width?.Value ?? 0), (int)(asset.Height?.Value ?? 0));
    }

    /// <summary>The preview of <paramref name="file"/>: each tag and slice and its clip.</summary>
    public static IReadOnlyList<ClipMapping> Preview(GameProject project, SpriteSheetFile file, IReadOnlyDictionary<string, string> names) =>
        SourceImports.PreviewSpriteSheet(project, file.Path, file.Sheet, names);

    /// <summary>Imports (or re-imports) <paramref name="file"/> with the clip names chosen.</summary>
    public static AsepriteImportResult ImportSpriteSheet(GameProject project, SpriteSheetFile file, string assetType, IReadOnlyDictionary<string, string> names) =>
        SourceImports.ImportSpriteSheet(
            project,
            file.Path,
            System.IO.Path.GetFileNameWithoutExtension(file.Path),
            assetType,
            file.Sheet,
            file.DataUrl,
            file.Width,
            file.Height,
            names);

    /// <summary>The level identifiers of an LDtk project file (empty for a Tiled map).</summary>
    public static IReadOnlyList<string> Levels(string path)
    {
        var text = System.Text.Encoding.UTF8.GetString(ReadLimited(path));
        return SourceImports.IsLdtk(path, text) ? SourceImports.LdtkLevels(text) : [];
    }

    /// <summary>Imports (or re-imports) a Tiled map or an LDtk level as a scene.</summary>
    public static MapImportResult ImportMap(GameProject project, string path, string? level)
    {
        var text = System.Text.Encoding.UTF8.GetString(ReadLimited(path));
        var source = SourceImports.ReadMap(path, text, level, file =>
        {
            if (!SideFiles.Contains(System.IO.Path.GetExtension(file).ToLowerInvariant())) return null;
            try
            {
                return System.Text.Encoding.UTF8.GetString(ReadLimited(file));
            }
            catch (Exception error) when (error is IOException or ArgumentException or UnauthorizedAccessException)
            {
                return null;
            }
        });
        return SourceImports.ImportMap(project, path, text, level, source, file =>
        {
            try
            {
                var asset = ArtImport.FromBytes(project, System.IO.Path.GetFileName(file), ReadLimited(file));
                return new ImportedImage(asset.DataUrl, (int)(asset.Width?.Value ?? 0), (int)(asset.Height?.Value ?? 0));
            }
            catch (Exception error) when (error is IOException or ArgumentException or UnauthorizedAccessException)
            {
                return null;
            }
        });
    }

    /// <summary>A one-paragraph summary of a map import's report.</summary>
    public static string Summary(MapImportResult result)
    {
        var report = result.Report.ToList();
        var verb = result.Created ? "Imported" : "Re-imported";
        if (report.Count == 0) return $"{verb} scene {result.SceneId}; everything mapped.";
        var lines = report.Take(6).Select(p => $"{p.SeverityName}: {p.Message} ({p.Path})");
        var more = report.Count > 6 ? $" …and {report.Count - 6} more." : "";
        return $"{verb} scene {result.SceneId}. {report.Count} note{(report.Count == 1 ? "" : "s")}: " + string.Join(" · ", lines) + more;
    }
}
