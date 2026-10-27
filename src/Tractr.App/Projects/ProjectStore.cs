using System.Text.Json;
using System.Text.Json.Nodes;
using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using Microsoft.FSharp.Collections;
using Tractr.Updates;

namespace Tractr.App.Projects;

/// <summary>When a project file was last written, and how big it is: a change by anything else shows.</summary>
public readonly record struct FileStamp(DateTime LastWriteUtc, long Length);

/// <summary>A row of the project list (web <c>ProjectIndexEntry</c> + file size).</summary>
public sealed record ProjectSummary(string Id, string Name, DateTimeOffset UpdatedAt, long SizeBytes);

/// <summary>Outcome of loading/importing a project: the project or user-facing errors.</summary>
public sealed record ProjectLoadResult(GameProject? Project, IReadOnlyList<string> Errors)
{
    public bool Ok => Project is not null && Errors.Count == 0;

    public static ProjectLoadResult Fail(params string[] errors) => new(null, errors);
}

/// <summary>
/// Desktop project storage: one project folder per project in <c>&lt;root&gt;/projects/&lt;id&gt;/</c>
/// (M8: <c>project.json</c>, a file per scene, NPC, dialogue and item, and the art and sounds as
/// files; see <see cref="ProjectFolder"/>) plus <c>index.json</c> (id, name, updated time). A
/// project saved before M8 as one <c>&lt;id&gt;.json</c> file still opens, and its next save moves
/// it into a folder. Every write is atomic and flushed to disk (<see cref="AtomicFile"/>), a save
/// writes only the files that changed, and the version it replaces is kept as single-file JSON in
/// <c>backups/</c>; every load goes through the F# project loader, which reads the current schema
/// version only.
/// </summary>
/// <remarks>
/// The FILE is the source of truth for a project's id: <see cref="Load"/> hands back the project
/// under the id its file name stands for, whatever id the JSON inside carries (a hand-made copy
/// repeats its original's id), so saving it writes the file it came from.
/// </remarks>
public sealed class ProjectStore
{
    private const string IndexFileName = "index.json";

    private static readonly JsonSerializerOptions IndexOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        WriteIndented = true,
    };

    private readonly TimeProvider _time;
    private readonly Lock _gate = new();

    public ProjectStore(string rootDirectory, TimeProvider? time = null)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(rootDirectory);
        RootDirectory = rootDirectory;
        _time = time ?? TimeProvider.System;
    }

    public string RootDirectory { get; }

    public string ProjectsDirectory => Path.Combine(RootDirectory, "projects");

    public string IndexPath => Path.Combine(ProjectsDirectory, IndexFileName);

    /// <summary>Previous versions of project files.</summary>
    public string BackupsDirectory => Path.Combine(ProjectsDirectory, "backups");

    /// <summary>The folder that holds project <paramref name="id"/> (<see cref="SafeFileName"/>).</summary>
    public string FolderFor(string id) => Path.Combine(ProjectsDirectory, SafeFileName(id));

    /// <summary>The project file of project <paramref name="id"/>'s folder (<c>project.json</c>).</summary>
    public string PathFor(string id) => Path.Combine(FolderFor(id), ProjectFolder.MainFile);

    /// <summary>The single file a project saved before M8 lives in; its next save moves it into its folder.</summary>
    public string LegacyPathFor(string id) => Path.Combine(ProjectsDirectory, SafeFileName(id) + ".json");

    /// <summary>The version of project <paramref name="id"/> before its last save.</summary>
    public string PreviousVersionPath(string id) => Path.Combine(BackupsDirectory, SafeFileName(id) + ".previous.json");

    public bool Exists(string id) => File.Exists(PathFor(id)) || File.Exists(LegacyPathFor(id));

    /// <summary>The hidden lock file of project <paramref name="id"/> (<see cref="ProjectLock"/>).</summary>
    public string LockPathFor(string id) => Path.Combine(ProjectsDirectory, "." + SafeFileName(id) + ".json.lock");

    /// <summary>
    /// The project's <see cref="FileStamp"/> (for a folder: its project files' latest write and
    /// their total size), or null when it is missing or unreadable.
    /// </summary>
    public FileStamp? Stamp(string id)
    {
        try
        {
            if (File.Exists(PathFor(id)))
            {
                var files = ProjectFiles(FolderFor(id)).Select(file => new FileInfo(file.FullPath)).ToList();
                return new FileStamp(files.Max(info => info.LastWriteTimeUtc), files.Sum(info => info.Length));
            }

            var info = new FileInfo(LegacyPathFor(id));
            return info.Exists ? new FileStamp(info.LastWriteTimeUtc, info.Length) : null;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return null;
        }
    }

    /// <summary>The size of project <paramref name="id"/> on disk: its folder's project files, or its single file.</summary>
    private long SizeOf(string id)
    {
        try
        {
            return File.Exists(PathFor(id))
                ? ProjectFiles(FolderFor(id)).Sum(file => new FileInfo(file.FullPath).Length)
                : new FileInfo(LegacyPathFor(id)).Length;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return 0;
        }
    }

    /// <summary>
    /// The files of a project folder that belong to the project (<see cref="ProjectFolder.isProjectFile"/>),
    /// with their paths relative to it (<c>/</c>-separated).
    /// </summary>
    private static IEnumerable<(string Relative, string FullPath)> ProjectFiles(string folder)
    {
        if (!Directory.Exists(folder))
        {
            yield break;
        }

        foreach (var file in Directory.EnumerateFiles(folder, "*", SearchOption.AllDirectories))
        {
            var relative = Path.GetRelativePath(folder, file).Replace(Path.DirectorySeparatorChar, '/');
            if (!Path.GetFileName(file).StartsWith('.') && ProjectFolder.isProjectFile(relative))
            {
                yield return (relative, file);
            }
        }
    }

    /// <summary>
    /// The project folder <paramref name="folder"/> (one copied from elsewhere, or a team's
    /// checkout) as single-file project JSON for <see cref="Import"/>, or what is wrong with it.
    /// </summary>
    public static (string? Json, IReadOnlyList<string> Errors) ReadFolderAsJson(string folder)
    {
        try
        {
            var joined = ProjectFolder.joinText(ReadFolder(folder));
            return joined.IsOk ? (joined.ResultValue, []) : (null, [.. joined.ErrorValue]);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return (null, [$"Could not read the project folder: {ex.Message}"]);
        }
    }

    /// <summary>A project folder's files, read (for <see cref="ProjectFolder.load"/>).</summary>
    private static Dictionary<string, byte[]> ReadFolder(string folder) =>
        ProjectFiles(folder).ToDictionary(file => file.Relative, file => File.ReadAllBytes(file.FullPath), StringComparer.Ordinal);

    /// <summary>All stored projects, most recently updated first.</summary>
    public IReadOnlyList<ProjectSummary> List()
    {
        lock (_gate)
        {
            var index = ReadIndex();
            var result = new List<ProjectSummary>();
            foreach (var entry in index.Projects)
            {
                if (!Exists(entry.Id))
                {
                    continue;
                }

                result.Add(new ProjectSummary(entry.Id, entry.Name, DateTimeOffset.FromUnixTimeMilliseconds(entry.UpdatedAt), SizeOf(entry.Id)));
            }

            return result.OrderByDescending(p => p.UpdatedAt).ThenBy(p => p.Name, StringComparer.OrdinalIgnoreCase).ToList();
        }
    }

    /// <summary>
    /// Loads project <paramref name="id"/>, under that id whatever the file's JSON
    /// says (see the remarks on <see cref="ProjectStore"/>). Never throws.
    /// </summary>
    public ProjectLoadResult Load(string id)
    {
        ProjectLoadResult loaded;
        try
        {
            loaded = File.Exists(PathFor(id)) ? ParseFolder(ReadFolder(FolderFor(id))) : Parse(File.ReadAllText(LegacyPathFor(id)));
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return ProjectLoadResult.Fail($"Could not read the project file: {ex.Message}");
        }

        if (loaded.Project is { } project && project.Id != id)
        {
            loaded = loaded with { Project = project.WithId(id) };
        }

        return loaded;
    }

    /// <summary>
    /// Writes the project folder (each changed file atomically, <c>project.json</c> after the
    /// files it names; files the project no longer has go), keeps the version it replaces in
    /// <see cref="PreviousVersionPath"/> and refreshes the index entry. A project still in its
    /// single pre-M8 file moves into its folder.
    /// </summary>
    public void Save(GameProject project)
    {
        ArgumentNullException.ThrowIfNull(project);
        ArgumentException.ThrowIfNullOrWhiteSpace(project.Id);
        lock (_gate)
        {
            WriteFolder(project);
            var index = ReadIndex();
            var entry = new IndexEntry { Id = project.Id, Name = DisplayName(project), UpdatedAt = _time.GetUtcNow().ToUnixTimeMilliseconds() };
            var projects = index.Projects.Where(p => p.Id != project.Id).Append(entry).ToList();
            WriteIndex(new IndexFile { Projects = projects });
        }
    }

    private void WriteFolder(GameProject project)
    {
        var id = project.Id;
        var folder = FolderFor(id);
        var legacy = LegacyPathFor(id);
        var existing = ProjectFiles(folder).ToDictionary(file => file.Relative, file => file.FullPath, StringComparer.Ordinal);
        var files = ProjectFolder.toFiles(project).Select(file => (Path: file.Item1, Bytes: file.Item2)).ToList();
        var changed = files.Where(file => !existing.TryGetValue(file.Path, out var path) || !SameBytes(path, file.Bytes)).ToList();
        var stale = existing.Keys.Except(files.Select(file => file.Path), StringComparer.Ordinal).ToList();
        if (changed.Count == 0 && stale.Count == 0 && !File.Exists(legacy))
        {
            return;
        }

        // The version this save replaces, as one file a creator can import.
        var previous = existing.Count > 0 ? ProjectFolder.singleFileText(ReadFolder(folder)) : File.Exists(legacy) ? File.ReadAllText(legacy) : "";
        if (previous.Length > 0)
        {
            AtomicFile.WriteAllText(PreviousVersionPath(id), previous);
        }

        // project.json last: until it is written, the folder still reads as the old project.
        foreach (var (path, bytes) in changed.OrderBy(file => file.Path == ProjectFolder.MainFile))
        {
            AtomicFile.WriteAllBytes(Path.Combine(folder, path.Replace('/', Path.DirectorySeparatorChar)), bytes);
        }

        foreach (var path in stale)
        {
            File.Delete(existing[path]);
        }

        if (File.Exists(legacy))
        {
            File.Delete(legacy);
        }
    }

    private static bool SameBytes(string path, byte[] bytes)
    {
        var info = new FileInfo(path);
        return info.Exists && info.Length == bytes.Length && File.ReadAllBytes(path).AsSpan().SequenceEqual(bytes);
    }

    /// <summary>
    /// Deletes project <paramref name="id"/>. Throws when the file can't be deleted (read-only,
    /// locked by another program) or the project is open in another editor window.
    /// </summary>
    public void Delete(string id)
    {
        if (ProjectLock.IsHeldElsewhere(this, id))
        {
            throw new IOException("It is open in another Tractr window.");
        }

        lock (_gate)
        {
            if (Directory.Exists(FolderFor(id)))
            {
                Directory.Delete(FolderFor(id), recursive: true);
            }

            if (File.Exists(LegacyPathFor(id)))
            {
                File.Delete(LegacyPathFor(id));
            }

            if (File.Exists(PreviousVersionPath(id)))
            {
                File.Delete(PreviousVersionPath(id));
            }

            var index = ReadIndex();
            WriteIndex(new IndexFile { Projects = index.Projects.Where(p => p.Id != id).ToList() });
        }
    }

    /// <summary>
    /// Project list "Rename" (web <c>renameProject</c>): rewrites the stored project with the new
    /// name. Blank names are refused. Never throws.
    /// </summary>
    public ProjectLoadResult Rename(string id, string name)
    {
        if (string.IsNullOrWhiteSpace(name))
        {
            return ProjectLoadResult.Fail("A project needs a name.");
        }

        var loaded = Load(id);
        if (!loaded.Ok)
        {
            return loaded;
        }

        var renamed = ProjectList.Rename(loaded.Project!, name);
        return TrySave(renamed);
    }

    /// <summary>
    /// Project list "Duplicate" (web <c>duplicateProject</c>): a copy under a fresh id, named
    /// "Name (copy)" unless <paramref name="name"/> is given. Never throws.
    /// </summary>
    public ProjectLoadResult Duplicate(string id, string? name = null)
    {
        var loaded = Load(id);
        if (!loaded.Ok)
        {
            return loaded;
        }

        return TrySave(ProjectList.Duplicate(loaded.Project!, NewId(), name));
    }

    private ProjectLoadResult TrySave(GameProject project)
    {
        try
        {
            Save(project);
            return new ProjectLoadResult(project, []);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return ProjectLoadResult.Fail($"Could not write the project file: {ex.Message}");
        }
    }

    /// <summary>A fresh unique project id (<c>proj-&lt;base36 time&gt;</c>).</summary>
    public string NewId()
    {
        var now = (double)_time.GetUtcNow().ToUnixTimeMilliseconds();
        var id = ProjectCatalog.NewProjectId(now);
        while (Exists(id))
        {
            now++;
            id = ProjectCatalog.NewProjectId(now);
        }

        return id;
    }

    /// <summary>Project JSON (indented, camelCase).</summary>
    public static string ToJson(GameProject project) => ProjectLoad.toText(project);

    /// <summary>Parses and validates stored project JSON. Never throws.</summary>
    public static ProjectLoadResult Parse(string json) => FromLoad(ProjectJson.loadProjectText(json));

    /// <summary>Joins and validates a project folder's files (relative <c>/</c>-separated paths to bytes). Never throws.</summary>
    public static ProjectLoadResult ParseFolder(IReadOnlyDictionary<string, byte[]> files) => FromLoad(ProjectFolder.load(files));

    private static ProjectLoadResult FromLoad(LoadResult<GameProject> result) =>
        result.Ok && result.Data.OrNull() is { } data
            ? new ProjectLoadResult(data, [])
            : ProjectLoadResult.Fail(result.Errors.Length > 0 ? [.. result.Errors] : ["Invalid project data"]);

    /// <summary>
    /// Imports project JSON (Export Project JSON's file) as a NEW project: a new id, and
    /// editor-only state starts fresh. The caller saves the result.
    /// </summary>
    public ProjectLoadResult Import(string json)
    {
        JsonNode? node;
        try
        {
            node = JsonNode.Parse(json, documentOptions: new JsonDocumentOptions { CommentHandling = JsonCommentHandling.Skip, AllowTrailingCommas = true });
        }
        catch (JsonException ex)
        {
            return ProjectLoadResult.Fail($"The file is not valid JSON: {ex.Message}");
        }

        if (node is not JsonObject)
        {
            return ProjectLoadResult.Fail("The file does not contain a project (expected a JSON object).");
        }

        var loaded = ProjectJson.loadProject(node);
        if (!loaded.Ok || loaded.Data.OrNull() is not { } project)
        {
            return ProjectLoadResult.Fail(loaded.Errors.Length > 0 ? [.. loaded.Errors] : ["Invalid project data"]);
        }

        var name = string.IsNullOrWhiteSpace(project.Name) ? "Imported Game" : project.Name.Trim();
        project = project.WithId(NewId()).WithName(name).WithMode("tiles").WithSelectedTileType("grass").WithSelectedNpcId(null).WithSelectedItemId(null).WithEventFlags(FSharpList<Tuple<string, FarmEngine.Authoring.Json>>.Empty);
        return new ProjectLoadResult(project, []);
    }

    private static string DisplayName(GameProject project) => string.IsNullOrWhiteSpace(project.Name) ? "Untitled Game" : project.Name;

    /// <summary>
    /// Deletes temp files a killed editor left in the projects folder (<see cref="AtomicFile.DeleteStaleTempFiles"/>).
    /// Never throws.
    /// </summary>
    public int DeleteStaleTempFiles() => AtomicFile.DeleteStaleTempFiles(ProjectsDirectory, TimeSpan.FromMinutes(10), _time, recursive: true);

    /// <summary>
    /// Device names Windows refuses as a file name, with or without an extension (and after
    /// trailing dots and spaces are dropped).
    /// </summary>
    private static readonly HashSet<string> WindowsReservedNames = new(StringComparer.OrdinalIgnoreCase)
    {
        "CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$",
        "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "COM\u00B9", "COM\u00B2", "COM\u00B3",
        "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "LPT\u00B9", "LPT\u00B2", "LPT\u00B3",
        // Not Windows's, the store's: a project folder may not take the backups folder's name.
        "BACKUPS",
    };

    /// <summary>
    /// The file stem for a project id, the same on every platform and one-to-one (two ids never
    /// share a file): characters Windows refuses in a file name, <c>%</c> itself, a leading dot
    /// and trailing dots or spaces are escaped as <c>%XX</c> (UTF-8 bytes), and so is the first
    /// letter of a Windows device name (<c>con</c> becomes <c>%63on</c>). Ordinary ids
    /// (<c>proj-abc</c>, <c>project-1</c>) are their own file names.
    /// </summary>
    internal static string SafeFileName(string id)
    {
        ArgumentNullException.ThrowIfNull(id);
        if (id.Length == 0)
        {
            return "%";
        }

        var reserved = WindowsReservedNames.Contains(id.Split('.')[0].TrimEnd(' ', '.'));
        var builder = new System.Text.StringBuilder(id.Length);
        for (var i = 0; i < id.Length; i++)
        {
            var c = id[i];
            var escape = c < 32 || c is '<' or '>' or ':' or '"' or '/' or '\\' or '|' or '?' or '*' or '%'
                || (i == 0 && (c == '.' || reserved))
                || (c is '.' or ' ' && id.AsSpan(i).TrimEnd(". ").IsEmpty);
            if (!escape)
            {
                builder.Append(c);
                continue;
            }

            // A surrogate pair is escaped as one character (its four UTF-8 bytes).
            var length = char.IsHighSurrogate(c) && i + 1 < id.Length && char.IsLowSurrogate(id[i + 1]) ? 2 : 1;
            foreach (var b in System.Text.Encoding.UTF8.GetBytes(id.Substring(i, length)))
            {
                builder.Append('%').Append(b.ToString("X2", System.Globalization.CultureInfo.InvariantCulture));
            }

            i += length - 1;
        }

        return builder.ToString();
    }

    /// <summary>
    /// The id whose <see cref="SafeFileName"/> is <paramref name="stem"/>, or null when no id maps
    /// to it (a hand-made file name such as <c>100% farm</c> or <c>con</c>).
    /// </summary>
    internal static string? IdForFileName(string stem)
    {
        ArgumentNullException.ThrowIfNull(stem);
        var bytes = new List<byte>();
        var text = new System.Text.StringBuilder(stem.Length);
        for (var i = 0; i < stem.Length; i++)
        {
            if (stem[i] == '%' && i + 2 < stem.Length
                && byte.TryParse(stem.AsSpan(i + 1, 2), System.Globalization.NumberStyles.AllowHexSpecifier, System.Globalization.CultureInfo.InvariantCulture, out var b))
            {
                bytes.Add(b);
                i += 2;
                continue;
            }

            DecodeBytes();
            text.Append(stem[i]);
        }

        DecodeBytes();
        var id = text.ToString();
        return id.Length > 0 && SafeFileName(id) == stem ? id : null;

        void DecodeBytes()
        {
            if (bytes.Count > 0)
            {
                text.Append(System.Text.Encoding.UTF8.GetString([.. bytes]));
                bytes.Clear();
            }
        }
    }

    private IndexFile ReadIndex()
    {
        IndexFile? index = null;
        try
        {
            if (File.Exists(IndexPath))
            {
                index = JsonSerializer.Deserialize<IndexFile>(File.ReadAllText(IndexPath), IndexOptions);
            }
        }
        catch (Exception ex) when (ex is IOException or JsonException or UnauthorizedAccessException)
        {
            index = null;
        }

        index ??= new IndexFile();

        // Adopt project files and folders the index doesn't know (copied in by hand, or a lost index).
        if (Directory.Exists(ProjectsDirectory))
        {
            var known = index.Projects.Select(p => p.Id).ToHashSet(StringComparer.Ordinal);
            var adopted = new List<IndexEntry>();
            foreach (var folder in Directory.EnumerateDirectories(ProjectsDirectory))
            {
                var name = Path.GetFileName(folder);
                var main = Path.Combine(folder, ProjectFolder.MainFile);
                if (name.StartsWith('.') || name == "backups" || !File.Exists(main) || IdForFileName(name) is not { } folderId)
                {
                    continue;
                }

                if (known.Add(folderId))
                {
                    adopted.Add(new IndexEntry { Id = folderId, Name = PeekName(main) ?? folderId, UpdatedAt = new DateTimeOffset(File.GetLastWriteTimeUtc(main)).ToUnixTimeMilliseconds() });
                }
            }

            foreach (var file in Directory.EnumerateFiles(ProjectsDirectory, "*.json"))
            {
                var fileName = Path.GetFileName(file);
                if (fileName == IndexFileName || fileName.StartsWith('.'))
                {
                    continue;
                }

                if (index.Projects.Any(p => LegacyPathFor(p.Id) == file))
                {
                    continue;
                }

                var stem = Path.GetFileNameWithoutExtension(file);
                var path = file;
                if (IdForFileName(stem) is not { } id)
                {
                    // A name no id maps to ("100% farm.json"): the file takes its id's name.
                    id = stem;
                    path = LegacyPathFor(id);
                    if (!TryRename(file, path))
                    {
                        continue;
                    }
                }

                if (!known.Add(id))
                {
                    continue;
                }

                adopted.Add(new IndexEntry { Id = id, Name = PeekName(path) ?? id, UpdatedAt = new DateTimeOffset(File.GetLastWriteTimeUtc(path)).ToUnixTimeMilliseconds() });
            }

            if (adopted.Count > 0)
            {
                index = new IndexFile { Projects = [.. index.Projects, .. adopted] };
            }
        }

        return index;
    }

    private void WriteIndex(IndexFile index) => AtomicFile.WriteAllText(IndexPath, JsonSerializer.Serialize(index, IndexOptions));

    private static bool TryRename(string from, string to)
    {
        try
        {
            if (File.Exists(to))
            {
                return false;
            }

            File.Move(from, to);
            return true;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            System.Diagnostics.Trace.TraceWarning($"Could not adopt the project file {from}: {ex.Message}");
            return false;
        }
    }

    private static string? PeekName(string file)
    {
        try
        {
            using var stream = File.OpenRead(file);
            using var document = JsonDocument.Parse(stream);
            return document.RootElement.TryGetProperty("name", out var name) && name.ValueKind == JsonValueKind.String ? name.GetString() : null;
        }
        catch (Exception ex) when (ex is IOException or JsonException or UnauthorizedAccessException)
        {
            return null;
        }
    }

    private sealed record IndexFile
    {
        public List<IndexEntry> Projects { get; init; } = [];
    }

    private sealed record IndexEntry
    {
        public string Id { get; init; } = "";

        public string Name { get; init; } = "";

        public long UpdatedAt { get; init; }
    }
}
