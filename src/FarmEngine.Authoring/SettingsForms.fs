namespace FarmEngine.Authoring

open System
open FarmEngine.Schemas

/// What the Project Settings view decides beyond single fields (web ProjectSettingsEditor.tsx):
/// the weather odds table, the mine card and the season arrows. The view shows these values
/// and hands back what the creator typed; the clamping and the edits are made here.
module SettingsForms =
    let private finite (value: float) = not (Double.IsNaN value || Double.IsInfinity value)

    /// The weight the weather table shows for one season and weather type: 0 without an entry.
    let weatherWeight (project: GameProject) (seasonId: string) (weatherId: string) : float =
        project.Weather.Table
        |> List.tryFind (fun (key, _) -> key = seasonId)
        |> Option.bind (fun (_, entries) -> entries |> List.tryFind (fun e -> e.WeatherId = weatherId))
        |> Option.map (fun e -> e.Weight)
        |> Option.defaultValue 0.0

    /// The weather input's `Math.max(0, parseFloat(value) || 0)`, with non-finite values as 0.
    let cleanWeight (weight: float) : float = if finite weight && weight > 0.0 then weight else 0.0

    /// The weather table saved as ONE undo step: a `SetWeatherWeight` for each cell whose
    /// weight changed, for seasons and weather types the project has. A weight of 0 removes the
    /// entry. Nothing changed → an empty batch, which `Document.apply` treats as a no-op.
    let weatherOdds (project: GameProject) (weights: (string * string * float) list) : Edit =
        let hasSeason id = project.Settings.Calendar.Seasons |> List.exists (fun s -> s.Id = id)
        let hasType id = project.Weather.Types |> List.exists (fun t -> t.Id = id)
        let edits =
            weights
            |> List.choose (fun (seasonId, weatherId, weight) ->
                let weight = cleanWeight weight
                if hasSeason seasonId && hasType weatherId && weatherWeight project seasonId weatherId <> weight then
                    Some(SetWeatherWeight(seasonId, weatherId, weight))
                else None)
        Batch("Weather odds", edits)

    /// The mine card's fields as the web inputs clamp them: the entrance scene must exist (else
    /// the start scene), entrance coordinates are whole and at least 0, floors at least 1, and the
    /// ladder chance between 0.02 and 1 (0.18 when unreadable). The rest of the config is kept.
    let mine (project: GameProject) (enabled: bool) (entranceSceneId: string) (x: float) (y: float) (floors: float) (ladderChance: float) : MineConfig =
        let current = Defaults.mineEnabled project enabled
        let whole fallback minimum (value: float) = if finite value then max minimum (Math.Truncate value) else fallback
        let sceneId =
            if project.Scenes |> List.exists (fun s -> s.Id = entranceSceneId) then entranceSceneId
            else defaultArg current.EntranceSceneId project.StartSceneId
        { current with
            EntranceSceneId = Some sceneId
            EntranceX = Some(whole 0.0 0.0 x)
            EntranceY = Some(whole 0.0 0.0 y)
            Floors = whole 1.0 1.0 floors
            LadderChance = (if finite ladderChance && ladderChance > 0.0 then min 1.0 (max 0.02 ladderChance) else 0.18) }

    /// Whether the season arrows can move `seasonId` by `delta` (the web disables the up arrow
    /// of the first season and the down arrow of the last).
    let canMoveSeason (project: GameProject) (seasonId: string) (delta: int) : bool =
        let seasons = project.Settings.Calendar.Seasons
        match List.tryFindIndex (fun (s: CalendarSeason) -> s.Id = seasonId) seasons with
        | Some index -> delta <> 0 && index + delta >= 0 && index + delta < seasons.Length
        | None -> false

    // ---- The game's words, calendar music, the atmosphere and weather looks (M1, M2) ----

    let private trimmed (text: string | null) =
        match text with
        | null -> ""
        | text -> text.Trim()

    let private optionalText (text: string | null) =
        match trimmed text with
        | "" -> None
        | text -> Some text

    /// A `#rrggbb` (or `#rrggbbaa`) colour as typed, lower-cased, or None when it is not one.
    let color (text: string | null) : string option =
        let text = trimmed text
        if text.StartsWith "#" && (text.Length = 7 || text.Length = 9) && text |> Seq.skip 1 |> Seq.forall Uri.IsHexDigit then
            Some(text.ToLowerInvariant())
        else None

    /// A colour field that may be left empty (None). Anything else that is not a colour is
    /// refused with a `FormatException` naming the field by `label`.
    let optionalColor (label: string) (text: string | null) : string option =
        match trimmed text with
        | "" -> None
        | typed ->
            match color typed with
            | Some value -> Some value
            | None -> raise (FormatException(sprintf "%s must be a colour like #ffd8a0." label))

    /// Day/night keyframes as the editor shows them: one per line, the minute of the day and its
    /// colour ("360 #ffd8a0").
    let daylightText (keys: DaylightKey list) : string =
        keys |> List.map (fun key -> sprintf "%s %s" (JsNumber.format key.Minute) key.Color) |> String.concat "\n"

    /// Day/night keyframes from their text (`daylightText`; commas also separate). Blank lines are
    /// skipped and the keys come out in minute order. A line that is not a minute of 0 or more and
    /// a colour is refused with a `FormatException` naming `label` and the line.
    let parseDaylight (label: string) (text: string | null) : DaylightKey list =
        (match text with
         | null -> ""
         | text -> text).Split('\n')
        |> Array.mapi (fun i line -> i + 1, line.Trim())
        |> Array.filter (fun (_, line) -> line.Length > 0)
        |> Array.map (fun (number, line) ->
            let fail () = raise (FormatException(sprintf "%s line %d needs a minute of the day and a colour, like 360 #ffd8a0." label number))
            match line.Split([| ' '; '\t'; ',' |], StringSplitOptions.RemoveEmptyEntries) with
            | [| minute; tint |] ->
                match Double.TryParse(minute, Globalization.NumberStyles.Float, Globalization.CultureInfo.InvariantCulture), color tint with
                | (true, minute), Some tint when finite minute && minute >= 0.0 -> ({ Minute = minute; Color = tint } : DaylightKey)
                | _ -> fail ()
            | _ -> fail ())
        |> List.ofArray
        |> List.sortBy (fun key -> key.Minute)

    /// An atmosphere look from its fields (`label` names them in errors: "Spring daylight line
    /// 2…"), or None when it sets nothing, so the look below it applies.
    let look (label: string) (daylight: string | null) (foliageTint: string | null) : AtmosphereLook option =
        let keys = parseDaylight (label + " daylight") daylight
        let tint = optionalColor (label + " foliage tint") foliageTint
        if keys.IsEmpty && tint.IsNone then None else Some { Daylight = keys; FoliageTint = tint }

    /// The game's atmosphere from the look of every season and the seasons' own (season id, look),
    /// as one `SetSettings`: looks of seasons the calendar does not have are dropped, and nothing
    /// set at all leaves the key out (the built-in look).
    let atmosphere (project: GameProject) (allSeasons: AtmosphereLook option) (seasons: (string * AtmosphereLook option) list) : Edit =
        let calendar = project.Settings.Calendar.Seasons |> List.map (fun s -> s.Id) |> Set.ofList
        let own = seasons |> List.choose (fun (id, look) -> look |> Option.filter (fun _ -> calendar.Contains id) |> Option.map (fun look -> id, look))
        let settings =
            if allSeasons.IsNone && own.IsEmpty then None
            else Some { AllSeasons = defaultArg allSeasons AtmosphereLook.Default; Seasons = own }
        SetSettings { project.Settings with Atmosphere = settings }

    /// The game's words from the settings fields: the money format (it must hold `{0}` where the
    /// amount goes), the energy word and skill names by skill id. Blank fields keep the built-in
    /// words and the translations (`Locales`) are kept; None when nothing is set.
    let words (current: GameWords option) (money: string | null) (energy: string | null) (skills: (string * string) list) : GameWords option =
        let money = optionalText money
        match money with
        | Some format when not (format.Contains "{0}") -> raise (FormatException "The money format needs {0} where the amount goes, like {0} credits.")
        | _ -> ()
        let skills = skills |> List.choose (fun (id, name) -> optionalText name |> Option.filter (fun _ -> trimmed id <> "") |> Option.map (fun name -> trimmed id, name))
        let next =
            { Money = money; Energy = optionalText energy; Skills = skills
              Locales = current |> Option.map (fun words -> words.Locales) |> Option.defaultValue [] }
        if next = GameWords.Default then None else Some next

    /// A season row of the calendar: an empty music id is none.
    let season (id: string) (name: string) (days: float) (temperature: float option) (music: string | null) : CalendarSeason =
        { Id = id; Name = name; Days = days; Temperature = temperature; Music = optionalText music }

    /// A festival row of the calendar: an empty music id is none.
    let festival (id: string) (name: string) (seasonId: string) (day: float) (music: string | null) : CalendarFestival =
        { Id = id; Name = name; SeasonId = seasonId; Day = day; Music = optionalText music }

    /// A weather overlay from its fields. An empty id, a style that is not one of
    /// `OverlayStyles` or a colour that is not one is refused with a `FormatException` naming it.
    let overlay (id: string) (style: string) (particles: string) (density: float) (speed: float) (tint: string | null) : WeatherOverlayDefinition =
        let id = trimmed id
        if id.Length = 0 then raise (FormatException "Each weather overlay needs an id.")
        let what = sprintf "Overlay \"%s\"" id
        if not (List.contains style OverlayStyles.All) then raise (FormatException(sprintf "%s needs a style: rain, snow, dust or none." what))
        let particles =
            match color particles with
            | Some value -> value
            | None -> raise (FormatException(sprintf "%s colour must be a colour like #ffd8a0." what))
        if not (finite density) || density < 0.0 then raise (FormatException(sprintf "%s density must be 0 or more." what))
        if not (finite speed) || speed < 0.0 then raise (FormatException(sprintf "%s speed must be 0 or more." what))
        { WeatherOverlayDefinition.Default with
            Id = id; Style = style; Color = particles; Density = density; Speed = speed; Tint = optionalColor (what + " tint") tint }

    /// How the weather looks and sounds as ONE `SetWeather`: the game's overlays (an overlay that
    /// was there keeps its undeclared keys; none left removes the key) and each weather type's
    /// overlay and music by weather id (empty: none). Types not listed keep theirs. Two overlays
    /// with one id are refused.
    let weatherLooks (project: GameProject) (overlays: WeatherOverlayDefinition list) (looks: (string * string * string) list) : Edit =
        match overlays |> List.countBy (fun o -> o.Id) |> List.tryFind (fun (_, count) -> count > 1) with
        | Some(id, _) -> raise (FormatException(sprintf "Two weather overlays have the id \"%s\"; overlay ids must be different." id))
        | None -> ()
        let before = defaultArg project.Weather.Overlays []
        let overlays =
            overlays
            |> List.map (fun o ->
                match before |> List.tryFind (fun b -> b.Id = o.Id) with
                | Some b -> { o with Extra = b.Extra }
                | None -> o)
        let types =
            project.Weather.Types
            |> List.map (fun weather ->
                match looks |> List.tryFind (fun (id, _, _) -> id = weather.Id) with
                | Some(_, overlay, music) -> { weather with Overlay = optionalText overlay; Music = optionalText music }
                | None -> weather)
        SetWeather { project.Weather with Types = types; Overlays = (if overlays.IsEmpty then None else Some overlays) }
