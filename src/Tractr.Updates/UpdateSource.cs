namespace Tractr.Updates;

/// <summary>Where releases are published.</summary>
public static class UpdateSource
{
    public const string Owner = "JX-Holdings-LLC";
    public const string Repository = "tractr";

    /// <summary>Repository URL handed to Velopack's <c>GithubSource</c>.</summary>
    public const string RepositoryUrl = "https://github.com/" + Owner + "/" + Repository;

    /// <summary>Human-facing list of all releases.</summary>
    public const string ReleasesPageUrl = RepositoryUrl + "/releases";

    /// <summary>GitHub REST endpoint listing releases (newest first).</summary>
    public const string ReleasesApiUrl = "https://api.github.com/repos/" + Owner + "/" + Repository + "/releases";
}
