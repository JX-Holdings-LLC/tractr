using System.Runtime.InteropServices;
using Tractr.App.Mvvm;
using Tractr.App.Projects;
using Tractr.App.Services;
using Tractr.Updates;

namespace Tractr.App.ViewModels;

/// <param name="version">The running version.</param>
/// <param name="launcher">Opens links and the log folder.</param>
/// <param name="logFolder">Where the editor's log files are (default <see cref="AppDataPaths.LogsDirectory"/>).</param>
public sealed class AboutViewModel(string version, IUrlLauncher launcher, string? logFolder = null) : ObservableObject
{
    public string Title => "Tractr";

    public string VersionText => $"Version {version}";

    public string Description =>
        "The farm game engine. Build and play cozy farming RPGs: design maps, crops, NPCs and quests, then jump straight into play mode.";

    public string RuntimeText =>
        $"{RuntimeInformation.FrameworkDescription} · Avalonia {typeof(Avalonia.Application).Assembly.GetName().Version?.ToString(3)} · {RuntimeInformation.OSDescription}";

    public string LicenseText =>
        "Source-available under the Business Source License 1.1. Free until a game you make earns US$1,000,000. Third-party notices are in the licenses folder next to the app.";

    public RelayCommand OpenRepositoryCommand { get; } = new(() => launcher.Open(UpdateSource.RepositoryUrl));

    public RelayCommand OpenLicenseCommand { get; } = new(() => launcher.Open(UpdateSource.RepositoryUrl + "/blob/main/LICENSE"));

    public RelayCommand OpenReleasesCommand { get; } = new(() => launcher.Open(UpdateSource.ReleasesPageUrl));

    /// <summary>The editor's log files (attach them to a bug report).</summary>
    public string LogFolder { get; } = logFolder ?? AppDataPaths.LogsDirectory();

    public string LogFolderText => $"Logs: {LogFolder}";

    public RelayCommand OpenLogFolderCommand { get; } = new(() =>
    {
        var folder = logFolder ?? AppDataPaths.LogsDirectory();
        try
        {
            Directory.CreateDirectory(folder);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            System.Diagnostics.Trace.TraceWarning($"Could not create the log folder {folder}: {ex.Message}");
        }

        launcher.OpenFolder(folder);
    });
}
