namespace FarmEngine.Authoring

open System
open FarmEngine.Schemas

/// Project -> immutable game content. Gameplay state is created by Rust; authored
/// defaults, pack composition and locale selection are resolved here before cartridge writing.
module ContentCompiler =
    let private integer (value: float) = not (Double.IsNaN value || Double.IsInfinity value) && Math.Truncate value = value

    /// A custom crop as the crop definition the engine gets, undeclared fields included. Through
    /// JSON, so every field the two
    /// records share carries over without a hand-kept field list (`Readouts.cropOfCustom` is this
    /// too). Throws InvalidOperationException for a crop the crop decoder refuses.
    let cropOfCustom (crop: CustomCropDefinition) : CropDefinition =
        match Decode.run SchemaJson.decodeCropDefinition (SchemaJson.encodeCustomCropDefinition crop) with
        | Ok definition -> definition
        | Error message -> invalidOp message

    /// Built-in crops with the project's custom crops over them.
    let mergeCrops (custom: CustomCropDefinition list option) : (string * CropDefinition) list =
        (Builtin.crops (), Option.defaultValue [] custom)
        ||> List.fold (fun crops crop ->
            let definition = cropOfCustom crop
            if crops |> List.exists (fun (id, _) -> id = crop.Id) then
                crops |> List.map (fun (id, existing) -> if id = crop.Id then id, definition else id, existing)
            else
                crops @ [ crop.Id, definition ])

    /// `builtins` with the project's definitions over them: one with a built-in's id takes its
    /// place, the others follow in project order (Rust `state::over_builtins`).
    let overBuiltins (idOf: 'T -> string) (builtins: 'T list) (own: 'T list option) : 'T list =
        (builtins, Option.defaultValue [] own)
        ||> List.fold (fun merged definition ->
            let id = idOf definition
            if merged |> List.exists (fun existing -> idOf existing = id) then
                merged |> List.map (fun existing -> if idOf existing = id then definition else existing)
            else
                merged @ [ definition ])

    /// The game's terrains: the built-in ones with the project's over them.
    let terrains (project: GameProject) = overBuiltins (fun (t: TerrainDefinition) -> t.Id) (Builtin.terrains ()) project.Terrains

    /// The game's tools: the built-in ones with the project's over them.
    let tools (project: GameProject) = overBuiltins (fun (t: ToolDefinition) -> t.Id) (Builtin.tools ()) project.Tools

    /// The game's item categories: the built-in ones with the project's over them.
    let itemCategories (project: GameProject) =
        overBuiltins (fun (c: ItemCategoryDefinition) -> c.Id) (Builtin.itemCategories ()) project.ItemCategories

    /// The game's crop quality tiers, ranked: the built-in ones with the project's over them.
    let cropQualities (project: GameProject) =
        overBuiltins (fun (q: CropQualityDefinition) -> q.Id) (Builtin.cropQualities ()) project.CropQualities

    /// The game's crop mutations, in roll order: the built-in ones with the project's over them.
    let cropMutations (project: GameProject) =
        overBuiltins (fun (m: CropMutationDefinition) -> m.Id) (Builtin.cropMutations ()) project.CropMutations

    let private validDays (days: float) = integer days && days > 0.0

    let private validWeather (weather: WeatherConfig) =
        (weather.Types |> List.forall (fun t -> t.CropDamageChance >= 0.0 && t.CropDamageChance <= 1.0))
        && (weather.Table |> List.forall (fun (_, entries) -> entries |> List.forall (fun entry -> entry.Weight > 0.0)))

    /// The project's items, or the built-in catalog when it has none.
    let items (project: GameProject) =
        if not (List.isEmpty project.Items) then project.Items else Builtin.items ()

    /// Built-in and mine node types the project does not replace, then the project's own.
    let nodeTypes (project: GameProject) =
        let ids = project.NodeTypes |> List.map (fun d -> d.Id) |> Set.ofList
        let builtIn = Builtin.nodeTypes () @ Builtin.mineNodeTypes () |> List.filter (fun d -> not (ids.Contains d.Id))
        builtIn @ project.NodeTypes

    /// The project's settings with each invalid part replaced by its default (an invalid day
    /// window or clock rate takes the default time settings) and calendar seasons and festivals
    /// with no valid day count dropped (Rust `state::resolve_settings`). Replacing every setting
    /// when one was invalid lost a whole calendar to one festival on day 0; Problems reports
    /// each replaced value as an error.
    let settings (project: GameProject) =
        let s = project.Settings
        let defaults = SettingsSchema.DefaultProjectSettings
        { s with
            Movement = if not (Double.IsNaN s.Movement.PlayerSpeed) && s.Movement.PlayerSpeed > 0.0 then s.Movement else defaults.Movement
            MaxEnergy = if s.MaxEnergy > 0.0 then s.MaxEnergy else defaults.MaxEnergy
            CollapseEnergyFraction =
                if s.CollapseEnergyFraction >= 0.0 && s.CollapseEnergyFraction <= 1.0 then s.CollapseEnergyFraction else defaults.CollapseEnergyFraction
            CollapseMoneyPenalty = if s.CollapseMoneyPenalty >= 0.0 then s.CollapseMoneyPenalty else defaults.CollapseMoneyPenalty
            Time = if SettingsSchema.validTime s.Time then s.Time else defaults.Time
            Calendar =
                { s.Calendar with
                    Seasons = s.Calendar.Seasons |> List.filter (fun season -> validDays season.Days)
                    Festivals = s.Calendar.Festivals |> List.filter (fun festival -> validDays festival.Day) }
            SkillLevelCurve = if s.SkillLevelCurve |> List.exists Double.IsNaN then defaults.SkillLevelCurve else s.SkillLevelCurve }

    /// The project's weather, or the default weather when it would not load.
    let weather (project: GameProject) =
        if validWeather project.Weather then project.Weather else MigrationsSchema.DefaultWeatherConfig()

    let baseContent (project: GameProject) : GameContent =
        { ContentVersion = GameContentSchema.CurrentContentVersion
          Crops = mergeCrops project.CustomCrops
          Items = items project
          Npcs = project.Npcs
          Dialogues = project.Dialogues
          Quests = project.Quests
          Events = project.Events
          Shops = project.Shops
          NodeTypes = nodeTypes project
          Settings = settings project
          Recipes = project.Recipes
          MachineTypes = project.MachineTypes
          Weather = weather project
          AnimalSpecies = project.AnimalSpecies
          FishTables = project.FishTables
          Mine = project.Mine
          Actions = project.Actions
          Minigames = project.Minigames
          Scenes = project.Scenes
          StartSceneId = project.StartSceneId
          Terrains = terrains project
          Tools = tools project
          ItemCategories = itemCategories project
          CropQualities = cropQualities project
          CropMutations = cropMutations project }

    let compile (project: GameProject) =
        let installs = project.ContentPacks
        let merged, _ = PackMerge.mergeIntoContent (baseContent project) installs
        PackMerge.applyLocaleStrings merged installs merged.Settings.Locale
