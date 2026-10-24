using FarmEngine.Interop;

namespace Tractr.App.Game;

/// <summary>
/// Plays a sound or music file for the Art tab's play button. Avalonia has no audio, so the real
/// one (<see cref="RustSoundPreview"/>) goes through the Rust player's decoder and output device;
/// tests swap in a fake.
/// </summary>
internal interface ISoundPreview
{
    /// <summary>Why nothing can play in this editor, or null when the preview works.</summary>
    string? UnavailableReason { get; }

    /// <summary>Whether a file is still playing.</summary>
    bool IsPlaying { get; }

    /// <summary>Plays <paramref name="file"/> once, stopping what played before.</summary>
    /// <exception cref="FarmFfiException">It won't play; the message says why.</exception>
    Task PlayAsync(byte[] file);

    void Stop();
}

/// <summary>The preview through <see cref="RustAudio"/> (one per process, like its output stream).</summary>
internal sealed class RustSoundPreview : ISoundPreview
{
    public static RustSoundPreview Instance { get; } = new();

    private RustSoundPreview()
    {
    }

    public string? UnavailableReason => RustAudio.OutputUnavailableReason;

    public bool IsPlaying => RustAudio.Playing;

    /// <summary>Decoding and resampling long music takes a moment, so it runs off the UI thread.</summary>
    public Task PlayAsync(byte[] file) => Task.Run(() => RustAudio.Play(file));

    public void Stop() => RustAudio.Stop();
}
