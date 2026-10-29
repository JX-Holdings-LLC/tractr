using Avalonia.Headless.XUnit;
using FarmEngine.Authoring;
using Tractr.App.Projects;
using static Tractr.App.Tests.Ui.UiTestHelpers;

namespace Tractr.App.Tests.Game;

/// <summary>
/// The debounced autosave on a fake clock (<see cref="TestTime"/>): edits are written once
/// they have been quiet for the delay, every edit restarts it, and a flush stops it.
/// </summary>
public sealed class AutosaveTests
{
    private static readonly TimeSpan Delay = TimeSpan.FromSeconds(1);

    private static (ProjectWorkspace Workspace, TestTime Time) Open(TempDir dir)
    {
        var time = new TestTime(new DateTimeOffset(2026, 10, 1, 9, 0, 0, TimeSpan.Zero));
        var workspace = new ProjectWorkspace(new ProjectStore(dir.Path, time), new AppSettingsStore(Path.Combine(dir.Path, "settings.json")), Delay, time);
        Assert.Empty(workspace.OpenStartupProject());
        return (workspace, time);
    }

    private static string Saved(ProjectWorkspace workspace) => workspace.Store.Load(workspace.Current!.Id).Project!.Name;

    private static void Rename(ProjectWorkspace workspace, string name) =>
        Assert.True(workspace.Apply(Edits.SetProjectInfo(name, workspace.Current!.Version)));

    /// <summary>Moves the fake clock and runs what its timers posted to the UI thread.</summary>
    private static void Wait(TestTime time, double seconds)
    {
        time.Advance(TimeSpan.FromSeconds(seconds));
        Pump();
    }

    [AvaloniaFact]
    public void EditsAreSavedOnceTheyHaveBeenQuietForTheDelay()
    {
        using var dir = new TempDir();
        var (workspace, time) = Open(dir);
        var original = Saved(workspace);

        Rename(workspace, "First");
        Assert.True(workspace.HasPendingSave);
        Wait(time, 0.6);
        Assert.Equal(original, Saved(workspace));

        // Another edit restarts the delay: 1.2 s after the first edit, nothing is written yet.
        Rename(workspace, "Second");
        Wait(time, 0.6);
        Assert.True(workspace.HasPendingSave);
        Assert.Equal(original, Saved(workspace));

        Wait(time, 0.4);
        PumpUntil(() => !workspace.HasPendingSave);
        Assert.False(workspace.HasPendingSave);
        Assert.Equal("Second", Saved(workspace));
        Assert.Equal(0, time.PendingTimers);
        workspace.ReleaseLock();
    }

    [AvaloniaFact]
    public void AFlushWritesAtOnceAndCancelsTheTimer()
    {
        using var dir = new TempDir();
        var (workspace, time) = Open(dir);

        Rename(workspace, "Flushed");
        Assert.Equal(1, time.PendingTimers);
        workspace.FlushPendingSave();
        Assert.Equal("Flushed", Saved(workspace));
        Assert.Equal(0, time.PendingTimers);

        // A later edit waits the whole delay again; the stopped timer never fires on its own.
        Rename(workspace, "Later");
        Wait(time, 0.99);
        Assert.Equal("Flushed", Saved(workspace));
        Wait(time, 0.01);
        PumpUntil(() => !workspace.HasPendingSave);
        Assert.Equal("Later", Saved(workspace));
        workspace.ReleaseLock();
    }

    [AvaloniaFact]
    public void ABackgroundFailureKeepsTheDraftAndReportsStatusOnTheUiThread()
    {
        using var dir = new TempDir();
        var (workspace, time) = Open(dir);
        var path = workspace.Store.PathFor(workspace.Current!.Id);
        File.Delete(path);
        Directory.CreateDirectory(path);
        var uiThread = Environment.CurrentManagedThreadId;
        var notifications = new List<int>();
        workspace.SaveStatusChanged += (_, _) => notifications.Add(Environment.CurrentManagedThreadId);
        Rename(workspace, "Unsaved draft");
        Wait(time, 1);
        PumpUntil(() => workspace.SaveError is not null);
        Assert.True(workspace.HasPendingSave);
        Assert.False(workspace.IsSaving);
        Assert.Equal("Unsaved draft", workspace.Current!.Name);
        Assert.All(notifications, thread => Assert.Equal(uiThread, thread));
        Directory.Delete(path);
        Assert.True(workspace.RetrySave());
        Assert.Equal("Unsaved draft", Saved(workspace));
        workspace.ReleaseLock();
    }

    private sealed class BlockingStoreClock : TimeProvider, IDisposable
    {
        public ManualResetEventSlim Entered { get; } = new(false);
        public ManualResetEventSlim Release { get; } = new(false);
        public bool Block { get; set; }
        public int WriterThread { get; private set; }

        public override DateTimeOffset GetUtcNow()
        {
            if (Block)
            {
                WriterThread = Environment.CurrentManagedThreadId;
                Entered.Set();
                if (!Release.Wait(TimeSpan.FromSeconds(20))) throw new TimeoutException("Test did not release the background writer.");
            }
            return DateTimeOffset.UtcNow;
        }

        public void Dispose()
        {
            Release.Set();
            Entered.Dispose();
            Release.Dispose();
        }
    }

    [AvaloniaFact]
    public void AutosaveRunsOffTheUiThreadAndFlushKeepsEditsMadeWhileItWrites()
    {
        using var dir = new TempDir();
        using var clock = new BlockingStoreClock();
        var time = new TestTime(DateTimeOffset.UtcNow);
        var workspace = new ProjectWorkspace(new ProjectStore(dir.Path, clock),
            new AppSettingsStore(Path.Combine(dir.Path, "settings.json")), Delay, time);
        Assert.Empty(workspace.OpenStartupProject());
        var uiThread = Environment.CurrentManagedThreadId;
        clock.Block = true;
        try
        {
            Rename(workspace, "Snapshot");
            Wait(time, 1);
            PumpUntil(() => clock.Entered.IsSet);
            Assert.True(workspace.IsSaving);
            Assert.NotEqual(uiThread, clock.WriterThread);
            // The UI can apply another immutable project while disk work is blocked.
            Rename(workspace, "Latest edit");
            Assert.Equal("Latest edit", workspace.Current!.Name);
        }
        finally
        {
            clock.Block = false;
            clock.Release.Set();
            workspace.FlushPendingSave();
        }
        Assert.False(workspace.IsSaving);
        Assert.False(workspace.HasPendingSave);
        Assert.Equal("Latest edit", Saved(workspace));
        // A queued completion from the old snapshot must not replace the flushed state.
        Pump();
        Assert.Equal("Latest edit", Saved(workspace));
        workspace.ReleaseLock();
    }

    [AvaloniaFact]
    public void ATickPostedBeforeARestartIsDropped()
    {
        var fired = 0;
        var time = new TestTime(DateTimeOffset.UnixEpoch);
        var timer = new DebounceTimer(time, Delay, () => fired++);

        timer.Start();
        time.Advance(Delay); // the tick is posted to the UI thread, not run yet
        timer.Start();       // re-armed before the UI thread ran it
        Pump();
        Assert.Equal(0, fired);
        Assert.True(timer.IsRunning);

        Wait(time, 1);
        Assert.Equal(1, fired);
        Assert.False(timer.IsRunning);

        timer.Start();
        timer.Stop();
        Wait(time, 5);
        Assert.Equal(1, fired);
    }
}
