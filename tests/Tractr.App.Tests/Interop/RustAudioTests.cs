using FarmEngine.Authoring;
using FarmEngine.Interop;

namespace Tractr.App.Tests.Interop;

/// <summary>
/// Sound and music through the player's decoder (<c>fe_audio_*</c>), and the F# check behind
/// the <c>asset.audioUnreadable</c> problem against that decoder.
/// </summary>
public sealed class RustAudioTests
{
    private static byte[] Tone() => File.ReadAllBytes(Path.Combine(AppContext.BaseDirectory, "Fixtures", "audio", "tone.ogg"));

    private static bool RustDecodes(byte[] file)
    {
        try
        {
            RustAudio.Probe(file);
            return true;
        }
        catch (FarmFfiException)
        {
            return false;
        }
    }

    [NativeFact]
    public void ProbeDecodesLikeThePlayer()
    {
        var info = RustAudio.Probe(Tone());
        Assert.Equal((22050, 2), (info.SampleRate, info.Channels));
        Assert.InRange(info.Seconds, 0.24, 0.26);
        var error = Assert.Throws<FarmFfiException>(() => RustAudio.Probe([0x89, (byte)'P', (byte)'N', (byte)'G']));
        Assert.Contains("not an Ogg Vorbis or WAV", error.Message, StringComparison.Ordinal);
        Assert.Equal(RustAudio.OutputAvailable, RustAudio.OutputUnavailableReason is null);
    }

    /// <summary>
    /// Problems may only call a file unplayable when the player cannot decode it: an error there
    /// blocks Export. Every cut and every changed byte of the tone is tried.
    /// </summary>
    [NativeFact]
    public void TheFSharpCheckNeverRefusesAFileThePlayerDecodes()
    {
        var tone = Tone();
        var variants = new List<byte[]>();
        for (var end = 0; end <= tone.Length; end++) variants.Add(tone[..end]);
        for (var i = 0; i < tone.Length; i++)
        {
            var changed = (byte[])tone.Clone();
            changed[i] ^= 0x5A;
            variants.Add(changed);
        }

        var refusedByBoth = 0;
        foreach (var variant in variants)
        {
            var rust = RustDecodes(variant);
            var fsharp = AudioFiles.check(variant);
            Assert.False(rust && fsharp.IsError, $"F# refused a {variant.Length}-byte file the player decodes: {(fsharp.IsError ? fsharp.ErrorValue : "")}");
            if (!rust && fsharp.IsError) refusedByBoth++;
        }

        // Nearly everything the decoder refuses, F# finds too (the page checksums catch most).
        var refusedByRust = variants.Count(variant => !RustDecodes(variant));
        Assert.True(refusedByBoth >= refusedByRust * 9 / 10, $"F# found {refusedByBoth} of {refusedByRust} files the player refuses.");
    }

    [NativeFact]
    public void ThePreviewRefusesBrokenFilesAndStops()
    {
        var error = Assert.Throws<FarmFfiException>(() => RustAudio.Play(Tone()[..100]));
        Assert.StartsWith("Ogg Vorbis", error.Message, StringComparison.Ordinal);
        RustAudio.Stop();
        Assert.False(RustAudio.Playing);
    }
}
