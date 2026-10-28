namespace Tractr.App.Projects;

/// <summary>Where the desktop app keeps its data.</summary>
public static class AppDataPaths
{
    /// <summary>Overrides the data directory (tests, portable installs).</summary>
    public const string DataDirectoryVariable = Tractr.Updates.AppDataDirectory.OverrideVariable;

    /// <summary>
    /// <c>%APPDATA%/Tractr</c> (Linux: <c>~/.config/Tractr</c>; macOS: <c>~/Library/Application Support/Tractr</c>), or
    /// <see cref="DataDirectoryVariable"/> when set. Holds <c>settings.json</c> and <c>projects/</c>.
    /// </summary>
    public static string DefaultRoot => Tractr.Updates.AppDataDirectory.DefaultRoot;

    /// <summary>
    /// The shared <c>settings.json</c> in <paramref name="root"/> (default <see cref="DefaultRoot"/>):
    /// the editor's and the Update Center's settings, so a portable install keeps both together.
    /// </summary>
    public static string SettingsFile(string? root = null) => Path.Combine(root ?? DefaultRoot, "settings.json");

    /// <summary>Rolling log files of the editor.</summary>
    public static string LogsDirectory(string? root = null) => Path.Combine(root ?? DefaultRoot, "logs");
}
