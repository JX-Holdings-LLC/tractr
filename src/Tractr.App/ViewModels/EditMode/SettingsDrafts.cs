using Tractr.App.Mvvm;

namespace Tractr.App.ViewModels.EditMode;

/// <summary>A text field of a settings draft row, as typed.</summary>
public abstract class DraftRow : ObservableObject
{
    /// <summary>Sets a text field (null reads as empty, as an empty text box does).</summary>
    protected bool SetText(ref string field, string? value, [System.Runtime.CompilerServices.CallerMemberName] string? propertyName = null) =>
        SetProperty(ref field, value ?? "", propertyName);
}

/// <summary>A skill level's XP, as typed.</summary>
public sealed class SkillLevelDraft(string xp) : DraftRow
{
    private string _xp = xp;

    public string Xp
    {
        get => _xp;
        set => SetText(ref _xp, value);
    }
}

/// <summary>
/// A season row of the calendar draft. <see cref="What"/> (its name when the row was made, or
/// "new season") names its fields in errors and for screen readers; <see cref="Key"/> (its id
/// then) names its buttons.
/// </summary>
public sealed class SeasonDraft : DraftRow
{
    private string _id;
    private string _name;
    private string _days;
    private string _temperature;
    private string _music;

    /// <param name="temperature">How warm the season is, as typed (empty: not set).</param>
    /// <param name="music">The music asset that plays in it ("" for the game's own music).</param>
    public SeasonDraft(string id, string name, string days, bool saved, string temperature = "", string music = "")
    {
        _id = id;
        _name = name;
        _days = days;
        _temperature = temperature;
        _music = music;
        Key = id;
        IsSaved = saved;
        What = string.IsNullOrWhiteSpace(name) ? "new season" : name;
    }

    /// <summary>The id the row was made with (button names follow it).</summary>
    public string Key { get; }

    /// <summary>A saved season's id can't change (only a new one's can).</summary>
    public bool IsSaved { get; }

    /// <summary>What the row's labels call the season.</summary>
    public string What { get; }

    public string IdLabel => $"{What} id";

    public string NameLabel => $"{What} name";

    public string DaysLabel => $"{What} days";

    public string TemperatureLabel => $"{What} temperature";

    public string MusicLabel => $"{What} music";

    public string Id
    {
        get => _id;
        set => SetText(ref _id, value);
    }

    public string Name
    {
        get => _name;
        set => SetText(ref _name, value);
    }

    public string Days
    {
        get => _days;
        set => SetText(ref _days, value);
    }

    public string Temperature
    {
        get => _temperature;
        set => SetText(ref _temperature, value);
    }

    /// <summary>The music asset's id ("" for none).</summary>
    public string Music
    {
        get => _music;
        set => SetText(ref _music, value);
    }
}

/// <summary>A festival row of the calendar draft; <see cref="What"/> names its fields like a season's.</summary>
public sealed class FestivalDraft : DraftRow
{
    private string _id;
    private string _name;
    private string _seasonId;
    private string _day;
    private string _music;

    public FestivalDraft(string id, string name, string seasonId, string day, string music = "")
    {
        _id = id;
        _name = name;
        _seasonId = seasonId;
        _day = day;
        _music = music;
        What = string.IsNullOrWhiteSpace(name) ? "new festival" : name;
    }

    public string What { get; }

    public string IdLabel => $"{What} id";

    public string NameLabel => $"{What} name";

    public string SeasonIdLabel => $"{What} season id";

    public string DayLabel => $"{What} day";

    public string MusicLabel => $"{What} music";

    public string Id
    {
        get => _id;
        set => SetText(ref _id, value);
    }

    public string Name
    {
        get => _name;
        set => SetText(ref _name, value);
    }

    public string SeasonId
    {
        get => _seasonId;
        set => SetText(ref _seasonId, value);
    }

    public string Day
    {
        get => _day;
        set => SetText(ref _day, value);
    }

    /// <summary>The music asset that plays all festival day ("" for none).</summary>
    public string Music
    {
        get => _music;
        set => SetText(ref _music, value);
    }
}

/// <summary>One weather weight: a weather type in a season.</summary>
public sealed class WeatherCellDraft(string seasonId, string weatherId, string weatherName, string seasonName, string weight) : DraftRow
{
    private string _weight = weight;

    public string SeasonId { get; } = seasonId;

    public string WeatherId { get; } = weatherId;

    public string WeatherName { get; } = weatherName;

    /// <summary>"Storm weight in Spring": its errors and screen reader name.</summary>
    public string Label => $"{WeatherName} weight in {SeasonName}";

    /// <summary>"Storm in Spring" (the tooltip).</summary>
    public string Tip => $"{WeatherName} in {SeasonName}";

    public string SeasonName { get; } = seasonName;

    public string Weight
    {
        get => _weight;
        set => SetText(ref _weight, value);
    }
}

/// <summary>A season's row of weather weights.</summary>
public sealed record WeatherSeasonRow(string SeasonName, IReadOnlyList<WeatherCellDraft> Cells);

/// <summary>A scene the mine entrance can be in.</summary>
public sealed record SceneChoice(string Id, string Name);

/// <summary>The name a skill is shown with (empty: its built-in name).</summary>
public sealed class SkillNameDraft(string skillId, string name) : DraftRow
{
    private string _name = name;

    public string SkillId { get; } = skillId;

    /// <summary>"Farming skill name": its screen reader name.</summary>
    public string Label => $"{Capitalized(SkillId)} skill name";

    public string Name
    {
        get => _name;
        set => SetText(ref _name, value);
    }

    private static string Capitalized(string id) => id.Length == 0 ? id : char.ToUpperInvariant(id[0]) + id[1..];
}

/// <summary>
/// A weather overlay row: how a weather looks on screen. <see cref="Key"/> (its id when the row
/// was made) names its controls.
/// </summary>
public sealed class OverlayDraft : DraftRow
{
    private string _id;
    private string _style;
    private string _color;
    private string _density;
    private string _speed;
    private string _tint;

    public OverlayDraft(string id, string style, string color, string density, string speed, string tint)
    {
        _id = id;
        _style = style;
        _color = color;
        _density = density;
        _speed = speed;
        _tint = tint;
        Key = id;
        What = string.IsNullOrWhiteSpace(id) ? "new overlay" : $"overlay {id}";
    }

    public string Key { get; }

    /// <summary>What the row's labels call the overlay.</summary>
    public string What { get; }

    public string IdLabel => $"{Capital(What)} id";

    public string StyleLabel => $"{Capital(What)} style";

    public string ColorLabel => $"{Capital(What)} colour";

    public string DensityLabel => $"{Capital(What)} density";

    public string SpeedLabel => $"{Capital(What)} speed";

    public string TintLabel => $"{Capital(What)} tint";

    public string Id
    {
        get => _id;
        set => SetText(ref _id, value);
    }

    /// <summary>One of the overlay styles (rain, snow, dust, none).</summary>
    public string Style
    {
        get => _style;
        set => SetText(ref _style, value);
    }

    public string Color
    {
        get => _color;
        set => SetText(ref _color, value);
    }

    public string Density
    {
        get => _density;
        set => SetText(ref _density, value);
    }

    public string Speed
    {
        get => _speed;
        set => SetText(ref _speed, value);
    }

    /// <summary>The tint over the scene while it falls (empty: none).</summary>
    public string Tint
    {
        get => _tint;
        set => SetText(ref _tint, value);
    }

    private static string Capital(string text) => char.ToUpperInvariant(text[0]) + text[1..];
}

/// <summary>A weather type's look and sound: the overlay it shows and the music that plays outdoors.</summary>
public sealed class WeatherLookDraft(string weatherId, string weatherName, string overlay, string music) : DraftRow
{
    private string _overlay = overlay;
    private string _music = music;

    public string WeatherId { get; } = weatherId;

    public string WeatherName { get; } = weatherName;

    public string OverlayLabel => $"{WeatherName} overlay";

    public string MusicLabel => $"{WeatherName} music";

    /// <summary>The overlay's id ("" for none).</summary>
    public string Overlay
    {
        get => _overlay;
        set => SetText(ref _overlay, value);
    }

    /// <summary>The music asset's id ("" for none).</summary>
    public string Music
    {
        get => _music;
        set => SetText(ref _music, value);
    }
}

/// <summary>
/// A look of the atmosphere: the day/night keyframes (one "minute #colour" per line) and the
/// foliage tint. <see cref="SeasonId"/> is null for the look of every season.
/// </summary>
public sealed class LookDraft(string? seasonId, string what, string daylight, string foliageTint) : DraftRow
{
    private string _daylight = daylight;
    private string _foliageTint = foliageTint;

    public string? SeasonId { get; } = seasonId;

    /// <summary>"Spring" or "All seasons": names the fields in errors and for screen readers.</summary>
    public string What { get; } = what;

    public string DaylightLabel => $"{What} daylight";

    public string FoliageTintLabel => $"{What} foliage tint";

    public string Daylight
    {
        get => _daylight;
        set => SetText(ref _daylight, value);
    }

    public string FoliageTint
    {
        get => _foliageTint;
        set => SetText(ref _foliageTint, value);
    }
}
