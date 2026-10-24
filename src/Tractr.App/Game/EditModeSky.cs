using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Layout;
using FarmEngine.Interop;
using FarmEngine.Schemas;
using Tractr.App.Localization;

namespace Tractr.App.Game;

/// <summary>
/// Edit Mode's sky preview: the toolbar's <b>Sky</b> box shows a row with a time of day, a season
/// and a weather, and the map is drawn with the day/night tint, foliage tint and weather overlay
/// play draws for them (the scene's own look first, then the season's and the game's; no weather
/// indoors). It is off by default so the map stays readable, and it is the editor's view state:
/// nothing is saved in the project. The weather particles stand still, so nothing moves on the map.
/// </summary>
public sealed partial class EditModeView
{
    /// <summary>
    /// The sky preview's times of day: an <see cref="EditorStrings"/> key and the minute of a
    /// 1440-minute day, scaled to the game's own day length.
    /// </summary>
    internal static readonly IReadOnlyList<(string Key, double Minute)> SkyTimes =
    [
        ("sky.morning", 7 * 60),
        ("sky.noon", 12 * 60),
        ("sky.evening", (18 * 60) + 40),
        ("sky.night", 22 * 60),
    ];

    private readonly CheckBox _showSky = new() { Name = "ShowSky", VerticalAlignment = VerticalAlignment.Center };
    private readonly ComboBox _skyTime = new() { Name = "SkyTime", MinWidth = 110 };
    private readonly ComboBox _skySeason = new() { Name = "SkySeason", MinWidth = 110 };
    private readonly ComboBox _skyWeather = new() { Name = "SkyWeather", MinWidth = 110 };
    /// <summary>The row of the time, season and weather pickers, shown while the sky is.</summary>
    private StackPanel? _skyRow;
    /// <summary>The seasons and weather types the pickers show, to refill them only when those change.</summary>
    private string? _skyKey;
    private bool _updatingSky;

    /// <summary>The sky the map is previewed under; null when the preview is off.</summary>
    public PreviewSky? Sky => _canvas.Sky;

    /// <summary>The toolbar's <b>Sky</b> box (beside Markers).</summary>
    private CheckBox BuildSkyToggle()
    {
        _showSky.Content = EditorStrings.Get("sky.label");
        AutomationProperties.SetName(_showSky, EditorStrings.Get("sky.name"));
        ToolTip.SetTip(_showSky, EditorStrings.Get("sky.tip"));
        _showSky.IsCheckedChanged += (_, _) => ApplySky();
        return _showSky;
    }

    /// <summary>The row under the toolbar's first line: the time, season and weather pickers.</summary>
    private StackPanel BuildSkyRow()
    {
        foreach (var (key, _) in SkyTimes)
        {
            _skyTime.Items.Add(new ComboBoxItem { Content = EditorStrings.Get(key), Tag = key });
        }

        // Evening: the built-in look's midday has no tint, so the first look shows one.
        _skyTime.SelectedIndex = 2;
        AutomationProperties.SetName(_skyTime, EditorStrings.Get("sky.timeName"));
        AutomationProperties.SetName(_skySeason, EditorStrings.Get("sky.seasonName"));
        AutomationProperties.SetName(_skyWeather, EditorStrings.Get("sky.weatherName"));
        foreach (var box in new[] { _skyTime, _skySeason, _skyWeather })
        {
            box.SelectionChanged += (_, _) =>
            {
                if (!_updatingSky)
                {
                    ApplySky();
                }
            };
        }

        _skyRow = Ui.HStack(8,
            Ui.Text(EditorStrings.Get("sky.time"), "hud-label"), _skyTime,
            Ui.Text(EditorStrings.Get("sky.season"), "hud-label"), _skySeason,
            Ui.Text(EditorStrings.Get("sky.weather"), "hud-label"), _skyWeather,
            Ui.Text(EditorStrings.Get("sky.note"), "muted", "small"));
        _skyRow.Name = "SkyRow";
        _skyRow.Margin = new Thickness(0, 8, 0, 0);
        _skyRow.IsVisible = false;
        return _skyRow;
    }

    /// <summary>
    /// Offers the project's seasons and weather types, keeping the creator's choice while it
    /// exists (else the starting season, and clear weather), and turns the weather off for an
    /// indoor scene, as play does.
    /// </summary>
    private void RefreshSky(GameProject project, Scene? scene)
    {
        var seasons = project.Settings.Calendar.Seasons;
        var weathers = project.Weather.Types;
        var key = string.Join('\u001f', seasons.Select(s => $"{s.Id}\u001e{s.Name}").Append("\u001d").Concat(weathers.Select(w => $"{w.Id}\u001e{w.Name}")));
        if (key != _skyKey)
        {
            _skyKey = key;
            _updatingSky = true;
            try
            {
                var season = TagOf(_skySeason);
                _skySeason.Items.Clear();
                foreach (var s in seasons)
                {
                    _skySeason.Items.Add(new ComboBoxItem { Content = string.IsNullOrWhiteSpace(s.Name) ? s.Id : s.Name, Tag = s.Id });
                }

                Select(_skySeason, seasons.Any(s => s.Id == season) ? season : seasons.Any(s => s.Id == project.CurrentSeason) ? project.CurrentSeason : seasons.FirstOrDefault()?.Id);
                var weather = TagOf(_skyWeather);
                _skyWeather.Items.Clear();
                _skyWeather.Items.Add(new ComboBoxItem { Content = EditorStrings.Get("sky.clear"), Tag = "" });
                foreach (var w in weathers)
                {
                    _skyWeather.Items.Add(new ComboBoxItem { Content = string.IsNullOrWhiteSpace(w.Name) ? w.Id : w.Name, Tag = w.Id });
                }

                Select(_skyWeather, weathers.Any(w => w.Id == weather) ? weather : "");
            }
            finally
            {
                _updatingSky = false;
            }
        }

        var indoor = scene?.Indoor.OrNullable() == true;
        _skyWeather.IsEnabled = !indoor;
        ToolTip.SetTip(_skyWeather, indoor ? EditorStrings.Get("sky.indoor") : null);
        ApplySky();
    }

    /// <summary>Hands the chosen sky (or none) to the map.</summary>
    private void ApplySky()
    {
        var on = _showSky.IsChecked == true && _workspace.Current is not null;
        if (_skyRow is not null)
        {
            _skyRow.IsVisible = on;
        }

        var time = SkyTimes.FirstOrDefault(t => t.Key == TagOf(_skyTime));
        if (!on || time.Key is null)
        {
            _canvas.Sky = null;
            return;
        }

        var dayMinutes = _workspace.Current!.Settings.Time.DayMinutes.OrNullable() is double minutes && minutes > 0 ? minutes : 1440;
        var weather = TagOf(_skyWeather);
        _canvas.Sky = new PreviewSky(time.Minute * dayMinutes / 1440, TagOf(_skySeason) ?? "", string.IsNullOrEmpty(weather) ? null : weather);
    }

    private static string? TagOf(ComboBox box) => (box.SelectedItem as ComboBoxItem)?.Tag as string;

    private static void Select(ComboBox box, string? tag) =>
        box.SelectedItem = box.Items.OfType<ComboBoxItem>().FirstOrDefault(item => Equals(item.Tag, tag));
}
