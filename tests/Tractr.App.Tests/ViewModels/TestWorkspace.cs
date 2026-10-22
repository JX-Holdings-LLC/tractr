using FarmEngine.Schemas;
using Tractr.App.Projects;
using Tractr.App.Tests.Game;

namespace Tractr.App.Tests.ViewModels;

/// <summary>
/// A <see cref="ProjectWorkspace"/> over a temp folder with a project open and every edit saved
/// at once: what the Edit Mode view models need, without a window or Avalonia.
/// </summary>
internal sealed class TestWorkspace : IDisposable
{
    private readonly TempDir _dir = new();

    public TestWorkspace(string template = ProjectTemplates.Starter)
    {
        Workspace = new ProjectWorkspace(
            new ProjectStore(_dir.Path),
            new AppSettingsStore(Path.Combine(_dir.Path, "settings.json")),
            autosaveDelay: TimeSpan.Zero);
        Workspace.Open(Workspace.CreateProject(template, "View Model Farm"));
    }

    public ProjectWorkspace Workspace { get; }

    public GameProject Project => Workspace.Current!;

    public void Dispose()
    {
        Workspace.ReleaseLock();
        _dir.Dispose();
    }
}
