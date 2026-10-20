using FarmingRpgMaker.App.Mvvm;

namespace FarmingRpgMaker.App.ViewModels.EditMode;

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

    public SeasonDraft(string id, string name, string days, bool saved)
    {
        _id = id;
        _name = name;
        _days = days;
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
}

/// <summary>A festival row of the calendar draft; <see cref="What"/> names its fields like a season's.</summary>
public sealed class FestivalDraft : DraftRow
{
    private string _id;
    private string _name;
    private string _seasonId;
    private string _day;

    public FestivalDraft(string id, string name, string seasonId, string day)
    {
        _id = id;
        _name = name;
        _seasonId = seasonId;
        _day = day;
        What = string.IsNullOrWhiteSpace(name) ? "new festival" : name;
    }

    public string What { get; }

    public string IdLabel => $"{What} id";

    public string NameLabel => $"{What} name";

    public string SeasonIdLabel => $"{What} season id";

    public string DayLabel => $"{What} day";

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
