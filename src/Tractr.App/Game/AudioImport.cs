using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Schemas;
using Tractr.App.Projects;

namespace Tractr.App.Game;

/// <summary>
/// Turns OGG (Vorbis) and WAV files into sound or music assets (M2): the file is kept as it is,
/// as a base64 data URL the player decodes when the game loads. The format is told by the file's
/// own header, not its name.
/// </summary>
internal static class AudioImport
{
    internal static readonly string[] FilePatterns = ["*.ogg", "*.wav"];

    /// <summary>
    /// An asset of <paramref name="type"/> (<see cref="CustomAssetTypes.Sound"/> or
    /// <see cref="CustomAssetTypes.Music"/>) holding <paramref name="bytes"/>.
    /// </summary>
    /// <exception cref="ArgumentException">The file is empty, too large, or not OGG or WAV.</exception>
    public static CustomAsset FromBytes(GameProject project, string fileName, byte[] bytes, string type)
    {
        ArgumentNullException.ThrowIfNull(project);
        ArgumentNullException.ThrowIfNull(fileName);
        ArgumentNullException.ThrowIfNull(bytes);
        if (type != CustomAssetTypes.Sound && type != CustomAssetTypes.Music)
            throw new ArgumentException("Import audio as a sound or as music.", nameof(type));
        if (bytes.Length == 0 || bytes.Length > PickedFiles.MaxAudioBytes)
            throw new ArgumentException($"Each sound must be under {PickedFiles.MaxAudioBytes / (1024 * 1024)} MB.", nameof(bytes));
        var mime = MimeType(bytes) ?? throw new ArgumentException("Use OGG (Vorbis) or WAV sound files.", nameof(bytes));
        var id = Defaults.NextId(type, project.CustomAssets.Select(asset => asset.Id));
        return CustomAsset.Default.WithId(id).WithName(fileName).WithType(type).WithDataUrl($"data:{mime};base64,{Convert.ToBase64String(bytes)}");
    }

    /// <summary>"audio/ogg" for an Ogg stream, "audio/wav" for a RIFF WAVE file, else null.</summary>
    internal static string? MimeType(byte[] bytes)
    {
        ArgumentNullException.ThrowIfNull(bytes);
        static bool At(byte[] data, int offset, string tag) =>
            data.Length >= offset + tag.Length && tag.Select((c, i) => data[offset + i] == c).All(same => same);
        return At(bytes, 0, "OggS") ? "audio/ogg"
            : At(bytes, 0, "RIFF") && At(bytes, 8, "WAVE") ? "audio/wav"
            : null;
    }

    /// <summary>Whether <paramref name="asset"/> is a sound or music asset (not art).</summary>
    internal static bool IsAudio(CustomAsset asset) => asset.Type == CustomAssetTypes.Sound || asset.Type == CustomAssetTypes.Music;

    /// <summary>How many bytes the audio of <paramref name="asset"/> holds (from its base64 data URL).</summary>
    internal static long Size(CustomAsset asset)
    {
        var data = asset.DataUrl;
        var comma = data.IndexOf(',', StringComparison.Ordinal);
        if (comma < 0) return 0;
        var length = data.Length - comma - 1;
        var padding = data.EndsWith("==", StringComparison.Ordinal) ? 2 : data.EndsWith('=') ? 1 : 0;
        return Math.Max(0, (long)length * 3 / 4 - padding);
    }
}
