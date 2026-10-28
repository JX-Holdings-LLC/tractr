namespace Tractr.Updates;

/// <summary>The shared platform location for editor and updater settings.</summary>
public static class AppDataDirectory
{
    public const string OverrideVariable = "TRACTR_DATA_DIR";

    public static string DefaultRoot => Resolve(
        Environment.GetEnvironmentVariable(OverrideVariable),
        OperatingSystem.IsMacOS(),
        Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData, Environment.SpecialFolderOption.DoNotVerify));

    internal static string Resolve(string? overridden, bool macOS, string home, string applicationData) =>
        !string.IsNullOrWhiteSpace(overridden) ? overridden
        : macOS ? Path.Combine(home, "Library", "Application Support", "Tractr")
        : Path.Combine(applicationData, "Tractr");
}
