using System.Text.Json.Nodes;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using Tractr.App.Projects;

namespace Tractr.App.Tests.Game;

public sealed class ProjectStoreTests
{
    private static readonly string FixturePath = Path.Combine(AppContext.BaseDirectory, "Fixtures", "fixture-project.json");

    [Fact]
    public void SaveThenLoad_RoundTripsTheProject_AndIndexesIt()
    {
        using var dir = new TempDir();
        var time = new TestTime(new DateTimeOffset(2026, 9, 23, 12, 0, 0, TimeSpan.Zero));
        var store = new ProjectStore(dir.Path, time);
        var project = FarmEngine.Authoring.ProjectCatalog.CreateNewProject(ProjectTemplates.Starter, "Sunny Acres", "proj-test", 0);

        store.Save(project);

        // A project is a folder (M8): its project file, a file per scene, and so on.
        Assert.True(File.Exists(Path.Combine(dir.Path, "projects", "proj-test", "project.json")));
        Assert.True(File.Exists(Path.Combine(dir.Path, "projects", "proj-test", "scenes", "scene-farm.json")));
        var loaded = store.Load("proj-test");
        Assert.True(loaded.Ok, string.Join("; ", loaded.Errors));
        Assert.Equal(ProjectStore.ToJson(project), ProjectStore.ToJson(loaded.Project!));

        var summary = Assert.Single(store.List());
        Assert.Equal(("proj-test", "Sunny Acres"), (summary.Id, summary.Name));
        Assert.Equal(time.Now, summary.UpdatedAt);
        Assert.True(summary.SizeBytes > 1000);
    }

    [Fact]
    public void RenameAndDuplicate_RewriteTheStoreAndIndex()
    {
        using var dir = new TempDir();
        var time = new TestTime(new DateTimeOffset(2026, 9, 23, 12, 0, 0, TimeSpan.Zero));
        var store = new ProjectStore(dir.Path, time);
        store.Save(FarmEngine.Authoring.ProjectCatalog.CreateNewProject(ProjectTemplates.Blank, "First", "proj-a", 0));

        var renamed = store.Rename("proj-a", " Meadow ");
        Assert.True(renamed.Ok, string.Join("; ", renamed.Errors));
        Assert.Equal("Meadow", store.Load("proj-a").Project!.Name);
        Assert.Equal("Meadow", Assert.Single(store.List()).Name);
        Assert.False(store.Rename("proj-a", "  ").Ok);
        Assert.False(store.Rename("proj-missing", "Name").Ok);

        var copy = store.Duplicate("proj-a");
        Assert.True(copy.Ok, string.Join("; ", copy.Errors));
        Assert.NotEqual("proj-a", copy.Project!.Id);
        Assert.StartsWith("proj-", copy.Project.Id, StringComparison.Ordinal);
        Assert.Equal("Meadow (copy)", store.Load(copy.Project.Id).Project!.Name);
        Assert.Equal(2, store.List().Count);
        Assert.False(store.Duplicate("proj-missing").Ok);
    }

    [Fact]
    public void Save_IsAtomic_ReplacesTheFileAndLeavesNoTempFiles()
    {
        using var dir = new TempDir();
        var store = new ProjectStore(dir.Path);
        var project = FarmEngine.Authoring.ProjectCatalog.CreateNewProject(ProjectTemplates.Blank, "First", "proj-a", 0);
        store.Save(project);
        store.Save(project.WithName("Second"));

        var files = Directory.GetFiles(Path.Combine(dir.Path, "projects"));
        Assert.Equal(["index.json"], files.Select(Path.GetFileName).Order(StringComparer.Ordinal));
        Assert.Empty(Directory.GetFiles(Path.Combine(dir.Path, "projects"), "*.tmp", SearchOption.AllDirectories));
        Assert.Equal("Second", store.Load("proj-a").Project!.Name);
        Assert.Equal("Second", Assert.Single(store.List()).Name);
        // Project JSON: camelCase, indented, at the current schema version.
        var text = File.ReadAllText(Path.Combine(dir.Path, "projects", "proj-a", "project.json"));
        Assert.Contains($"\n  \"schemaVersion\": {ProjectSchema.CurrentProjectSchemaVersion}", text, StringComparison.Ordinal);
    }

    [Fact]
    public void Load_ReadsTheFixtureProject()
    {
        using var dir = new TempDir();
        var store = new ProjectStore(dir.Path);
        Directory.CreateDirectory(store.ProjectsDirectory);
        File.Copy(FixturePath, store.LegacyPathFor("fixture"));

        // Files the index doesn't know are adopted (name read from the file).
        var summary = Assert.Single(store.List());
        Assert.Equal("fixture", summary.Id);

        var loaded = store.Load("fixture");
        Assert.True(loaded.Ok, string.Join("; ", loaded.Errors));
        Assert.Equal("fixture", loaded.Project!.Id);
        Assert.Equal(ProjectSchema.ClassicProjectSchemaVersion, loaded.Project.SchemaVersion);
        Assert.NotEmpty(loaded.Project.Scenes);
    }

    [Fact]
    public void Load_ReportsErrorsForBrokenFiles()
    {
        using var dir = new TempDir();
        var store = new ProjectStore(dir.Path);
        Directory.CreateDirectory(store.ProjectsDirectory);
        File.WriteAllText(store.LegacyPathFor("broken"), "{ not json");
        File.WriteAllText(store.LegacyPathFor("future"), """{ "schemaVersion": 999, "scenes": [] }""");
        File.WriteAllText(store.LegacyPathFor("old"), """{ "schemaVersion": 0, "scenes": [] }""");

        var broken = store.Load("broken");
        Assert.False(broken.Ok);
        Assert.Contains("not valid JSON", Assert.Single(broken.Errors), StringComparison.Ordinal);
        var future = store.Load("future");
        Assert.False(future.Ok);
        Assert.Contains("newer than this engine supports", future.Errors[0], StringComparison.Ordinal);
        var old = store.Load("old");
        Assert.False(old.Ok);
        Assert.Contains("schema version 0 is not supported", old.Errors[0], StringComparison.Ordinal);
        Assert.False(store.Load("missing").Ok);
    }

    [Fact]
    public void Import_CreatesANewProjectFromProjectJson()
    {
        using var dir = new TempDir();
        var store = new ProjectStore(dir.Path);

        var fromFixture = store.Import(File.ReadAllText(FixturePath));
        Assert.True(fromFixture.Ok, string.Join("; ", fromFixture.Errors));
        Assert.StartsWith("proj-", fromFixture.Project!.Id, StringComparison.Ordinal);
        Assert.Equal("tiles", fromFixture.Project.Mode);

        // Only whole projects import: a file without the editor fields is refused.
        var partial = JsonNode.Parse(ProjectStore.ToJson(FarmEngine.Authoring.ProjectCatalog.CreateInitialProject(0)))!.AsObject();
        partial.Remove("player");
        Assert.False(store.Import(partial.ToJsonString()).Ok);

        Assert.False(store.Import("[1, 2]").Ok);
        Assert.False(store.Import("nope").Ok);
    }

    [Fact]
    public void Delete_RemovesFileAndIndexEntry()
    {
        using var dir = new TempDir();
        var store = new ProjectStore(dir.Path);
        store.Save(FarmEngine.Authoring.ProjectCatalog.CreateNewProject(ProjectTemplates.Blank, "A", "proj-a", 0));
        store.Save(FarmEngine.Authoring.ProjectCatalog.CreateNewProject(ProjectTemplates.Blank, "B", "proj-b", 0));

        store.Delete("proj-a");

        Assert.False(store.Exists("proj-a"));
        Assert.Equal(["proj-b"], store.List().Select(p => p.Id));
    }

    [Fact]
    public void AppSettings_PreserveOtherSections()
    {
        using var dir = new TempDir();
        var path = Path.Combine(dir.Path, "settings.json");
        File.WriteAllText(path, """{ "updates": { "channel": "beta" } }""");
        var settings = new AppSettingsStore(path);

        settings.Save(new WorkspaceSettings { LastProjectId = "proj-x" });

        Assert.Equal("proj-x", settings.Load().LastProjectId);
        var root = JsonNode.Parse(File.ReadAllText(path))!;
        Assert.Equal("beta", root["updates"]!["channel"]!.GetValue<string>());
    }

    [Fact]
    public void AppSettingsStore_DoesNotRewriteTheUpdateCentersSection()
    {
        using var dir = new TempDir();
        var path = Path.Combine(dir.Path, "settings.json");
        var updates = new Tractr.Updates.JsonSettingsStore(path);
        updates.Save(new Tractr.Updates.UpdateSettings
        {
            Channel = Tractr.Updates.UpdateChannel.Prerelease,
            LastChecked = new DateTimeOffset(2026, 9, 27, 8, 30, 0, TimeSpan.FromHours(2)),
        });
        new AppSettingsStore(path).Save(new WorkspaceSettings { LastProjectId = "proj-x" });
        var afterWorkspace = File.ReadAllText(path);

        // Both stores serialize the whole file the same way: saving the updates section again
        // leaves the file byte-identical, and the timestamp keeps its readable "+02:00".
        updates.Save(updates.Load());
        Assert.Equal(afterWorkspace, File.ReadAllText(path));
        Assert.Contains("+02:00", afterWorkspace, StringComparison.Ordinal);
        Assert.Contains("\"prerelease\"", afterWorkspace, StringComparison.Ordinal);
    }

    [Theory]
    [InlineData("con", "%63on")]
    [InlineData("NUL", "%4EUL")]
    [InlineData("com1.backup", "%63om1.backup")]
    [InlineData("COM0", "%43OM0")]
    [InlineData("lpt¹", "%6Cpt¹")]
    [InlineData("CONOUT$", "%43ONOUT$")]
    [InlineData("a/b:c", "a%2Fb%3Ac")]
    [InlineData("a_b", "a_b")]
    [InlineData("100%", "100%25")]
    [InlineData("..", "%2E%2E")]
    [InlineData(".hidden", "%2Ehidden")]
    [InlineData("trailing. ", "trailing%2E%20")]
    [InlineData("project-1", "project-1")]
    [InlineData("proj-abc - Copy", "proj-abc - Copy")]
    public void SafeFileName_AvoidsReservedAndInvalidNames(string id, string expected)
    {
        Assert.Equal(expected, ProjectStore.SafeFileName(id));
        Assert.Equal(id, ProjectStore.IdForFileName(expected));
    }

    [Fact]
    public void SafeFileName_GivesDifferentIdsDifferentFiles()
    {
        string[] ids = ["a:b", "a_b", "a%3Ab", "con", "con_", "%63on", "x.", "x", "x%2E"];
        Assert.Equal(ids.Length, ids.Select(ProjectStore.SafeFileName).Distinct(StringComparer.Ordinal).Count());
        Assert.Null(ProjectStore.IdForFileName("100% farm"));
        Assert.Null(ProjectStore.IdForFileName("con"));
    }

    private static string WebExport(string name) =>
        ProjectStore.ToJson(FarmEngine.Authoring.ProjectCatalog.CreateNewProject(ProjectTemplates.Blank, name, "project-1", 0));

    [Fact]
    public void HandCopiedFiles_ThatShareAnId_EachSaveToTheirOwnFile()
    {
        using var dir = new TempDir();
        var store = new ProjectStore(dir.Path);
        Directory.CreateDirectory(store.ProjectsDirectory);
        File.WriteAllText(Path.Combine(store.ProjectsDirectory, "farmA.json"), WebExport("Farm A"));
        File.WriteAllText(Path.Combine(store.ProjectsDirectory, "farmB.json"), WebExport("Farm B"));

        Assert.Equal(["farmA", "farmB"], store.List().Select(p => p.Id).Order(StringComparer.Ordinal));
        var a = store.Load("farmA").Project!;
        var b = store.Load("farmB").Project!;
        Assert.Equal(("farmA", "farmB"), (a.Id, b.Id));

        store.Save(a.WithName("Farm A edited"));
        store.Save(b.WithName("Farm B edited"));

        Assert.False(store.Exists("project-1"));
        Assert.Equal("Farm A edited", store.Load("farmA").Project!.Name);
        Assert.Equal("Farm B edited", store.Load("farmB").Project!.Name);
        Assert.Equal(2, store.List().Count);
    }

    [Fact]
    public void AnExplorerCopy_SavesToTheCopy_NotTheOriginal()
    {
        using var dir = new TempDir();
        var store = new ProjectStore(dir.Path);
        store.Save(FarmEngine.Authoring.ProjectCatalog.CreateNewProject(ProjectTemplates.Blank, "Original", "proj-abc", 0));
        CopyFolder(store.FolderFor("proj-abc"), Path.Combine(store.ProjectsDirectory, "proj-abc - Copy"));

        var copy = store.Load("proj-abc - Copy");
        Assert.True(copy.Ok, string.Join("; ", copy.Errors));
        Assert.Equal("proj-abc - Copy", copy.Project!.Id);
        store.Save(copy.Project.WithName("Edited copy"));

        Assert.Equal("Original", store.Load("proj-abc").Project!.Name);
        Assert.Equal("Edited copy", store.Load("proj-abc - Copy").Project!.Name);
        Assert.True(File.Exists(Path.Combine(store.ProjectsDirectory, "proj-abc - Copy", "project.json")));
    }

    [Fact]
    public void AHandMadeFileName_NoIdMapsTo_IsRenamedWhenAdopted()
    {
        using var dir = new TempDir();
        var store = new ProjectStore(dir.Path);
        Directory.CreateDirectory(store.ProjectsDirectory);
        File.WriteAllText(Path.Combine(store.ProjectsDirectory, "100% farm.json"), WebExport("Full Farm"));

        var summary = Assert.Single(store.List());
        Assert.Equal(("100% farm", "Full Farm"), (summary.Id, summary.Name));
        Assert.True(File.Exists(store.LegacyPathFor("100% farm")));
        Assert.Equal("Full Farm", store.Load("100% farm").Project!.Name);
    }

    [Fact]
    public void Save_KeepsThePreviousVersion()
    {
        using var dir = new TempDir();
        var store = new ProjectStore(dir.Path);
        Directory.CreateDirectory(store.ProjectsDirectory);
        File.Copy(FixturePath, store.LegacyPathFor("fixture"));

        var loaded = store.Load("fixture");
        store.Save(loaded.Project!);
        store.Save(loaded.Project!.WithName("Renamed"));

        Assert.Equal(loaded.Project!.Name, ProjectStore.Parse(File.ReadAllText(store.PreviousVersionPath("fixture"))).Project!.Name);
        Assert.Equal("Renamed", store.Load("fixture").Project!.Name);
        // Backups never show up as projects.
        Assert.Single(store.List());
    }

    private static void CopyFolder(string from, string to)
    {
        foreach (var file in Directory.GetFiles(from, "*", SearchOption.AllDirectories))
        {
            var target = Path.Combine(to, Path.GetRelativePath(from, file));
            Directory.CreateDirectory(Path.GetDirectoryName(target)!);
            File.Copy(file, target);
        }
    }

    [Fact]
    public void ASingleFileProject_OpensAndMovesIntoAFolderOnItsNextSave()
    {
        using var dir = new TempDir();
        var store = new ProjectStore(dir.Path);
        Directory.CreateDirectory(store.ProjectsDirectory);
        File.Copy(FixturePath, store.LegacyPathFor("fixture"));
        var loaded = store.Load("fixture");
        Assert.True(loaded.Ok, string.Join("; ", loaded.Errors));

        store.Save(loaded.Project!);

        Assert.False(File.Exists(store.LegacyPathFor("fixture")));
        Assert.True(File.Exists(store.PathFor("fixture")));
        Assert.Equal(ProjectStore.ToJson(loaded.Project!), ProjectStore.ToJson(store.Load("fixture").Project!));
        Assert.Single(store.List());
    }

    [Fact]
    public void Save_WritesOnlyWhatChanged_AndDropsWhatTheProjectNoLongerHas()
    {
        using var dir = new TempDir();
        var store = new ProjectStore(dir.Path);
        var project = FarmEngine.Authoring.ProjectCatalog.CreateNewProject(ProjectTemplates.Starter, "Farm", "proj-a", 0);
        store.Save(project);
        var folder = store.FolderFor("proj-a");
        var farm = Path.Combine(folder, "scenes", "scene-farm.json");
        var item = Directory.GetFiles(Path.Combine(folder, "items")).First();
        File.SetLastWriteTimeUtc(farm, new DateTime(2020, 1, 1, 0, 0, 0, DateTimeKind.Utc));
        // A file of the creator's own (a README, a .git folder) is left alone.
        File.WriteAllText(Path.Combine(folder, "README.md"), "notes");

        var itemId = Path.GetFileNameWithoutExtension(item);
        var fewer = project.WithItems(project.Items.Where(i => i.Id != itemId)).WithName("Renamed");
        store.Save(fewer);

        Assert.Equal(new DateTime(2020, 1, 1, 0, 0, 0, DateTimeKind.Utc), File.GetLastWriteTimeUtc(farm));
        Assert.False(File.Exists(item));
        Assert.True(File.Exists(Path.Combine(folder, "README.md")));
        var reloaded = store.Load("proj-a").Project!;
        Assert.Equal("Renamed", reloaded.Name);
        Assert.DoesNotContain(reloaded.Items, i => i.Id == itemId);
        // What it replaced is kept as one file a creator can import.
        Assert.Equal("Farm", ProjectStore.Parse(File.ReadAllText(store.PreviousVersionPath("proj-a"))).Project!.Name);
    }

    [Fact]
    public void AProjectFolderCopiedIn_IsAdopted()
    {
        using var dir = new TempDir();
        var source = new ProjectStore(Path.Combine(dir.Path, "elsewhere"));
        source.Save(FarmEngine.Authoring.ProjectCatalog.CreateNewProject(ProjectTemplates.Blank, "Shared Farm", "proj-team", 0));
        var store = new ProjectStore(Path.Combine(dir.Path, "mine"));
        CopyFolder(source.FolderFor("proj-team"), store.FolderFor("proj-team"));

        var summary = Assert.Single(store.List());
        Assert.Equal(("proj-team", "Shared Farm"), (summary.Id, summary.Name));
        Assert.True(store.Load("proj-team").Ok);
        store.Delete("proj-team");
        Assert.False(Directory.Exists(store.FolderFor("proj-team")));
    }

    [Fact]
    public void AProjectFolderFromElsewhere_ImportsAsOneProject()
    {
        using var dir = new TempDir();
        var source = new ProjectStore(Path.Combine(dir.Path, "team"));
        var project = FarmEngine.Authoring.ProjectCatalog.CreateNewProject(ProjectTemplates.Starter, "Team Farm", "proj-team", 0);
        source.Save(project);

        var (json, errors) = ProjectStore.ReadFolderAsJson(source.FolderFor("proj-team"));
        Assert.Empty(errors);
        var imported = new ProjectStore(Path.Combine(dir.Path, "mine")).Import(json!);
        Assert.True(imported.Ok, string.Join("; ", imported.Errors));
        Assert.Equal("Team Farm", imported.Project!.Name);
        Assert.Equal(project.Scenes.Length, imported.Project.Scenes.Length);

        File.Delete(Path.Combine(source.FolderFor("proj-team"), "scenes", "scene-farm.json"));
        var (missing, why) = ProjectStore.ReadFolderAsJson(source.FolderFor("proj-team"));
        Assert.Null(missing);
        Assert.Contains("scenes/scene-farm.json is missing from the project folder", why);
    }

    [Fact]
    public void AProjectCalledBackups_DoesNotTakeTheBackupsFolder()
    {
        Assert.NotEqual("backups", ProjectStore.SafeFileName("backups"));
        Assert.Equal("backups", ProjectStore.IdForFileName(ProjectStore.SafeFileName("backups")));
    }

    [Fact]
    public void DeleteStaleTempFiles_CleansTheProjectsFolder()
    {
        using var dir = new TempDir();
        var time = new TestTime(DateTimeOffset.UtcNow.AddHours(1));
        var store = new ProjectStore(dir.Path, time);
        Directory.CreateDirectory(store.ProjectsDirectory);
        File.WriteAllText(Path.Combine(store.ProjectsDirectory, $".proj-a.json.{Guid.NewGuid():N}.tmp"), "half a project");
        Directory.CreateDirectory(Path.Combine(store.ProjectsDirectory, "proj-a", "scenes"));
        File.WriteAllText(Path.Combine(store.ProjectsDirectory, "proj-a", "scenes", $".farm.json.{Guid.NewGuid():N}.tmp"), "half a scene");

        Assert.Equal(2, store.DeleteStaleTempFiles());
        Assert.Empty(Directory.GetFiles(store.ProjectsDirectory));
    }
}
