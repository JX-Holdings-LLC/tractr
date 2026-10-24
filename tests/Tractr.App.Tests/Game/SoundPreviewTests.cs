using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Headless.XUnit;
using Avalonia.Interactivity;
using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Interop;
using FarmEngine.Schemas;
using Tractr.App.Game;
using static Tractr.App.Tests.Ui.UiTestHelpers;

namespace Tractr.App.Tests.Game;

/// <summary>The Art tab's sound list (M11): playing a sound or music asset, and renaming it.</summary>
public sealed class SoundPreviewTests
{
    /// <summary>A preview that records what it was asked to play instead of playing it.</summary>
    private sealed class FakePreview(string? unavailable = null) : ISoundPreview
    {
        public string? UnavailableReason => unavailable;

        public bool IsPlaying { get; set; }

        public List<byte[]> Played { get; } = [];

        public int Stops { get; private set; }

        /// <summary>When set, playing fails with this message (no device, a broken file).</summary>
        public string? Failure { get; set; }

        public Task PlayAsync(byte[] file)
        {
            if (Failure is { } failure) throw new FarmFfiException(failure);
            Played.Add(file);
            IsPlaying = true;
            return Task.CompletedTask;
        }

        public void Stop()
        {
            Stops++;
            IsPlaying = false;
        }
    }

    private static readonly byte[] Wav = [.. "RIFF"u8.ToArray(), 4, 0, 0, 0, .. "WAVE"u8.ToArray()];

    private static ArtEditorView OpenArt(GameTestHost host)
    {
        host.Workspace.Apply(Edits.UpsertAsset(CustomAsset.Default.WithId("sound-1").WithName("chime.wav").WithType(CustomAssetTypes.Sound)
            .WithDataUrl("data:audio/wav;base64," + Convert.ToBase64String(Wav))));
        host.Workspace.Apply(Edits.UpsertAsset(CustomAsset.Default.WithId("music-1").WithName("theme.ogg").WithType(CustomAssetTypes.Music)
            .WithDataUrl("data:audio/ogg;base64,T2dnUw==")));
        FindByName<TabControl>(host.Window, "EditorTabs").SelectedIndex = 5;
        Pump();
        return FindByName<ArtEditorView>(host.Window, "ArtEditorView");
    }

    private static void Select(GameTestHost host, string id)
    {
        var sounds = FindByName<ListBox>(host.Window, "SoundAssets");
        sounds.SelectedItem = sounds.Items.OfType<ListBoxItem>().First(item => Equals(item.Tag, id));
        Pump();
    }

    private static void Press(GameTestHost host, string name)
    {
        FindByName<Button>(host.Window, name).RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
        Pump();
    }

    [AvaloniaFact]
    public void ThePlayButtonPlaysTheSelectedSoundAndStops()
    {
        using var host = new GameTestHost();
        var art = OpenArt(host);
        var preview = new FakePreview();
        art.SoundPreview = preview;
        var play = FindByName<Button>(host.Window, "PlaySoundButton");
        Assert.False(play.IsEnabled, "nothing is selected yet");

        Select(host, "sound-1");
        Assert.True(play.IsEnabled);
        Assert.Equal("Play sound", AutomationProperties.GetName(play));
        Press(host, "PlaySoundButton");
        Assert.Equal(Wav, Assert.Single(preview.Played));
        Assert.Equal("sound-1", art.PlayingSoundId);
        Assert.Equal("Stop sound", AutomationProperties.GetName(play));
        Assert.Equal("Playing chime.wav.", FindByName<TextBlock>(host.Window, "ArtMessage").Text);

        // Pressed again it stops; a sound that ends turns the button back by itself.
        Press(host, "PlaySoundButton");
        Assert.Equal((null, 1), (art.PlayingSoundId, preview.Stops));
        Assert.Equal("Play sound", AutomationProperties.GetName(play));
        Press(host, "PlaySoundButton");
        preview.IsPlaying = false;
        art.WatchSound();
        Assert.Null(art.PlayingSoundId);
        Assert.Equal("Play sound", AutomationProperties.GetName(play));

        // Removing the sound that plays stops it.
        Press(host, "PlaySoundButton");
        Press(host, "RemoveSoundButton");
        Assert.Null(art.PlayingSoundId);
        Assert.Equal(2, preview.Stops);
    }

    [AvaloniaFact]
    public void AFileThatWontPlaySaysWhy()
    {
        using var host = new GameTestHost();
        var art = OpenArt(host);
        var preview = new FakePreview { Failure = "Ogg Vorbis: the file is cut short" };
        art.SoundPreview = preview;
        Select(host, "music-1");
        Press(host, "PlaySoundButton");
        Assert.Null(art.PlayingSoundId);
        Assert.Equal("Could not play theme.ogg: Ogg Vorbis: the file is cut short", FindByName<TextBlock>(host.Window, "ArtMessage").Text);
        Assert.True(FindByName<Button>(host.Window, "PlaySoundButton").IsEnabled);
    }

    [AvaloniaFact]
    public void WithoutSoundOutputThePlayButtonIsOffAndSaysWhy()
    {
        using var host = new GameTestHost();
        var art = OpenArt(host);
        art.SoundPreview = new FakePreview("This editor was built without sound output.");
        Select(host, "sound-1");
        var play = FindByName<Button>(host.Window, "PlaySoundButton");
        Assert.False(play.IsEnabled);
        Assert.Equal("Sound preview is unavailable: This editor was built without sound output.", ToolTip.GetTip(play));
        Assert.True(ToolTip.GetShowOnDisabled(play));
    }

    [AvaloniaFact]
    public void TheRealPreviewIsOnWhenThisEditorHasSoundOutput()
    {
        using var host = new GameTestHost();
        OpenArt(host);
        Select(host, "sound-1");
        Assert.Equal(RustAudio.OutputAvailable, FindByName<Button>(host.Window, "PlaySoundButton").IsEnabled);
    }

    [AvaloniaFact]
    public void RenamingASoundKeepsItsIdAndIsOneUndoStep()
    {
        using var host = new GameTestHost();
        OpenArt(host);
        Select(host, "sound-1");
        var name = FindByName<TextBox>(host.Window, "SoundAssetName");
        Assert.Equal("chime.wav", name.Text);
        var before = host.Workspace.Current!;
        name.Text = "  Door chime ";
        Press(host, "RenameSoundButton");
        var sound = host.Workspace.Current!.CustomAssets.Single(asset => asset.Id == "sound-1");
        Assert.Equal("Door chime", sound.Name);
        Assert.Equal("Renamed to Door chime.", FindByName<TextBlock>(host.Window, "ArtMessage").Text);
        Assert.Contains(FindByName<ListBox>(host.Window, "SoundAssets").Items.OfType<ListBoxItem>(),
            item => Equals(item.Tag, "sound-1") && (AutomationProperties.GetName(item) ?? "").StartsWith("Door chime · sound", StringComparison.Ordinal));
        Assert.Equal("Door chime", name.Text);

        // A blank name changes nothing.
        name.Text = "   ";
        Press(host, "RenameSoundButton");
        Assert.Equal("Door chime", host.Workspace.Current!.CustomAssets.Single(asset => asset.Id == "sound-1").Name);

        host.Workspace.Undo();
        Pump();
        Assert.Same(before, host.Workspace.Current);
        Assert.Equal("chime.wav", name.Text);
    }
}
