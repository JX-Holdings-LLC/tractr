using System.Text;
using System.Text.Json;

namespace FarmEngine.Interop;

/// <summary>What the player's decoder made of a sound or music file.</summary>
public sealed record AudioClipInfo(int SampleRate, int Channels, double Seconds);

/// <summary>
/// Sound and music assets through the Rust player's own decoders and output device
/// (<c>crates/farm-ffi/src/audio.rs</c>): decoding a file as the game will, and the Art tab's
/// preview, which plays one file at a time on the default output device. Avalonia has no
/// audio of its own.
/// </summary>
public static class RustAudio
{
    /// <summary>
    /// Whether this editor can play sound: the library loaded and was built with sound output
    /// (<c>FarmFfiAudio</c>; an editor built for machines without ALSA has none). Whether there
    /// is a device shows only when something plays.
    /// </summary>
    public static bool OutputAvailable => FarmFfi.IsAvailable && NativeMethods.fe_audio_output() == 1;

    /// <summary>
    /// Why this editor cannot play sound, or null when it can (see <see cref="OutputAvailable"/>).
    /// </summary>
    public static string? OutputUnavailableReason =>
        !FarmFfi.IsAvailable ? $"The Rust engine library is not available: {FarmFfi.LoadError}"
        : NativeMethods.fe_audio_output() == 1 ? null
        : "This editor was built without sound output.";

    /// <summary>Decodes an Ogg Vorbis or WAV file as the game does.</summary>
    /// <exception cref="FarmFfiException">The file won't play; the message says why.</exception>
    public static AudioClipInfo Probe(byte[] file)
    {
        ArgumentNullException.ThrowIfNull(file);
        var json = Call(file, play: false);
        using var info = JsonDocument.Parse(json);
        var root = info.RootElement;
        return new AudioClipInfo(root.GetProperty("sampleRate").GetInt32(), root.GetProperty("channels").GetInt32(), root.GetProperty("seconds").GetDouble());
    }

    /// <summary>
    /// Plays an Ogg Vorbis or WAV file once, stopping what the preview played before. Decoding
    /// takes a moment for long music: call it off the UI thread.
    /// </summary>
    /// <exception cref="FarmFfiException">The file won't play, there is no output device, or this editor has no sound output; the message says why.</exception>
    public static void Play(byte[] file)
    {
        ArgumentNullException.ThrowIfNull(file);
        Call(file, play: true);
    }

    /// <summary>Stops the preview (nothing happens when none plays or the library is missing).</summary>
    public static void Stop()
    {
        if (FarmFfi.IsAvailable) NativeMethods.fe_audio_preview_stop();
    }

    /// <summary>Whether the preview is still playing.</summary>
    public static bool Playing => FarmFfi.IsAvailable && NativeMethods.fe_audio_preview_playing() == 1;

    private static string Call(byte[] file, bool play)
    {
        FarmFfi.EnsureAvailable();
        unsafe
        {
            fixed (byte* ptr = file)
            {
                NativeMethods.FeBytes output = default;
                var result = play
                    ? NativeMethods.fe_audio_preview_play(ptr, (nuint)file.Length, &output)
                    : NativeMethods.fe_audio_probe(ptr, (nuint)file.Length, &output);
                var text = Encoding.UTF8.GetString(RustRender.TakeBytes(output));
                if (result != NativeMethods.FeResult.Ok)
                {
                    throw new FarmFfiException(text.Length > 0 ? text : $"{(play ? nameof(Play) : nameof(Probe))} failed: {result}.");
                }

                return text;
            }
        }
    }
}
