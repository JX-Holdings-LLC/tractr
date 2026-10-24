using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Headless.XUnit;
using FarmEngine.Authoring;
using FarmEngine.Authoring.Net;
using FarmEngine.Interop;
using Tractr.App.Game;
using Tractr.App.Localization;
using static Tractr.App.Tests.Ui.UiTestHelpers;

namespace Tractr.App.Tests.Game;

/// <summary>
/// Edit Mode's sky preview (M11): the map toolbar's time, season and weather pickers draw the
/// scene under play's day/night tint, foliage tint and weather. It is view state: off by
/// default, never an edit. The tints themselves are tested in <c>crates/farm-render</c>.
/// </summary>
public sealed class SkyPreviewTests
{
    private static void Choose(GameTestHost host, string name, string tag)
    {
        var picker = FindByName<ComboBox>(host.Window, name);
        picker.SelectedItem = picker.Items.OfType<ComboBoxItem>().First(item => (string)item.Tag! == tag);
        Pump();
    }

    private static IReadOnlyList<string?> Tags(ComboBox picker) => [.. picker.Items.OfType<ComboBoxItem>().Select(item => item.Tag as string)];

    /// <summary>The map's pixels as the window draws them now (the toolbar changes as the pickers show).</summary>
    private static byte[] Frame(GameTestHost host)
    {
        var canvas = host.Surface.EditView.Canvas;
        var origin = canvas.TranslatePoint(default, host.Window)!.Value;
        using var frame = host.Window.CaptureRenderedFrame()!;
        using var buffer = frame.Lock();
        var scale = frame.Size.Width / host.Window.Bounds.Width;
        var left = (int)Math.Max(0, origin.X * scale);
        var top = (int)Math.Max(0, origin.Y * scale);
        var width = (int)Math.Min(canvas.Bounds.Width * scale, buffer.Size.Width - left);
        var height = (int)Math.Min(canvas.Bounds.Height * scale, buffer.Size.Height - top);
        var bytes = new byte[width * height * 4];
        for (var y = 0; y < height; y++)
        {
            System.Runtime.InteropServices.Marshal.Copy(buffer.Address + ((top + y) * buffer.RowBytes) + (left * 4), bytes, y * width * 4, width * 4);
        }

        return bytes;
    }

    [AvaloniaFact]
    public void TheSkyIsOffUntilChosen_ThenDrawsTheTimeSeasonAndWeather_WithoutEditingTheProject()
    {
        using var host = new GameTestHost();
        var edit = host.Surface.EditView;
        var project = host.Workspace.Current!;
        var sky = FindByName<CheckBox>(host.Window, "ShowSky");
        var row = FindByName<StackPanel>(host.Window, "SkyRow");
        var time = FindByName<ComboBox>(host.Window, "SkyTime");
        var season = FindByName<ComboBox>(host.Window, "SkySeason");
        var weather = FindByName<ComboBox>(host.Window, "SkyWeather");
        Assert.False(sky.IsChecked == true);
        Assert.False(row.IsVisible);
        Assert.Null(edit.Sky);
        Assert.Equal(EditorStrings.Get("sky.name"), AutomationProperties.GetName(sky));
        Assert.Equal(EditorStrings.Get("sky.timeName"), AutomationProperties.GetName(time));
        Assert.Equal(["sky.morning", "sky.noon", "sky.evening", "sky.night"], Tags(time));
        Assert.Equal(project.Settings.Calendar.Seasons.Select(s => (string?)s.Id), Tags(season));
        Assert.Equal(["", .. project.Weather.Types.Select(w => (string?)w.Id)], Tags(weather));
        var plain = Frame(host);

        // On: the evening of the starting season, clear.
        sky.IsChecked = true;
        Pump();
        Assert.True(row.IsVisible);
        Assert.Equal(new PreviewSky((18 * 60) + 40, project.CurrentSeason, null), edit.Sky);
        Assert.Equal(edit.Sky, edit.Canvas.Sky);
        Choose(host, "SkyTime", "sky.noon");
        var noon = Frame(host);
        Choose(host, "SkyTime", "sky.night");
        Assert.Equal(new PreviewSky(22 * 60, project.CurrentSeason, null), edit.Sky);
        Assert.False(Frame(host).AsSpan().SequenceEqual(noon), "the night tint is drawn");

        var winter = project.Settings.Calendar.Seasons[^1].Id;
        var rain = project.Weather.Types.First(w => w.Id != "sun").Id;
        Choose(host, "SkySeason", winter);
        Choose(host, "SkyWeather", rain);
        Choose(host, "SkyTime", "sky.morning");
        Assert.Equal(new PreviewSky(7 * 60, winter, rain), edit.Sky);

        // A view choice: no undo step, the project is untouched.
        Assert.Same(project, host.Workspace.Current);
        Assert.False(host.Workspace.CanUndo);

        // The choice survives edits; off draws the plain map again, and on brings the choice back.
        host.Workspace.Apply(Edits.SetProjectInfo("Sky Farm", project.Version));
        Pump();
        Assert.Equal(new PreviewSky(7 * 60, winter, rain), edit.Sky);
        host.Workspace.Undo();
        Pump();
        sky.IsChecked = false;
        Pump();
        Assert.Null(edit.Sky);
        Assert.False(row.IsVisible);
        Assert.True(Frame(host).AsSpan().SequenceEqual(plain));
        sky.IsChecked = true;
        Pump();
        Assert.Equal(new PreviewSky(7 * 60, winter, rain), edit.Sky);
    }

    [AvaloniaFact]
    public void AnIndoorSceneHasNoWeather_AndTheGamesDayLengthScalesTheTimes()
    {
        using var host = new GameTestHost();
        var edit = host.Surface.EditView;
        var project = host.Workspace.Current!;
        host.Workspace.Apply(Edits.SetSettings(project.Settings.WithTime(SettingsForm.WithDayMinutes(project.Settings.Time, 720))));
        Pump();
        FindByName<CheckBox>(host.Window, "ShowSky").IsChecked = true;
        Pump();
        Assert.Equal(((18 * 60) + 40) / 2.0, edit.Sky!.TimeMinutes);
        var weather = FindByName<ComboBox>(host.Window, "SkyWeather");
        Assert.True(weather.IsEnabled);

        host.Workspace.Apply(Edits.SetSceneIndoor(edit.SceneId!, true));
        Pump();
        Assert.False(weather.IsEnabled);
        Assert.Equal(EditorStrings.Get("sky.indoor"), ToolTip.GetTip(weather));
    }

    [AvaloniaFact]
    public void TheSkyControlsReadTheEditorsLanguage()
    {
        EditorStrings.SetLanguage("es");
        try
        {
            using var host = new GameTestHost();
            var time = FindByName<ComboBox>(host.Window, "SkyTime");
            Assert.Equal(["Mañana", "Mediodía", "Atardecer", "Noche"], time.Items.OfType<ComboBoxItem>().Select(item => item.Content as string));
            Assert.Equal("Cielo", FindByName<CheckBox>(host.Window, "ShowSky").Content);
            Assert.Equal("Vista previa del cielo: hora del día", AutomationProperties.GetName(time));
        }
        finally
        {
            EditorStrings.SetLanguage(EditorStrings.English);
        }
    }
}
