namespace FarmEngine.Authoring

open System.Collections.Generic
open FarmEngine.Schemas

/// Editor-level checks on content: duplicate ids, items, crops, quests, the event/action
/// vocabulary (event-forms.tsx), shops, recipes and machines, gathering nodes, wildlife,
/// artwork (validate-graphics.ts) and packs. Ports `validate-extensibility.ts` and
/// `validate-graphics.ts` and adds the checks the components make before saving.
module internal ChecksContent =
    let private hasValue = Context.hasValue

    /// A non-empty optional reference to something `set` does not contain.
    let private missing (set: HashSet<string>) (id: string option) =
        match id with
        | None -> false
        | Some value -> value.Length > 0 && not (set.Contains value)

    /// A non-empty required reference to something `set` does not contain.
    let private missingText (set: HashSet<string>) (id: string) = id.Length > 0 && not (set.Contains id)

    /// An optional reference as the message prints it.
    let private text (value: string option) = defaultArg value ""

    /// The skills experience can go to: the built-in ones and combat (M7).
    let private knownSkills = HashSet<string>(SaveSchema.SkillNames)

    let private duplicates (context: Context) (sink: Sink) =
        let check (family: string) (path: string) (ids: seq<string>) (target: string -> NavigationTarget option) =
            let seen = HashSet<string>()

            ids
            |> Seq.iteri (fun i id ->
                if id.Length > 0 && not (seen.Add id) then
                    sink.Error(sprintf "duplicate.%s" family, sprintf "%s[%d].id" path i, sprintf "Duplicate %s id \"%s\"" family id, target id))

        let p = context.Project
        check "item" "items" (p.Items |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.Item id))
        check "npc" "npcs" (p.Npcs |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.Npc id))
        check "quest" "quests" (p.Quests |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.Quest id))
        check "event" "events" (p.Events |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.Event id))
        check "dialogue" "dialogues" (p.Dialogues |> Seq.map (fun x -> x.Id)) (fun _ -> None)
        check "shop" "shops" (p.Shops |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.Shop id))
        check "recipe" "recipes" (p.Recipes |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.Recipe id))
        check "nodeType" "nodeTypes" (p.NodeTypes |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.NodeType id))
        check "machineType" "machineTypes" (p.MachineTypes |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.MachineType id))
        check "animalSpecies" "animalSpecies" (p.AnimalSpecies |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.AnimalSpecies id))
        check "animal" "animals" (p.Animals |> Seq.map (fun x -> x.Id)) (fun _ -> None)
        check "fishTable" "fishTables" (p.FishTables |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.FishTable id))
        check "action" "actions" (p.Actions |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.Action id))
        check "minigame" "minigames" (p.Minigames |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.Minigame id))
        check "asset" "customAssets" (p.CustomAssets |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.Asset id))
        check "heartEvent" "heartEvents" (defaultArg p.HeartEvents [] |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.HeartEvent id))
        check "building" "buildingTypes" (defaultArg p.BuildingTypes [] |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.Building id))
        check "placedBuilding" "buildings" (defaultArg p.Buildings [] |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.PlacedBuilding id))
        check "monster" "monsters" (defaultArg p.Monsters [] |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.Monster id))
        check "collection" "collections" (defaultArg p.Collections [] |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.Collection id))
        check "bundle" "bundles" (defaultArg p.Bundles [] |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.Bundle id))
        check "achievement" "achievements" (defaultArg p.Achievements [] |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.Achievement id))
        check "mail" "mail" (defaultArg p.Mail [] |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.Mail id))

        match p.CustomCrops with
        | None -> ()
        | Some crops -> check "crop" "customCrops" (crops |> Seq.map (fun x -> x.Id)) (fun id -> Some(NavigationTarget.Crop id))

    let private items (context: Context) (sink: Sink) =
        let categories = ContentCompiler.itemCategories context.Project

        let kind (item: Item) =
            categories |> List.tryFind (fun c -> c.Id = item.Type)

        let isSeed item =
            kind item |> Option.exists (fun c -> c.Plantable)

        context.Project.Items
        |> Seq.iteri (fun i item ->
            let path = sprintf "items[%d]" i
            let target = Some(NavigationTarget.Item item.Id)

            if System.String.IsNullOrWhiteSpace item.Name then
                sink.Warning("item.emptyName", path + ".name", sprintf "Item \"%s\" has no name" item.Id, target)

            if isSeed item && not (hasValue item.CropType) then
                sink.Warning("item.seedWithoutCrop", path + ".cropType", sprintf "Seed \"%s\" does not say which crop it grows" item.Name, target)

            if
                kind item |> Option.exists (fun c -> c.Equippable)
                && not (hasValue item.ToolType)
            then
                sink.Warning("item.toolWithoutType", path + ".toolType", sprintf "Tool \"%s\" has no tool type, so it cannot be used" item.Name, target)

            if not (isSeed item) && missing context.CropIds item.CropType then
                sink.Warning(
                    "item.unknownCrop",
                    path + ".cropType",
                    sprintf "Item \"%s\" belongs to missing crop \"%s\"" item.Name (text item.CropType),
                    target
                )
            // validate-extensibility.ts: "on use" action must exist.
            if missing context.ActionIds item.UseActionId then
                sink.Error(
                    "item.useActionMissing",
                    path + ".useActionId",
                    sprintf "Item \"%s\" uses missing action \"%s\"" item.Name (text item.UseActionId),
                    target
                ))

    let private crops (context: Context) (sink: Sink) =
        match context.Project.CustomCrops with
        | None -> ()
        | Some crops ->
            let plantable =
                HashSet<string>(
                    ContentCompiler.itemCategories context.Project
                    |> Seq.filter (fun c -> c.Plantable)
                    |> Seq.map (fun c -> c.Id)
                )

            let seedsFor =
                HashSet<string>(
                    context.Project.Items
                    |> Seq.filter (fun i -> plantable.Contains i.Type)
                    |> Seq.choose (fun i -> i.CropType)
                )

            crops
            |> Seq.iteri (fun i crop ->
                let path = sprintf "customCrops[%d]" i
                let target = Some(NavigationTarget.Crop crop.Id)
                // CropEditor handleSaveCrop rules.
                if System.String.IsNullOrWhiteSpace crop.Name then
                    sink.Error("crop.emptyName", path + ".name", sprintf "Crop \"%s\" has no name" crop.Id, target)

                if
                    (match crop.GrowthDays with
                     | Some days -> days <= 0.0
                     | None -> true)
                then
                    sink.Error("crop.growthDays", path + ".growthDays", sprintf "Crop \"%s\" needs at least 1 growth day" crop.Name, target)

                if crop.Stages < 2.0 then
                    sink.Error("crop.stagesTooFew", path + ".stages", sprintf "Crop \"%s\" needs at least 2 stages" crop.Name, target)

                if crop.Seasons.IsEmpty then
                    sink.Error("crop.noSeasons", path + ".seasons", sprintf "Crop \"%s\" grows in no season" crop.Name, target)

                crop.Seasons
                |> Seq.iteri (fun k season ->
                    if not (context.SeasonIds.Contains season) then
                        sink.Warning(
                            "crop.unknownSeason",
                            sprintf "%s.seasons[%d]" path k,
                            sprintf "Crop \"%s\" grows in \"%s\", which is not in the calendar" crop.Name season,
                            target
                        ))

                if not (seedsFor.Contains crop.Id) then
                    sink.Warning("crop.noSeedItem", path, sprintf "Crop \"%s\" has no seed item, so nobody can plant it" crop.Name, target)
                // farm_sim::farming::crops::harvest_item: harvestItemId, else crop-<id> (or
                // pack:crop-<local> for a pack crop).
                match crop.HarvestItemId with
                | Some id when id.Length > 0 ->
                    if not (context.ItemIds.Contains id) then
                        sink.Error(
                            "crop.noHarvestItem",
                            path + ".harvestItemId",
                            sprintf "Crop \"%s\" is harvested as missing item \"%s\"" crop.Name id,
                            target
                        )
                | _ ->
                    let fallback = sprintf "crop-%s" crop.Id

                    let packed =
                        match crop.Id.Split([| ':' |], 2) with
                        | [| pack; local |] when pack.Length > 0 && local.Length > 0 -> Some(sprintf "%s:crop-%s" pack local)
                        | _ -> None

                    if
                        not (
                            context.ItemIds.Contains fallback
                            || packed |> Option.exists context.ItemIds.Contains
                        )
                    then
                        sink.Warning(
                            "crop.noHarvestItem",
                            path + ".harvestItemId",
                            sprintf "Crop \"%s\" has no harvest item (no \"%s\" and no harvest item set), so harvesting it gives nothing" crop.Name fallback,
                            target
                        ))

    /// Birthdays follow the project's calendar: the season must exist and the day must fall
    /// inside it, or the birthday never comes.
    let private npcs (context: Context) (sink: Sink) =
        let seasons = context.Project.Settings.Calendar.Seasons

        context.Project.Npcs
        |> Seq.iteri (fun i npc ->
            match npc.Birthday with
            | None -> ()
            | Some birthday ->
                let path = sprintf "npcs[%d].birthday" i
                let target = Some(NavigationTarget.Npc npc.Id)

                match seasons |> List.tryFind (fun season -> season.Id = birthday.Season) with
                | None ->
                    sink.Warning(
                        "npc.birthdayUnknownSeason",
                        path + ".season",
                        sprintf "NPC \"%s\" has a birthday in \"%s\", which is not in the calendar" npc.Name birthday.Season,
                        target
                    )
                | Some season when birthday.Day < 1.0 || birthday.Day > season.Days ->
                    sink.Warning(
                        "npc.birthdayDay",
                        path + ".day",
                        sprintf "NPC \"%s\" has a birthday on day %g of %s, which only has days 1 to %g" npc.Name birthday.Day season.Name season.Days,
                        target
                    )
                | Some _ -> ())

    let private quests (context: Context) (sink: Sink) =
        context.Project.Quests
        |> Seq.iteri (fun i quest ->
            let path = sprintf "quests[%d]" i
            let target = Some(NavigationTarget.Quest quest.Id)

            if quest.Objectives.IsEmpty then
                sink.Warning("quest.noObjectives", path + ".objectives", sprintf "Quest \"%s\" has no objectives, so it completes at once" quest.Name, target)

            if missing context.NpcIds quest.Giver then
                sink.Error("quest.giverMissing", path + ".giver", sprintf "Quest \"%s\" is given by missing NPC \"%s\"" quest.Name (text quest.Giver), target)

            match quest.Prerequisites with
            | None -> ()
            | Some prerequisites ->
                prerequisites
                |> Seq.iteri (fun k id ->
                    if id = quest.Id then
                        sink.Error("quest.prerequisiteSelf", sprintf "%s.prerequisites[%d]" path k, sprintf "Quest \"%s\" requires itself" quest.Name, target))

            match quest.Rewards.Skill with
            | Some skill when
                skill.Length > 0
                && not (SaveSchema.SkillNames |> List.contains skill)
                && not (
                    defaultArg context.Project.Settings.SkillDefinitions []
                    |> List.exists (fun definition -> definition.Id = skill)
                )
                ->
                sink.Warning(
                    "quest.unknownSkill",
                    path + ".rewards.skill",
                    sprintf "Quest \"%s\" gives experience to unknown skill \"%s\"" quest.Name skill,
                    target
                )
            | _ -> ()

            if missing context.MailIds quest.Rewards.MailId then
                sink.Error(
                    "quest.unknownMail",
                    path + ".rewards.mailId",
                    sprintf "Quest \"%s\" sends missing letter \"%s\"" quest.Name (text quest.Rewards.MailId),
                    target
                )

            match quest.AvailableFromDay, quest.AvailableToDay with
            | Some fromDay, Some toDay when fromDay > toDay ->
                sink.Warning(
                    "quest.availableWindow",
                    path + ".availableFromDay",
                    sprintf "Quest \"%s\" is available from day %g until day %g, which never happens" quest.Name fromDay toDay,
                    target
                )
            | _ -> ()

            match quest.AvailableSeasons with
            | None -> ()
            | Some seasons ->
                seasons
                |> Seq.iteri (fun k season ->
                    if not (context.SeasonIds.Contains season) then
                        sink.Warning(
                            "quest.unknownSeason",
                            sprintf "%s.availableSeasons[%d]" path k,
                            sprintf "Quest \"%s\" is available in \"%s\", which is not in the calendar" quest.Name season,
                            target
                        ))

            quest.Objectives
            |> Seq.iteri (fun k objective ->
                let opath = sprintf "%s.objectives[%d]" path k

                let noTarget (field: string) =
                    sink.Warning(
                        "quest.objectiveNoTarget",
                        opath + "." + field,
                        sprintf "Quest \"%s\" objective %d (%s) has no target yet" quest.Name (k + 1) objective.Type,
                        target
                    )

                if missing context.MonsterIds objective.TargetMonsterId then
                    sink.Error("quest.unknownMonster", opath + ".targetMonsterId", "Quest objective references a missing monster", target)

                if missing context.BundleIds objective.TargetBundleId then
                    sink.Error("quest.unknownBundle", opath + ".targetBundleId", "Quest objective references a missing bundle", target)

                match objective.TargetMineId with
                | Some id when
                    id <> "mine"
                    && not (defaultArg context.Project.Mines [] |> List.exists (fun mine -> mine.Id = id))
                    ->
                    sink.Error("quest.unknownMine", opath + ".targetMineId", "Quest objective references a missing mine", target)
                | _ -> ()

                match objective.Type with
                | QuestObjectiveTypes.Collect
                | QuestObjectiveTypes.Craft
                | QuestObjectiveTypes.Gift
                | QuestObjectiveTypes.Ship
                | QuestObjectiveTypes.Catch when not (hasValue objective.TargetItemId) -> noTarget "targetItemId"
                | QuestObjectiveTypes.Harvest when not (hasValue objective.TargetCropType) -> noTarget "targetCropType"
                | QuestObjectiveTypes.Talk
                | QuestObjectiveTypes.Hearts when not (hasValue objective.TargetNpcId) -> noTarget "targetNPCId"
                | QuestObjectiveTypes.Visit when not (hasValue objective.TargetSceneId) -> noTarget "targetSceneId"
                | QuestObjectiveTypes.Slay when not (hasValue objective.TargetMonsterId) -> noTarget "targetMonsterId"
                | QuestObjectiveTypes.Bundle when not (hasValue objective.TargetBundleId) -> noTarget "targetBundleId"
                | _ -> ()))

    /// The condition/outcome vocabulary shared by events, actions and minigame tiers (event-forms.tsx).
    /// `sceneId` is the scene tile conditions and tile outcomes are checked against (empty = any).
    let rec private conditions
        (context: Context)
        (sink: Sink)
        (family: string)
        (path: string)
        (label: string)
        (sceneId: string)
        (target: NavigationTarget option)
        (list: EventCondition list)
        =
        list
        |> Seq.iteri (fun c condition ->
            let cpath = sprintf "%s.conditions[%d]" path c

            let unknownNpc (npcId: string) (what: string) =
                if npcId.Length > 0 && not (context.NpcIds.Contains npcId) then
                    sink.Error(
                        sprintf "%s.conditionUnknownNpc" family,
                        cpath + ".npcId",
                        sprintf "%s checks %s with missing NPC \"%s\"" label what npcId,
                        target
                    )

            let tile (x: float) (y: float) =
                if
                    sceneId.Length > 0
                    && context.Scenes.ContainsKey sceneId
                    && not (Context.tileInScene context sceneId x y)
                then
                    sink.Error(
                        sprintf "%s.conditionTileOutOfBounds" family,
                        cpath + ".x",
                        sprintf "%s watches tile (%g,%g), outside its scene" label x y,
                        target
                    )

            match condition with
            | EventCondition.MineFloor m ->
                match m.MineId with
                | Some id when
                    id <> "mine"
                    && not (defaultArg context.Project.Mines [] |> List.exists (fun mine -> mine.Id = id))
                    ->
                    sink.Error("condition.unknownMine", cpath + ".mineId", "Condition references a missing mine", target)
                | _ -> ()
            | EventCondition.EnterTile t -> tile t.X t.Y
            | EventCondition.InteractTile t -> tile t.X t.Y
            | EventCondition.Friendship f ->
                if missingText context.NpcIds f.NpcId then
                    sink.Error(
                        sprintf "%s.conditionUnknownNpc" family,
                        cpath + ".npcId",
                        sprintf "%s checks friendship with missing NPC \"%s\"" label f.NpcId,
                        target
                    )
            | EventCondition.Flag f ->
                if f.Flag.Length > 0 && not (context.KnownFlags.Contains f.Flag) then
                    sink.Warning(
                        sprintf "%s.conditionUnknownFlag" family,
                        cpath + ".flag",
                        sprintf "%s checks flag \"%s\", which nothing ever sets" label f.Flag,
                        target
                    )
            | EventCondition.Weather w ->
                w.WeatherIds
                |> Seq.iteri (fun k id ->
                    if not (context.WeatherIds.Contains id) then
                        sink.Warning(
                            sprintf "%s.conditionUnknownWeather" family,
                            sprintf "%s.weatherIds[%d]" cpath k,
                            sprintf "%s checks unknown weather \"%s\"" label id,
                            target
                        ))
            | EventCondition.Season s ->
                s.Seasons
                |> Seq.iteri (fun k id ->
                    if not (context.SeasonIds.Contains id) then
                        sink.Warning(
                            sprintf "%s.conditionUnknownSeason" family,
                            sprintf "%s.seasons[%d]" cpath k,
                            sprintf "%s checks season \"%s\", which is not in the calendar" label id,
                            target
                        ))
            | EventCondition.Weekday w ->
                let ids =
                    context.Project.Settings.Calendar.Weekdays
                    |> Option.defaultValue []
                    |> List.map (fun d -> d.Id)
                    |> Set.ofList

                w.WeekdayIds
                |> List.iteri (fun k id ->
                    if not (ids.Contains id) then
                        sink.Error(
                            sprintf "%s.conditionUnknownWeekday" family,
                            sprintf "%s.weekdayIds[%d]" cpath k,
                            sprintf "%s checks missing weekday \"%s\"" label id,
                            target
                        ))
            | EventCondition.FestivalId f ->
                if missingText context.FestivalIds f.FestivalId then
                    sink.Error(
                        sprintf "%s.conditionUnknownFestival" family,
                        cpath + ".festivalId",
                        sprintf "%s checks missing festival \"%s\"" label f.FestivalId,
                        target
                    )
            | EventCondition.InventorySpace i ->
                if missingText context.ItemIds i.ItemId then
                    sink.Error(
                        sprintf "%s.conditionUnknownItem" family,
                        cpath + ".itemId",
                        sprintf "%s checks room for missing item \"%s\"" label i.ItemId,
                        target
                    )
            // Event conditions on items and quests are content lints (validation.ts); actions get them here.
            | EventCondition.HasItem h when family <> "event" ->
                if missingText context.ItemIds h.ItemId then
                    sink.Error(sprintf "%s.conditionUnknownItem" family, cpath + ".itemId", sprintf "%s checks missing item \"%s\"" label h.ItemId, target)
            | EventCondition.QuestStatus q when family <> "event" ->
                if missingText context.QuestIds q.QuestId then
                    sink.Error(sprintf "%s.conditionUnknownQuest" family, cpath + ".questId", sprintf "%s checks missing quest \"%s\"" label q.QuestId, target)
            // M3 and M4: groups are checked inside; variables are flags.
            | EventCondition.All g -> conditions context sink family cpath label sceneId target g.Conditions
            | EventCondition.Any g -> conditions context sink family cpath label sceneId target g.Conditions
            | EventCondition.Not g -> conditions context sink family cpath label sceneId target g.Conditions
            | EventCondition.Variable v ->
                if v.Name.Length > 0 && not (context.KnownFlags.Contains v.Name) then
                    sink.Warning(
                        sprintf "%s.conditionUnknownVariable" family,
                        cpath + ".name",
                        sprintf "%s checks variable \"%s\", which nothing ever sets" label v.Name,
                        target
                    )
            | EventCondition.Scene s ->
                s.SceneIds
                |> Seq.iteri (fun k id ->
                    if not (context.Scenes.ContainsKey id) then
                        sink.Warning(
                            sprintf "%s.conditionUnknownScene" family,
                            sprintf "%s.sceneIds[%d]" cpath k,
                            sprintf "%s checks missing scene \"%s\"" label id,
                            target
                        ))
            | EventCondition.Hearts h -> unknownNpc h.NpcId "hearts"
            | EventCondition.Relationship r -> unknownNpc r.NpcId "the relationship"
            // M5–M7: what the new conditions name must exist.
            | EventCondition.Birthday b -> unknownNpc b.NpcId "the birthday"
            | EventCondition.Perk _ -> ()
            | EventCondition.Buff s ->
                if System.String.IsNullOrWhiteSpace s.Id then
                    sink.Error(sprintf "%s.conditionBuffId" family, cpath + ".id", "Buff id is required", target)
            | EventCondition.SkillLevel s ->
                if
                    s.Skill.Length > 0
                    && not (knownSkills.Contains s.Skill)
                    && not (
                        defaultArg context.Project.Settings.SkillDefinitions []
                        |> List.exists (fun skill -> skill.Id = s.Skill)
                    )
                then
                    sink.Warning(sprintf "%s.conditionUnknownSkill" family, cpath + ".skill", sprintf "%s checks unknown skill \"%s\"" label s.Skill, target)
            | EventCondition.Collection k ->
                match
                    defaultArg context.Project.Collections []
                    |> List.tryFind (fun c -> c.Id = k.CollectionId)
                with
                | None ->
                    if k.CollectionId.Length > 0 then
                        sink.Error(
                            sprintf "%s.conditionUnknownCollection" family,
                            cpath + ".collectionId",
                            sprintf "%s checks missing collection \"%s\"" label k.CollectionId,
                            target
                        )
                | Some collection ->
                    match k.Min with
                    | Some min when min > float collection.Items.Length ->
                        sink.Warning(
                            sprintf "%s.conditionCollectionMin" family,
                            cpath + ".min",
                            sprintf "%s waits for %g items of \"%s\", which only has %d, so it never holds" label min collection.Name collection.Items.Length,
                            target
                        )
                    | _ -> ()
            | EventCondition.Bundle b ->
                if missingText context.BundleIds b.BundleId then
                    sink.Error(
                        sprintf "%s.conditionUnknownBundle" family,
                        cpath + ".bundleId",
                        sprintf "%s checks missing bundle \"%s\"" label b.BundleId,
                        target
                    )
            | EventCondition.Achievement a ->
                if missingText context.AchievementIds a.AchievementId then
                    sink.Error(
                        sprintf "%s.conditionUnknownAchievement" family,
                        cpath + ".achievementId",
                        sprintf "%s checks missing achievement \"%s\"" label a.AchievementId,
                        target
                    )
            | EventCondition.Shipped s ->
                if missingText context.ItemIds s.ItemId then
                    sink.Error(
                        sprintf "%s.conditionUnknownItem" family,
                        cpath + ".itemId",
                        sprintf "%s counts shipments of missing item \"%s\"" label s.ItemId,
                        target
                    )
            | EventCondition.Building b ->
                if missingText context.BuildingIds b.BuildingId then
                    sink.Error(
                        sprintf "%s.conditionUnknownBuilding" family,
                        cpath + ".buildingId",
                        sprintf "%s checks missing building \"%s\"" label b.BuildingId,
                        target
                    )
            | EventCondition.Slain s ->
                if missingText context.MonsterIds s.MonsterId then
                    sink.Error(
                        sprintf "%s.conditionUnknownMonster" family,
                        cpath + ".monsterId",
                        sprintf "%s counts slain missing monster \"%s\"" label s.MonsterId,
                        target
                    )
            | _ -> ())

    let private outcomes
        (context: Context)
        (sink: Sink)
        (family: string)
        (path: string)
        (label: string)
        (sceneId: string)
        (target: NavigationTarget option)
        (outcomes: EventOutcome list)
        =
        outcomes
        |> Seq.iteri (fun o outcome ->
            let opath = sprintf "%s.outcomes[%d]" path o

            let needs (field: string) (present: bool) =
                if not present then
                    sink.Error(
                        sprintf "%s.outcomeMissingField" family,
                        opath + "." + field,
                        sprintf "%s has a %s outcome with no %s" label outcome.Type field,
                        target
                    )

            let sceneFor (id: string option) =
                if hasValue id then text id else sceneId
            // Event outcomes on items, quests, NPCs and warps are content lints (validation.ts);
            // actions and minigame tiers get the same checks here.
            let unknown (code: string) (field: string) (known: string -> bool) (id: string option) (what: string) =
                if
                    family <> "event"
                    && (match id with
                        | None -> false
                        | Some id -> id.Length > 0 && not (known id))
                then
                    sink.Error(sprintf "%s.%s" family code, opath + "." + field, sprintf "%s references missing %s \"%s\"" label what (text id), target)

            let unknownScene () =
                if
                    (match outcome.SceneId with
                     | None -> false
                     | Some id -> id.Length > 0 && not (context.Scenes.ContainsKey id))
                then
                    sink.Error(
                        sprintf "%s.outcomeUnknownScene" family,
                        opath + ".sceneId",
                        sprintf "%s %s in missing scene \"%s\"" label outcome.Type (text outcome.SceneId),
                        target
                    )

            match outcome.Type with
            | EventOutcomeTypes.Message -> needs "message" (hasValue outcome.Message)
            | EventOutcomeTypes.GiveItem
            | EventOutcomeTypes.TakeItem ->
                needs "itemId" (hasValue outcome.ItemId)
                unknown "outcomeUnknownItem" "itemId" context.ItemIds.Contains outcome.ItemId "item"
            | EventOutcomeTypes.GiveMoney
            | EventOutcomeTypes.TakeMoney
            | EventOutcomeTypes.ModifyEnergy -> needs "amount" outcome.Amount.IsSome
            | EventOutcomeTypes.ModifyFriendship ->
                needs "npcId" (hasValue outcome.NpcId)
                needs "amount" outcome.Amount.IsSome

                if missing context.NpcIds outcome.NpcId then
                    sink.Error(
                        sprintf "%s.outcomeUnknownNpc" family,
                        opath + ".npcId",
                        sprintf "%s changes friendship with missing NPC \"%s\"" label (text outcome.NpcId),
                        target
                    )
            | EventOutcomeTypes.SetFlag
            | EventOutcomeTypes.ClearFlag -> needs "flagName" (hasValue outcome.FlagName)
            | EventOutcomeTypes.StartQuest
            | EventOutcomeTypes.CompleteQuest ->
                needs "questId" (hasValue outcome.QuestId)
                unknown "outcomeUnknownQuest" "questId" context.QuestIds.Contains outcome.QuestId "quest"
            | EventOutcomeTypes.SpawnNpc
            | EventOutcomeTypes.RemoveNpc ->
                needs "npcId" (hasValue outcome.NpcId)
                unknown "outcomeUnknownNpc" "npcId" context.NpcIds.Contains outcome.NpcId "NPC"
            | EventOutcomeTypes.StartDialogue ->
                needs "npcId" (hasValue outcome.NpcId)
                unknown "outcomeUnknownNpc" "npcId" context.NpcIds.Contains outcome.NpcId "NPC"

                match outcome.NpcId with
                | None -> ()
                | Some npcId ->
                    match outcome.DialogueId with
                    | None -> ()
                    | Some dialogueId when dialogueId.Length > 0 ->
                        match context.Project.Npcs |> Seq.tryFind (fun n -> n.Id = npcId) with
                        | Some npc when not (npc.Dialogue |> Seq.exists (fun d -> d.Id = dialogueId)) ->
                            sink.Error(
                                sprintf "%s.outcomeUnknownDialogue" family,
                                opath + ".dialogueId",
                                sprintf "%s starts dialogue \"%s\", which %s does not have" label dialogueId npc.Name,
                                target
                            )
                        | _ -> ()
                    | _ -> ()
            | EventOutcomeTypes.ChangeTile ->
                needs "newTileType" (hasValue outcome.NewTileType)
                unknownScene ()
                let scene = sceneFor outcome.SceneId
                let x = defaultArg outcome.TileX 0.0
                let y = defaultArg outcome.TileY 0.0

                if
                    scene.Length > 0
                    && context.Scenes.ContainsKey scene
                    && not (Context.tileInScene context scene x y)
                then
                    sink.Error(
                        sprintf "%s.outcomeTileOutOfBounds" family,
                        opath + ".tileX",
                        sprintf "%s changes tile (%g,%g), outside the scene" label x y,
                        target
                    )
            | EventOutcomeTypes.WarpPlayer ->
                needs "sceneId" (hasValue outcome.SceneId)
                unknown "outcomeUnknownScene" "sceneId" context.Scenes.ContainsKey outcome.SceneId "scene"
                let x = defaultArg outcome.X 0.0
                let y = defaultArg outcome.Y 0.0

                match outcome.SceneId with
                | None -> ()
                | Some scene when context.Scenes.ContainsKey scene && not (Context.tileInScene context scene x y) ->
                    sink.Error(
                        sprintf "%s.outcomeTileOutOfBounds" family,
                        opath + ".x",
                        sprintf "%s warps the player to (%g,%g), outside \"%s\"" label x y context.Scenes.[scene].Name,
                        target
                    )
                | Some scene when context.Scenes.ContainsKey scene ->
                    match ChecksWorld.tileAt context.Scenes.[scene] x y with
                    | Some tile when ChecksWorld.tileBlocks context.Project tile ->
                        sink.Warning(
                            sprintf "%s.outcomeWarpBlocked" family,
                            opath + ".x",
                            sprintf
                                "%s warps the player onto a blocked tile (%g,%g) in \"%s\"; the game moves the player to the nearest open tile"
                                label
                                x
                                y
                                context.Scenes.[scene].Name,
                            target
                        )
                    | _ -> ()
                | _ -> ()
            | EventOutcomeTypes.LockTransition
            | EventOutcomeTypes.UnlockTransition ->
                unknownScene ()
                let scene = sceneFor outcome.SceneId
                let x = defaultArg outcome.X 0.0
                let y = defaultArg outcome.Y 0.0

                match context.Scenes.TryGetValue scene with
                | true, s when not (s.Transitions |> Seq.exists (fun t -> t.FromX = x && t.FromY = y)) ->
                    sink.Warning(
                        sprintf "%s.outcomeUnknownTransition" family,
                        opath + ".x",
                        sprintf
                            "%s %ss a transition at (%g,%g) in \"%s\", but there is none"
                            label
                            (if outcome.Type = EventOutcomeTypes.LockTransition then
                                 "lock"
                             else
                                 "unlock")
                            x
                            y
                            s.Name,
                        target
                    )
                | _ -> ()
            | EventOutcomeTypes.PlaySound -> needs "soundId" (hasValue outcome.SoundId)
            // validate-extensibility.ts: performAction / startMinigame targets must exist.
            | EventOutcomeTypes.PerformAction ->
                needs "actionId" (hasValue outcome.ActionId)

                if missing context.ActionIds outcome.ActionId then
                    sink.Error(
                        sprintf "%s.outcomeUnknownAction" family,
                        opath + ".actionId",
                        sprintf "%s performs missing action \"%s\"" label (text outcome.ActionId),
                        target
                    )
            | EventOutcomeTypes.StartMinigame ->
                needs "minigameId" (hasValue outcome.MinigameId)

                if missing context.MinigameIds outcome.MinigameId then
                    sink.Error(
                        sprintf "%s.outcomeUnknownMinigame" family,
                        opath + ".minigameId",
                        sprintf "%s starts missing minigame \"%s\"" label (text outcome.MinigameId),
                        target
                    )
            // M3 and M4 outcomes: what they need and what they name.
            | EventOutcomeTypes.SetVariable
            | EventOutcomeTypes.AddVariable -> needs "variable" (hasValue outcome.Variable)
            | EventOutcomeTypes.MoveNpc
            | EventOutcomeTypes.SetRelationship ->
                needs "npcId" (hasValue outcome.NpcId)

                if missing context.NpcIds outcome.NpcId then
                    sink.Error(
                        sprintf "%s.outcomeUnknownNpc" family,
                        opath + ".npcId",
                        sprintf "%s moves or changes missing NPC \"%s\"" label (text outcome.NpcId),
                        target
                    )

                if outcome.Type = EventOutcomeTypes.MoveNpc then
                    unknownScene ()
                else
                    needs "status" (hasValue outcome.Status)
            | EventOutcomeTypes.SetWeather ->
                needs "weatherId" (hasValue outcome.WeatherId)

                if missing context.WeatherIds outcome.WeatherId then
                    sink.Error(
                        sprintf "%s.outcomeUnknownWeather" family,
                        opath + ".weatherId",
                        sprintf "%s sets missing weather \"%s\"" label (text outcome.WeatherId),
                        target
                    )
            | EventOutcomeTypes.GiveExperience ->
                needs "skill" (hasValue outcome.Skill)
                needs "amount" outcome.Amount.IsSome
            | EventOutcomeTypes.AddShopStock
            | EventOutcomeTypes.RemoveShopStock ->
                needs "shopId" (hasValue outcome.ShopId)
                needs "itemId" (hasValue outcome.ItemId)

                if missing context.ShopIds outcome.ShopId then
                    sink.Error(
                        sprintf "%s.outcomeUnknownShop" family,
                        opath + ".shopId",
                        sprintf "%s changes missing shop \"%s\"" label (text outcome.ShopId),
                        target
                    )

                if missing context.ItemIds outcome.ItemId then
                    sink.Error(
                        sprintf "%s.outcomeUnknownItem" family,
                        opath + ".itemId",
                        sprintf "%s stocks missing item \"%s\"" label (text outcome.ItemId),
                        target
                    )
            | EventOutcomeTypes.PlaceNode
            | EventOutcomeTypes.RemoveNode
            | EventOutcomeTypes.PlaceMachine
            | EventOutcomeTypes.RemoveMachine ->
                unknownScene ()

                if outcome.Type = EventOutcomeTypes.PlaceNode then
                    needs "nodeTypeId" (hasValue outcome.NodeTypeId)

                    if missing context.NodeTypeIds outcome.NodeTypeId then
                        sink.Error(
                            sprintf "%s.outcomeUnknownNodeType" family,
                            opath + ".nodeTypeId",
                            sprintf "%s places missing node type \"%s\"" label (text outcome.NodeTypeId),
                            target
                        )

                if outcome.Type = EventOutcomeTypes.PlaceMachine then
                    needs "machineTypeId" (hasValue outcome.MachineTypeId)

                    if missing context.MachineTypeIds outcome.MachineTypeId then
                        sink.Error(
                            sprintf "%s.outcomeUnknownMachineType" family,
                            opath + ".machineTypeId",
                            sprintf "%s places missing machine type \"%s\"" label (text outcome.MachineTypeId),
                            target
                        )

                let scene = sceneFor outcome.SceneId
                let x = defaultArg outcome.TileX 0.0
                let y = defaultArg outcome.TileY 0.0

                if
                    scene.Length > 0
                    && context.Scenes.ContainsKey scene
                    && not (Context.tileInScene context scene x y)
                then
                    sink.Error(
                        sprintf "%s.outcomeTileOutOfBounds" family,
                        opath + ".tileX",
                        sprintf "%s uses tile (%g,%g), outside the scene" label x y,
                        target
                    )
            // M6 and M7 outcomes.
            | EventOutcomeTypes.SendMail ->
                needs "mailId" (hasValue outcome.MailId)

                if missing context.MailIds outcome.MailId then
                    sink.Error(
                        sprintf "%s.outcomeUnknownMail" family,
                        opath + ".mailId",
                        sprintf "%s sends missing letter \"%s\"" label (text outcome.MailId),
                        target
                    )
            | EventOutcomeTypes.SetInventorySize -> needs "amount" outcome.Amount.IsSome
            | EventOutcomeTypes.AddInventorySize -> needs "amount" outcome.Amount.IsSome
            | EventOutcomeTypes.AddMaxEnergy -> needs "amount" outcome.Amount.IsSome
            | EventOutcomeTypes.AddMaxHealth -> needs "amount" outcome.Amount.IsSome
            | EventOutcomeTypes.ApplyBuff -> needs "buff" outcome.Buff.IsSome
            | EventOutcomeTypes.ModifyHealth -> needs "amount" outcome.Amount.IsSome
            | _ -> ())

    let private events (context: Context) (sink: Sink) =
        context.Project.Events
        |> Seq.iteri (fun i event ->
            let path = sprintf "events[%d]" i
            let label = sprintf "Event \"%s\"" event.Name
            let target = Some(NavigationTarget.Event event.Id)

            if event.Outcomes.IsEmpty then
                sink.Warning("event.noOutcomes", path + ".outcomes", sprintf "%s does nothing (no outcomes)" label, target)

            let hasTile =
                event.Conditions
                |> List.exists (fun c ->
                    match c with
                    | EventCondition.EnterTile _
                    | EventCondition.InteractTile _ -> true
                    | _ -> false)

            if event.Trigger = EventTriggers.Enter && not hasTile then
                sink.Warning(
                    "event.enterWithoutTile",
                    path + ".conditions",
                    sprintf "%s fires on enter but has no tile or region condition, so it fires on every step" label,
                    target
                )

            if event.Trigger = EventTriggers.Interact && not hasTile then
                sink.Warning(
                    "event.interactWithoutTile",
                    path + ".conditions",
                    sprintf "%s fires on interact but has no tile or region condition" label,
                    target
                )
            // M3 triggers: a time event needs its minute; a subject names an item, NPC or quest.
            if event.Trigger = EventTriggers.Time && event.Minute.IsNone then
                sink.Error("event.timeWithoutMinute", path + ".minute", sprintf "%s fires at a time of day but has no minute" label, target)

            match event.Subject with
            | Some subject when subject.Length > 0 ->
                let known =
                    match event.Trigger with
                    | EventTriggers.Friendship -> context.NpcIds.Contains subject
                    | EventTriggers.Quest -> context.QuestIds.Contains subject
                    | EventTriggers.Harvest -> context.ItemIds.Contains subject || context.CropIds.Contains subject
                    | EventTriggers.Craft ->
                        context.ItemIds.Contains subject
                        || context.Project.Recipes |> List.exists (fun r -> r.Id = subject)
                    | EventTriggers.Sell
                    | EventTriggers.Catch
                    | EventTriggers.PickUp -> context.ItemIds.Contains subject
                    | EventTriggers.Build -> context.BuildingIds.Contains subject
                    | EventTriggers.Bundle -> context.BundleIds.Contains subject
                    | EventTriggers.Achievement -> context.AchievementIds.Contains subject
                    | EventTriggers.Slay -> context.MonsterIds.Contains subject
                    | _ -> true

                if not known then
                    sink.Warning("event.unknownSubject", path + ".subject", sprintf "%s waits for \"%s\", which the game does not have" label subject, target)
            | _ -> ()

            conditions context sink "event" path label event.SceneId target event.Conditions
            outcomes context sink "event" path label event.SceneId target event.Outcomes)

    /// Heart events (M4): their NPC, cutscene and scene must exist, and their conditions are the events'.
    let private heartEvents (context: Context) (sink: Sink) =
        defaultArg context.Project.HeartEvents []
        |> Seq.iteri (fun i heartEvent ->
            let path = sprintf "heartEvents[%d]" i
            let label = sprintf "Heart event \"%s\"" heartEvent.Name
            let target = Some(NavigationTarget.HeartEvent heartEvent.Id)

            if not (context.NpcIds.Contains heartEvent.NpcId) then
                sink.Error("heartEvent.unknownNpc", path + ".npcId", sprintf "%s is for missing NPC \"%s\"" label heartEvent.NpcId, target)

            if not (context.CutsceneIds.Contains heartEvent.CutsceneId) then
                sink.Error("heartEvent.unknownCutscene", path + ".cutsceneId", sprintf "%s plays missing cutscene \"%s\"" label heartEvent.CutsceneId, target)

            match heartEvent.SceneId with
            | Some scene when scene.Length > 0 && not (context.Scenes.ContainsKey scene) ->
                sink.Error("heartEvent.unknownScene", path + ".sceneId", sprintf "%s waits in missing scene \"%s\"" label scene, target)
            | _ -> ()

            conditions context sink "heartEvent" path label "" target heartEvent.Conditions)

    /// Daily lines (M4): their conditions are the events'.
    let private lines (context: Context) (sink: Sink) =
        context.Project.Npcs
        |> Seq.iteri (fun i npc ->
            let variants = defaultArg npc.Schedules []

            variants
            |> List.iteri (fun k variant ->
                let path = sprintf "npcs[%d].schedules[%d]" i k

                conditions
                    context
                    sink
                    "npc"
                    path
                    (sprintf "NPC %s schedule %s" npc.Name variant.Name)
                    ""
                    (Some(NavigationTarget.Npc npc.Id))
                    variant.Conditions

                if
                    variants
                    |> List.take k
                    |> List.exists (fun earlier -> earlier.Conditions.IsEmpty)
                then
                    sink.Warning("npc.scheduleShadowed", path, "An earlier unconditional schedule always wins", Some(NavigationTarget.Npc npc.Id)))

            defaultArg npc.Lines []
            |> Seq.iteri (fun k line ->
                let path = sprintf "npcs[%d].lines[%d]" i k
                conditions context sink "npc" path (sprintf "NPC \"%s\" line %d" npc.Name (k + 1)) "" (Some(NavigationTarget.Npc npc.Id)) line.Conditions))

    /// The relationship rules (M4): the gifts, the wedding cutscene and the home must exist.
    let private relationships (context: Context) (sink: Sink) =
        match context.Project.Settings.Relationships with
        | None -> ()
        | Some rules ->
            let target = Some NavigationTarget.Settings

            let item (field: string) (id: string option) =
                if missing context.ItemIds id then
                    sink.Error(
                        "relationships.unknownItem",
                        "settings.relationships." + field,
                        sprintf "The relationship rules name missing item \"%s\"" (text id),
                        target
                    )

            item "datingItemId" rules.DatingItemId
            item "proposalItemId" rules.ProposalItemId

            rules.SpouseGiftItemIds
            |> List.iteri (fun k id -> item (sprintf "spouseGiftItemIds[%d]" k) (Some id))

            if missing context.CutsceneIds rules.WeddingCutsceneId then
                sink.Error(
                    "relationships.unknownCutscene",
                    "settings.relationships.weddingCutsceneId",
                    sprintf "The wedding plays missing cutscene \"%s\"" (text rules.WeddingCutsceneId),
                    target
                )

            if missing (HashSet<string>(context.Scenes.Keys)) rules.HomeSceneId then
                sink.Error(
                    "relationships.unknownScene",
                    "settings.relationships.homeSceneId",
                    sprintf "The spouse's home is missing scene \"%s\"" (text rules.HomeSceneId),
                    target
                )

            if rules.Marriage && not (hasValue rules.ProposalItemId) then
                sink.Warning(
                    "relationships.noProposal",
                    "settings.relationships.proposalItemId",
                    "Marriage is on but no item proposes, so no one can marry",
                    target
                )

            if
                (rules.Dating || rules.Marriage)
                && not (context.Project.Npcs |> List.exists (fun n -> n.Romanceable = Some true))
            then
                sink.Warning("relationships.noRomanceable", "settings.relationships", "Dating or marriage is on but no NPC is romanceable", target)

    /// Collections (M6): their items and donation scene exist, the kind is one of
    /// `CollectionKinds`, and each reward can be reached; reward outcomes are the events'.
    let private collections (context: Context) (sink: Sink) =
        defaultArg context.Project.Collections []
        |> List.iteri (fun i collection ->
            let path = sprintf "collections[%d]" i
            let label = sprintf "Collection \"%s\"" collection.Name
            let target = Some(NavigationTarget.Collection collection.Id)

            if collection.Items.IsEmpty then
                sink.Warning("collection.noItems", path + ".items", sprintf "%s holds no items" label, target)

            collection.Items
            |> List.iteri (fun k id ->
                if not (context.ItemIds.Contains id) then
                    sink.Error("collection.unknownItem", sprintf "%s.items[%d]" path k, sprintf "%s lists missing item \"%s\"" label id, target))

            if not (List.contains collection.Kind CollectionKinds.All) then
                sink.Error(
                    "collection.unknownKind",
                    path + ".kind",
                    sprintf "%s has unknown kind \"%s\" (found, shipped or donated)" label collection.Kind,
                    target
                )

            if missing (HashSet<string>(context.Scenes.Keys)) collection.SceneId then
                sink.Error(
                    "collection.unknownScene",
                    path + ".sceneId",
                    sprintf "%s takes donations in missing scene \"%s\"" label (text collection.SceneId),
                    target
                )

            collection.Rewards
            |> List.iteri (fun k reward ->
                let rp = sprintf "%s.rewards[%d]" path k

                if reward.Count > float collection.Items.Length then
                    sink.Warning(
                        "collection.rewardUnreachable",
                        rp + ".count",
                        sprintf "%s rewards %g items but only has %d, so the reward never comes" label reward.Count collection.Items.Length,
                        target
                    )

                outcomes context sink "collection" rp (sprintf "%s reward %d" label (k + 1)) "" target reward.Outcomes))

    /// Bundles (M6): their lines name items and qualities the game has, `required` is within the
    /// lines, and their outcomes are the events'.
    let private bundles (context: Context) (sink: Sink) =
        defaultArg context.Project.Bundles []
        |> List.iteri (fun i bundle ->
            let path = sprintf "bundles[%d]" i
            let label = sprintf "Bundle \"%s\"" bundle.Name
            let target = Some(NavigationTarget.Bundle bundle.Id)

            if bundle.Items.IsEmpty then
                sink.Warning("bundle.noItems", path + ".items", sprintf "%s asks for nothing, so it can never be handed in" label, target)

            bundle.Items
            |> List.iteri (fun k line ->
                let lp = sprintf "%s.items[%d]" path k

                if not (context.ItemIds.Contains line.ItemId) then
                    sink.Error("bundle.unknownItem", lp + ".itemId", sprintf "%s asks for missing item \"%s\"" label line.ItemId, target)

                if missing context.QualityIds line.Quality then
                    sink.Error("bundle.unknownQuality", lp + ".quality", sprintf "%s asks for missing crop quality \"%s\"" label (text line.Quality), target)

                if line.Quantity < 1.0 then
                    sink.Error("bundle.quantity", lp + ".quantity", sprintf "%s line %d asks for %g items; at least 1" label (k + 1) line.Quantity, target))

            match bundle.Required with
            | Some required when required < 1.0 || required > float bundle.Items.Length ->
                sink.Error(
                    "bundle.required",
                    path + ".required",
                    sprintf "%s needs %g lines but has %d; the game counts 1 to %d" label required bundle.Items.Length (max 1 bundle.Items.Length),
                    target
                )
            | _ -> ()

            if missing (HashSet<string>(context.Scenes.Keys)) bundle.SceneId then
                sink.Error("bundle.unknownScene", path + ".sceneId", sprintf "%s is handed in in missing scene \"%s\"" label (text bundle.SceneId), target)

            outcomes context sink "bundle" path label "" target bundle.Outcomes)

    /// Achievements (M6): their conditions are the events'.
    let private achievements (context: Context) (sink: Sink) =
        defaultArg context.Project.Achievements []
        |> List.iteri (fun i achievement ->
            let path = sprintf "achievements[%d]" i
            let label = sprintf "Achievement \"%s\"" achievement.Name
            let target = Some(NavigationTarget.Achievement achievement.Id)

            if achievement.Conditions.IsEmpty then
                sink.Warning("achievement.noConditions", path + ".conditions", sprintf "%s has no conditions, so it unlocks at once" label, target)

            conditions context sink "achievement" path label "" target achievement.Conditions)

    /// Letters (M6): the sender and attachments exist; outcomes and conditions are the events'.
    let private mail (context: Context) (sink: Sink) =
        defaultArg context.Project.Mail []
        |> List.iteri (fun i letter ->
            let path = sprintf "mail[%d]" i
            let label = sprintf "Letter \"%s\"" letter.Subject
            let target = Some(NavigationTarget.Mail letter.Id)

            if missingText context.NpcIds letter.From then
                sink.Error("mail.unknownNpc", path + ".from", sprintf "%s is from missing NPC \"%s\"" label letter.From, target)

            letter.Attachments
            |> List.iteri (fun k attachment ->
                if not (context.ItemIds.Contains attachment.ItemId) then
                    sink.Error(
                        "mail.unknownItem",
                        sprintf "%s.attachments[%d].itemId" path k,
                        sprintf "%s holds missing item \"%s\"" label attachment.ItemId,
                        target
                    ))

            if letter.Money < 0.0 then
                sink.Error("mail.money", path + ".money", sprintf "%s holds %g money; a letter can only give money" label letter.Money, target)

            conditions context sink "mail" path label "" target letter.Conditions
            outcomes context sink "mail" path label "" target letter.Outcomes)

    /// Monsters (M7): positive health, a movement the game knows and drops it has.
    let private monsters (context: Context) (sink: Sink) =
        defaultArg context.Project.Monsters []
        |> List.iteri (fun i monster ->
            let path = sprintf "monsters[%d]" i
            let label = sprintf "Monster \"%s\"" monster.Name
            let target = Some(NavigationTarget.Monster monster.Id)

            if monster.Health < 1.0 then
                sink.Error("monster.health", path + ".health", sprintf "%s has %g health; it needs at least 1" label monster.Health, target)

            if monster.Damage < 0.0 then
                sink.Error("monster.damage", path + ".damage", sprintf "%s does %g damage; damage cannot be negative" label monster.Damage, target)

            if monster.Defense < 0.0 then
                sink.Error("monster.defense", path + ".defense", sprintf "%s has %g defense; defense cannot be negative" label monster.Defense, target)

            if not (List.contains monster.Movement MonsterMovements.All) then
                sink.Error(
                    "monster.unknownMovement",
                    path + ".movement",
                    sprintf "%s moves by unknown \"%s\" (chase, wander or still)" label monster.Movement,
                    target
                )

            monster.Drops
            |> List.iteri (fun k drop ->
                let dp = sprintf "%s.drops[%d]" path k

                if not (context.ItemIds.Contains drop.ItemId) then
                    sink.Error("monster.unknownDrop", dp + ".itemId", sprintf "%s drops missing item \"%s\"" label drop.ItemId, target)

                if drop.Min > drop.Max then
                    sink.Error("monster.dropRange", dp + ".min", sprintf "%s drop %d has min %g above max %g" label (k + 1) drop.Min drop.Max, target)))

    /// reserved-keys.ts: gameplay keys creator hotkeys may never claim.
    let reservedActionKeys =
        HashSet<string>(
            [
                "w"
                "a"
                "s"
                "d"
                "e"
                "q"
                "t"
                "r"
                "f"
                "c"
                "z"
                "i"
                "j"
                "x"
                " "
                "enter"
                "escape"
                "arrowup"
                "arrowdown"
                "arrowleft"
                "arrowright"
            ]
        )

    let private actions (context: Context) (sink: Sink) =
        let hotkeys = Dictionary<string, string>()

        context.Project.Actions
        |> Seq.iteri (fun i action ->
            let path = sprintf "actions[%d]" i
            let label = sprintf "Action \"%s\"" action.Name
            let target = Some(NavigationTarget.Action action.Id)

            if action.Outcomes.IsEmpty then
                sink.Warning("action.noOutcomes", path + ".outcomes", sprintf "%s does nothing (no outcomes)" label, target)

            match action.Hotkey with
            | Some key when key.Length > 0 ->
                let lowered = key.ToLowerInvariant()

                if reservedActionKeys.Contains lowered then
                    sink.Warning(
                        "action.reservedHotkey",
                        path + ".hotkey",
                        sprintf "%s uses hotkey \"%s\", which is reserved for gameplay and ignored in play" label key,
                        target
                    )

                match hotkeys.TryGetValue lowered with
                | true, other ->
                    sink.Warning("action.duplicateHotkey", path + ".hotkey", sprintf "%s shares hotkey \"%s\" with \"%s\"" label key other, target)
                | _ -> hotkeys.[lowered] <- action.Name
            | _ -> ()

            conditions context sink "action" path label "" target action.Conditions
            outcomes context sink "action" path label "" target action.Outcomes)

    /// Actions that end up performing themselves again (A → B → A). The game stops an action chain
    /// after 4 levels or 256 actions in all and says so, so such a loop never runs the way it
    /// reads.
    let private actionCycles (context: Context) (sink: Sink) =
        let performs (action: ActionDef) =
            action.Outcomes
            |> List.choose (fun o ->
                if o.Type = EventOutcomeTypes.PerformAction then
                    o.ActionId
                else
                    None)
            |> List.filter (fun id -> id.Length > 0)

        let byId = Dictionary<string, ActionDef>()

        for action in context.Project.Actions do
            if not (byId.ContainsKey action.Id) then
                byId.[action.Id] <- action

        let reachesItself (start: ActionDef) =
            let seen = HashSet<string>()
            let pending = Stack<string>(performs start)
            let mutable found = false

            while not found && pending.Count > 0 do
                let id = pending.Pop()

                if id = start.Id then
                    found <- true
                elif seen.Add id then
                    match byId.TryGetValue id with
                    | true, next ->
                        for further in performs next do
                            pending.Push further
                    | _ -> ()

            found
        // Dialogue choices and minigame results resume only after player input. Keep those
        // edges separate: a repeatable conversation or contest is not runaway recursion.
        let graph = Dictionary<string * string, (string * string) list>()

        let reference (kind: string) (id: string option) =
            id
            |> Option.filter (fun value -> value.Length > 0)
            |> Option.map (fun value -> kind, value)
            |> Option.toList

        let outcomeEdges outcomes =
            outcomes
            |> List.collect (fun (outcome: EventOutcome) ->
                match outcome.Type with
                | EventOutcomeTypes.PerformAction -> reference "action" outcome.ActionId
                | EventOutcomeTypes.StartDialogue -> reference "dialogue" outcome.DialogueId
                | EventOutcomeTypes.StartMinigame -> reference "minigame" outcome.MinigameId
                | _ -> [])

        for action in context.Project.Actions do
            graph.TryAdd(("action", action.Id), outcomeEdges action.Outcomes) |> ignore

        for dialogue in DialogueCopies.shown context.Project do
            let edges =
                dialogue.Options
                // One-shot choices and choices hidden by their own flag cannot repeat.
                |> List.filter (fun option ->
                    option.Once <> Some true
                    && not (option.EventFlag.IsSome && option.EventFlag = option.HiddenIfFlag))
                |> List.collect (fun option -> reference "action" option.ActionId @ reference "dialogue" option.NextDialogueId)

            graph.TryAdd(("dialogue", dialogue.Id), edges) |> ignore

        for minigame in context.Project.Minigames do
            graph.TryAdd(("minigame", minigame.Id), minigame.ResultTiers |> List.collect (fun tier -> outcomeEdges tier.Outcomes))
            |> ignore

        let resumesItself id =
            let start = "action", id
            let pending = Stack<(string * string) * bool>()
            let seen = HashSet<(string * string) * bool>()

            match graph.TryGetValue start with
            | true, edges ->
                for edge in edges do
                    pending.Push(edge, fst edge <> "action")
            | _ -> ()

            let mutable found = false

            while not found && pending.Count > 0 do
                let node, interactive = pending.Pop()

                if node = start && interactive then
                    found <- true
                elif seen.Add(node, interactive) then
                    match graph.TryGetValue node with
                    | true, edges ->
                        for edge in edges do
                            pending.Push(edge, interactive || fst edge <> "action")
                    | _ -> ()

            found

        let reported = HashSet<string>()

        context.Project.Actions
        |> List.iteri (fun i action ->
            if reported.Add action.Id then
                if reachesItself action then
                    sink.Warning(
                        "action.cycle",
                        sprintf "actions[%d].outcomes" i,
                        sprintf "Action \"%s\" ends up performing itself again; the game stops such a chain after 4 levels or 256 actions" action.Name,
                        Some(NavigationTarget.Action action.Id)
                    )
                elif resumesItself action.Id then
                    sink.Info(
                        "action.interactiveCycle",
                        sprintf "actions[%d].outcomes" i,
                        sprintf
                            "Action \"%s\" can repeat through a dialogue choice or minigame result; check that this is intentional and that the player has an exit"
                            action.Name,
                        Some(NavigationTarget.Action action.Id)
                    ))

    let private minigames (context: Context) (sink: Sink) =
        context.Project.Minigames
        |> Seq.iteri (fun i minigame ->
            let path = sprintf "minigames[%d]" i
            let target = Some(NavigationTarget.Minigame minigame.Id)

            if System.String.IsNullOrWhiteSpace minigame.Kind then
                sink.Error("minigame.emptyKind", path + ".kind", sprintf "Minigame \"%s\" has no kind" minigame.Name, target)
            else
                match MinigameKinds.tryFind minigame.Kind with
                | None when (MinigameKinds.tryFindPackKind context.Project minigame.Kind).IsSome ->
                    // An enabled pack's plugin defines it; its settings are the plugin's own.
                    ()
                | None ->
                    let fallback = "it plays as a single \"Go!\" button that always scores 0.5"

                    let packId, name =
                        match minigame.Kind.IndexOf ':' with
                        | -1 -> "", minigame.Kind
                        | at -> minigame.Kind.Substring(0, at), minigame.Kind.Substring(at + 1)

                    let install =
                        context.Project.ContentPacks
                        |> List.tryFind (fun install -> install.Pack.Manifest.Id = packId)

                    let kindPath = path + ".kind"

                    match install with
                    | _ when packId.Length = 0 ->
                        sink.Warning(
                            "minigame.unknownKind",
                            kindPath,
                            sprintf
                                "Minigame \"%s\" uses kind \"%s\", which the game doesn't have: %s. Pick one of the built-in kinds"
                                minigame.Name
                                minigame.Kind
                                fallback,
                            target
                        )
                    | None ->
                        sink.Warning(
                            "minigame.packMissing",
                            kindPath,
                            sprintf
                                "Minigame \"%s\" uses kind \"%s\" from pack \"%s\", which is not installed: %s. Install the pack or pick another kind"
                                minigame.Name
                                minigame.Kind
                                packId
                                fallback,
                            target
                        )
                    | Some install when not install.Enabled ->
                        sink.Warning(
                            "minigame.packDisabled",
                            kindPath,
                            sprintf
                                "Minigame \"%s\" uses kind \"%s\" from pack \"%s\", which is turned off: %s. Turn the pack on in Mods or pick another kind"
                                minigame.Name
                                minigame.Kind
                                install.Pack.Manifest.Name
                                fallback,
                            target
                        )
                    | Some install ->
                        let pack = install.Pack

                        let declared =
                            pack.Plugins
                            |> List.exists (fun plugin ->
                                plugin.Minigames
                                |> Option.defaultValue []
                                |> List.choose (PackRules.minigameLocalName packId)
                                |> List.contains name)
                        // Declared but not found: the manifest does not allow it.
                        let code, reason =
                            if declared then
                                "minigame.packPermission",
                                sprintf
                                    "pack \"%s\" does not have the permission for it (its manifest's permissions.minigames does not list \"%s\")"
                                    pack.Manifest.Name
                                    name
                            else
                                "minigame.packUndefined",
                                sprintf
                                    "no plugin of pack \"%s\" defines it (a plugin's minigames list \"%s\" and its source calls api.minigame('%s', …))"
                                    pack.Manifest.Name
                                    name
                                    name

                        sink.Warning(
                            code,
                            kindPath,
                            sprintf "Minigame \"%s\" uses kind \"%s\", but %s: %s" minigame.Name minigame.Kind reason fallback,
                            target
                        )
                | Some kind ->
                    for key, value in minigame.Config do
                        let spath = sprintf "%s.config.%s" path key

                        match kind.Settings |> List.tryFind (fun setting -> setting.Key = key) with
                        | None ->
                            sink.Warning(
                                "minigame.unknownSetting",
                                spath,
                                sprintf "Minigame \"%s\" has setting \"%s\", which the %s kind doesn't use" minigame.Name key kind.Id,
                                target
                            )
                        | Some setting ->
                            match setting.Kind, value with
                            | _, JNull -> ()
                            | MinigameSettingKind.Text, JString _ -> ()
                            | (MinigameSettingKind.Number | MinigameSettingKind.Integer), JNumber n ->
                                let below = setting.Min |> Option.exists (fun min -> n < min)
                                let above = setting.Max |> Option.exists (fun max -> n > max)

                                if below || above then
                                    let range =
                                        match setting.Min, setting.Max with
                                        | Some min, Some max -> sprintf "%g to %g" min max
                                        | Some min, None -> sprintf "at least %g" min
                                        | None, Some max -> sprintf "at most %g" max
                                        | None, None -> ""

                                    sink.Warning(
                                        "minigame.settingRange",
                                        spath,
                                        sprintf "Minigame \"%s\": %s is %g; the game uses %s" minigame.Name setting.Label n range,
                                        target
                                    )
                            | MinigameSettingKind.Text, _ ->
                                sink.Warning(
                                    "minigame.settingType",
                                    spath,
                                    sprintf "Minigame \"%s\": %s should be text; the game ignores it" minigame.Name setting.Label,
                                    target
                                )
                            | _ ->
                                sink.Warning(
                                    "minigame.settingType",
                                    spath,
                                    sprintf "Minigame \"%s\": %s should be a number; the game ignores it" minigame.Name setting.Label,
                                    target
                                )

            if minigame.ResultTiers.IsEmpty then
                sink.Warning(
                    "minigame.noTiers",
                    path + ".resultTiers",
                    sprintf "Minigame \"%s\" has no result tiers, so its score changes nothing" minigame.Name,
                    target
                )

            minigame.ResultTiers
            |> Seq.iteri (fun t tier ->
                outcomes
                    context
                    sink
                    "minigame"
                    (sprintf "%s.resultTiers[%d]" path t)
                    (sprintf "Minigame \"%s\" tier %d" minigame.Name (t + 1))
                    ""
                    target
                    tier.Outcomes))

    let private shops (context: Context) (sink: Sink) =
        let opened =
            HashSet<string>(
                Seq.append
                    (context.Project.Dialogues
                     |> Seq.collect (fun d -> d.Options |> Seq.choose (fun o -> o.OpenShopId)))
                    (context.Project.Npcs
                     |> Seq.collect (fun n ->
                         n.Dialogue
                         |> Seq.collect (fun d -> d.Options |> Seq.choose (fun o -> o.OpenShopId))))
            )

        context.Project.Shops
        |> Seq.iteri (fun i shop ->
            let path = sprintf "shops[%d]" i
            let target = Some(NavigationTarget.Shop shop.Id)

            let weekdayIds =
                context.Project.Settings.Calendar.Weekdays
                |> Option.defaultValue []
                |> List.map (fun d -> d.Id)
                |> Set.ofList

            let checkWeekdays field days =
                days
                |> List.iteri (fun k id ->
                    if not (weekdayIds.Contains id) then
                        sink.Error("shop.unknownWeekday", sprintf "%s.%s[%d]" path field k, sprintf "Unknown weekday %s" id, target))

            match shop.Hours with
            | Some hours when hours.Open = hours.Close ->
                sink.Warning("shop.neverOpen", path + ".hours", "Equal opening and closing times keep the shop closed all day", target)
            | Some hours when
                hours.Open < 0.0
                || hours.Close < 0.0
                || hours.Open > context.Project.Settings.Time.DayEndMinute
                || hours.Close > context.Project.Settings.Time.DayEndMinute
                ->
                sink.Error("shop.hoursOutOfRange", path + ".hours", "Shop hours must fall within the game's day", target)
            | _ -> ()

            match shop.ClosedOn with
            | Some closed ->
                checkWeekdays "closedOn.weekdays" closed.Weekdays

                closed.Seasons
                |> List.iteri (fun k id ->
                    if not (context.SeasonIds.Contains id) then
                        sink.Error("shop.unknownSeason", sprintf "%s.closedOn.seasons[%d]" path k, sprintf "Unknown season %s" id, target))

                closed.Festivals
                |> List.iteri (fun k id ->
                    if not (context.FestivalIds.Contains id) then
                        sink.Error("shop.unknownFestival", sprintf "%s.closedOn.festivals[%d]" path k, sprintf "Unknown festival %s" id, target))

                if
                    (not weekdayIds.IsEmpty
                     && weekdayIds |> Set.forall (fun id -> List.contains id closed.Weekdays))
                    || (context.SeasonIds.Count > 0
                        && context.SeasonIds |> Seq.forall (fun id -> List.contains id closed.Seasons))
                then
                    sink.Warning("shop.neverOpen", path + ".closedOn", "This shop is closed every day", target)
            | None -> ()

            match shop.KeeperNpcId with
            | Some id when not (context.NpcIds.Contains id) ->
                sink.Error("shop.unknownKeeper", path + ".keeperNpcId", sprintf "Missing shopkeeper %s" id, target)
            | Some id ->
                let keeper = context.Project.Npcs |> List.tryFind (fun npc -> npc.Id = id)

                let merchantScenes =
                    context.Project.Npcs
                    |> List.filter (fun npc ->
                        npc.Dialogue
                        |> List.exists (fun d -> d.Options |> List.exists (fun o -> o.OpenShopId = Some shop.Id)))
                    |> List.map (fun npc -> npc.SceneId)

                match keeper with
                | Some npc when not merchantScenes.IsEmpty ->
                    let reachable =
                        npc.SceneId
                        :: ((defaultArg npc.Schedule [] |> List.map (fun stop -> stop.SceneId))
                            @ (defaultArg npc.Schedules []
                               |> List.collect (fun variant -> variant.Stops |> List.map (fun stop -> stop.SceneId))))

                    if not (reachable |> List.exists (fun scene -> List.contains scene merchantScenes)) then
                        sink.Warning(
                            "shop.keeperNeverPresent",
                            path + ".keeperNpcId",
                            "The shopkeeper never visits a scene containing this shop's merchant",
                            target
                        )
                | _ -> ()
            | None -> ()

            defaultArg shop.PriceModifiers []
            |> List.iteri (fun k rule ->
                let rulePath = sprintf "%s.priceModifiers[%d]" path k
                checkWeekdays (sprintf "priceModifiers[%d].weekdays" k) rule.Weekdays
                conditions context sink "shop" rulePath (sprintf "Shop %s pricing" shop.Name) "" target rule.Conditions

                for value in [ rule.BuyMultiplier; rule.SellMultiplier ] |> List.choose id do
                    if value < 0.0 || value > 1000.0 then
                        sink.Error("shop.priceMultiplier", rulePath, "Price multipliers must be between 0 and 1000", target))

            match shop.Saturation with
            | Some rule when
                rule.QuantityPerStep < 1.0
                || rule.RecoveryDays < 1.0
                || rule.ReductionPerStep < 0.0
                || rule.ReductionPerStep > 1.0
                || rule.MinimumMultiplier < 0.0
                || rule.MinimumMultiplier > 1.0
                ->
                sink.Error(
                    "shop.saturationRange",
                    path + ".saturation",
                    "Saturation needs positive quantity and recovery days, and multipliers between 0 and 1",
                    target
                )
            | _ -> ()

            if shop.Stock.IsEmpty then
                sink.Info("shop.noStock", path + ".stock", sprintf "Shop \"%s\" sells nothing" shop.Name, target)

            if not (opened.Contains shop.Id) then
                sink.Info("shop.unreachable", path, sprintf "No dialogue option opens shop \"%s\"" shop.Name, target)

            shop.Stock
            |> Seq.iteri (fun k entry ->
                match entry.Seasons with
                | None -> ()
                | Some seasons ->
                    seasons
                    |> Seq.iteri (fun s season ->
                        if not (context.SeasonIds.Contains season) then
                            sink.Warning(
                                "shop.unknownSeason",
                                sprintf "%s.stock[%d].seasons[%d]" path k s,
                                sprintf "Shop \"%s\" sells \"%s\" in season \"%s\", which is not in the calendar" shop.Name entry.ItemId season,
                                target
                            )))
            // A builder and an animal shop (M5).
            defaultArg shop.Buildings []
            |> List.iteri (fun k id ->
                if not (context.BuildingIds.Contains id) then
                    sink.Error(
                        "shop.unknownBuilding",
                        sprintf "%s.buildings[%d]" path k,
                        sprintf "Shop \"%s\" builds missing building \"%s\"" shop.Name id,
                        target
                    ))

            defaultArg shop.Animals []
            |> List.iteri (fun k id ->
                if not (context.SpeciesIds.Contains id) then
                    sink.Error(
                        "shop.unknownSpecies",
                        sprintf "%s.animals[%d]" path k,
                        sprintf "Shop \"%s\" sells missing animal species \"%s\"" shop.Name id,
                        target
                    )))

    let private recipes (context: Context) (sink: Sink) =
        let stations =
            HashSet<string>(context.Project.MachineTypes |> Seq.collect (fun m -> m.StationCategories))

        context.Project.Recipes
        |> Seq.iteri (fun i recipe ->
            let path = sprintf "recipes[%d]" i
            let target = Some(NavigationTarget.Recipe recipe.Id)

            let ingredients (kind: string) (list: RecipeIngredient list) =
                list
                |> Seq.iteri (fun k ingredient ->
                    if not (context.ItemIds.Contains ingredient.ItemId) then
                        sink.Error(
                            sprintf "recipe.unknown%s" (if kind = "inputs" then "Input" else "Output"),
                            sprintf "%s.%s[%d].itemId" path kind k,
                            sprintf "Recipe \"%s\" %s missing item \"%s\"" recipe.Name (if kind = "inputs" then "needs" else "makes") ingredient.ItemId,
                            target
                        ))

            ingredients "inputs" recipe.Inputs
            ingredients "outputs" recipe.Outputs

            if recipe.Outputs.IsEmpty then
                sink.Warning("recipe.noOutputs", path + ".outputs", sprintf "Recipe \"%s\" makes nothing" recipe.Name, target)

            if missing context.MachineTypeIds recipe.MachineTypeId then
                sink.Error(
                    "recipe.unknownMachine",
                    path + ".machineTypeId",
                    sprintf "Recipe \"%s\" needs missing machine \"%s\"" recipe.Name (text recipe.MachineTypeId),
                    target
                )

            match recipe.RequiresStationCategory with
            | Some station when station.Length > 0 && not (stations.Contains station) ->
                sink.Warning(
                    "recipe.unknownStation",
                    path + ".requiresStationCategory",
                    sprintf "Recipe \"%s\" needs a \"%s\" station, which no machine provides" recipe.Name station,
                    target
                )
            | _ -> ()

            match recipe.Unlock with
            | None -> ()
            | Some unlock ->
                match unlock.Skill with
                | Some skill when
                    not (SaveSchema.SkillNames |> Seq.contains skill.Skill)
                    && not (
                        defaultArg context.Project.Settings.SkillDefinitions []
                        |> List.exists (fun definition -> definition.Id = skill.Skill)
                    )
                    ->
                    sink.Warning(
                        "recipe.unknownSkill",
                        path + ".unlock.skill.skill",
                        sprintf "Recipe \"%s\" unlocks with unknown skill \"%s\"" recipe.Name skill.Skill,
                        target
                    )
                | _ -> ()

                if missing context.QuestIds unlock.QuestId then
                    sink.Error(
                        "recipe.unlockQuestMissing",
                        path + ".unlock.questId",
                        sprintf "Recipe \"%s\" unlocks after missing quest \"%s\"" recipe.Name (text unlock.QuestId),
                        target
                    )

                match unlock.Seasons with
                | None -> ()
                | Some seasons ->
                    seasons
                    |> Seq.iteri (fun k season ->
                        if not (context.SeasonIds.Contains season) then
                            sink.Warning(
                                "recipe.unknownSeason",
                                sprintf "%s.unlock.seasons[%d]" path k,
                                sprintf "Recipe \"%s\" is craftable in \"%s\", which is not in the calendar" recipe.Name season,
                                target
                            )))

    let private machineTypes (context: Context) (sink: Sink) =
        context.Project.MachineTypes
        |> Seq.iteri (fun i machine ->
            let path = sprintf "machineTypes[%d]" i
            let target = Some(NavigationTarget.MachineType machine.Id)

            match machine.ItemId with
            | None ->
                sink.Warning("machineType.noItem", path + ".itemId", sprintf "Machine \"%s\" has no item, so players cannot place it" machine.Name, target)
            | Some id when id.Length = 0 ->
                sink.Warning("machineType.noItem", path + ".itemId", sprintf "Machine \"%s\" has no item, so players cannot place it" machine.Name, target)
            | Some id when not (context.ItemIds.Contains id) ->
                sink.Error("machineType.unknownItem", path + ".itemId", sprintf "Machine \"%s\" is placed with missing item \"%s\"" machine.Name id, target)
            | _ -> ()
            // Placed objects (M5): storage and area effects within what the engine applies.
            match machine.Storage with
            | Some slots when slots < 0.0 || slots > FarmSchema.MaxStorageSlots ->
                sink.Warning(
                    "machineType.storage",
                    path + ".storage",
                    sprintf "Machine \"%s\" stores %g slots; the game keeps 0 to %g" machine.Name slots FarmSchema.MaxStorageSlots,
                    target
                )
            | _ -> ()

            defaultArg machine.AreaEffects []
            |> List.iteri (fun k effect ->
                let ep = sprintf "%s.areaEffects[%d]" path k

                if not (List.contains effect.Effect AreaEffects.All) then
                    sink.Error(
                        "machineType.unknownAreaEffect",
                        ep + ".effect",
                        sprintf "Machine \"%s\" has unknown area effect \"%s\" (water or protect)" machine.Name effect.Effect,
                        target
                    )

                if effect.Radius < 0.0 || effect.Radius > FarmSchema.MaxAreaRadius then
                    sink.Warning(
                        "machineType.areaRadius",
                        ep + ".radius",
                        sprintf "Machine \"%s\" reaches %g tiles; the game reaches 0 to %g" machine.Name effect.Radius FarmSchema.MaxAreaRadius,
                        target
                    )))

    let private nodeTypes (context: Context) (sink: Sink) =
        context.Project.NodeTypes
        |> Seq.iteri (fun i node ->
            let path = sprintf "nodeTypes[%d]" i
            let target = Some(NavigationTarget.NodeType node.Id)

            if node.Drops.IsEmpty then
                sink.Warning("nodeType.noDrops", path + ".drops", sprintf "Node type \"%s\" drops nothing" node.Name, target)

            node.Drops
            |> Seq.iteri (fun k drop ->
                if drop.Min > drop.Max then
                    sink.Error(
                        "nodeType.dropRange",
                        sprintf "%s.drops[%d].min" path k,
                        sprintf "Node type \"%s\" drop %d has min %g above max %g" node.Name (k + 1) drop.Min drop.Max,
                        target
                    )))

    let private wildlife (context: Context) (sink: Sink) =
        context.Project.AnimalSpecies
        |> Seq.iteri (fun i species ->
            let path = sprintf "animalSpecies[%d]" i
            let target = Some(NavigationTarget.AnimalSpecies species.Id)

            if species.ProductItemId.Length = 0 then
                sink.Warning("species.noProduct", path + ".productItemId", sprintf "Species \"%s\" produces nothing" species.Name, target)
            elif not (context.ItemIds.Contains species.ProductItemId) then
                sink.Error(
                    "species.unknownProduct",
                    path + ".productItemId",
                    sprintf "Species \"%s\" produces missing item \"%s\"" species.Name species.ProductItemId,
                    target
                )

            if missing context.ItemIds species.FeedItemId then
                sink.Error(
                    "species.unknownFeed",
                    path + ".feedItemId",
                    sprintf "Species \"%s\" eats missing item \"%s\"" species.Name (text species.FeedItemId),
                    target
                )
            // Housing, more products, mood qualities and breeding (M5).
            defaultArg species.Housing []
            |> List.iteri (fun k id ->
                if not (context.BuildingIds.Contains id) then
                    sink.Error(
                        "species.unknownHousing",
                        sprintf "%s.housing[%d]" path k,
                        sprintf "Species \"%s\" lives in missing building \"%s\"" species.Name id,
                        target
                    ))

            defaultArg species.Products []
            |> List.iteri (fun k product ->
                let pp = sprintf "%s.products[%d]" path k

                if not (context.ItemIds.Contains product.ItemId) then
                    sink.Error(
                        "species.unknownProduct",
                        pp + ".itemId",
                        sprintf "Species \"%s\" produces missing item \"%s\"" species.Name product.ItemId,
                        target
                    )

                if product.IntervalDays < 1.0 then
                    sink.Error(
                        "species.productInterval",
                        pp + ".intervalDays",
                        sprintf "Species \"%s\" product %d needs at least 1 day between products" species.Name (k + 1),
                        target
                    ))

            defaultArg species.MoodQualities []
            |> List.iteri (fun k tier ->
                if not (context.QualityIds.Contains tier.Quality) then
                    sink.Error(
                        "species.unknownMoodQuality",
                        sprintf "%s.moodQualities[%d].quality" path k,
                        sprintf "Species \"%s\" earns missing crop quality \"%s\"" species.Name tier.Quality,
                        target
                    ))

            match species.Breeding with
            | Some breeding when breeding.Chance < 0.0 || breeding.Chance > 1.0 ->
                sink.Error(
                    "species.breedingChance",
                    path + ".breeding.chance",
                    sprintf "Species \"%s\" breeds with chance %g; a chance is 0 to 1" species.Name breeding.Chance,
                    target
                )
            | _ -> ())

        context.Project.Animals
        |> Seq.iteri (fun i animal ->
            let path = sprintf "animals[%d]" i
            let target = Some(NavigationTarget.AnimalSpecies animal.SpeciesId)

            if not (context.SpeciesIds.Contains animal.SpeciesId) then
                sink.Error(
                    "animal.unknownSpecies",
                    path + ".speciesId",
                    sprintf "Animal \"%s\" is of missing species \"%s\"" animal.Name animal.SpeciesId,
                    target
                )

            if not (context.Scenes.ContainsKey animal.SceneId) then
                sink.Error("animal.unknownScene", path + ".sceneId", sprintf "Animal \"%s\" lives in missing scene \"%s\"" animal.Name animal.SceneId, target)
            elif not (Context.tileInScene context animal.SceneId animal.X animal.Y) then
                sink.Error(
                    "animal.outOfBounds",
                    path + ".x",
                    sprintf "Animal \"%s\" stands at (%g,%g), outside its scene" animal.Name animal.X animal.Y,
                    Some(NavigationTarget.Scene(animal.SceneId, 0, 0))
                )

            if missing context.PlacedBuildingIds animal.BuildingId then
                sink.Error(
                    "animal.unknownBuilding",
                    path + ".buildingId",
                    sprintf "Animal \"%s\" lives in missing building \"%s\"" animal.Name (text animal.BuildingId),
                    target
                ))

        context.Project.FishTables
        |> Seq.iteri (fun i table ->
            let path = sprintf "fishTables[%d]" i
            let target = Some(NavigationTarget.FishTable table.Id)

            if table.Entries.IsEmpty then
                sink.Warning("fishTable.noEntries", path + ".entries", sprintf "Fish table \"%s\" has no fish" table.Name, target)

            table.Entries
            |> Seq.iteri (fun k entry ->
                if not (context.ItemIds.Contains entry.ItemId) then
                    sink.Error(
                        "fishTable.unknownItem",
                        sprintf "%s.entries[%d].itemId" path k,
                        sprintf "Fish table \"%s\" catches missing item \"%s\"" table.Name entry.ItemId,
                        target
                    ))

            if missing context.ItemIds table.JunkItemId then
                sink.Error(
                    "fishTable.unknownJunk",
                    path + ".junkItemId",
                    sprintf "Fish table \"%s\" catches missing junk item \"%s\"" table.Name (text table.JunkItemId),
                    target
                )

            match table.SceneIds with
            | None -> ()
            | Some scenes ->
                scenes
                |> Seq.iteri (fun k id ->
                    if not (context.Scenes.ContainsKey id) then
                        sink.Error(
                            "fishTable.unknownScene",
                            sprintf "%s.sceneIds[%d]" path k,
                            sprintf "Fish table \"%s\" applies to missing scene \"%s\"" table.Name id,
                            target
                        ))

            match table.Seasons with
            | None -> ()
            | Some seasons ->
                seasons
                |> Seq.iteri (fun k season ->
                    if not (context.SeasonIds.Contains season) then
                        sink.Warning(
                            "fishTable.unknownSeason",
                            sprintf "%s.seasons[%d]" path k,
                            sprintf "Fish table \"%s\" applies in \"%s\", which is not in the calendar" table.Name season,
                            target
                        )))

    /// validate-graphics.ts: bindings, frames, clips and sheets must fit their assets.
    let private graphics (context: Context) (sink: Sink) =
        let project = context.Project
        let assets = Dictionary<string, CustomAsset>()

        for a in project.CustomAssets do
            if not (assets.ContainsKey a.Id) then
                assets.[a.Id] <- a

        let frame (path: string) (f: ArtFrame) (defaultId: string) (label: string) (target: NavigationTarget option) =
            let id = defaultArg f.AssetId defaultId

            match assets.TryGetValue id with
            | false, _ -> sink.Error("graphics.missingAsset", path, sprintf "%s: missing frame image %s" label id, target)
            | true, a ->
                match a.Width, a.Height with
                | Some width, Some height when f.X + f.Width > width || f.Y + f.Height > height ->
                    sink.Error("graphics.frameOutside", path, sprintf "%s: frame is outside %s (%g×%g)" label a.Name width height, target)
                | _ -> ()

        let visual (path: string) (v: VisualRef option) (label: string) (target: NavigationTarget option) =
            match v with
            | None -> ()
            | Some v ->
                match assets.TryGetValue v.AssetId with
                | false, _ -> sink.Error("graphics.missingAsset", path + ".assetId", sprintf "%s: missing artwork %s" label v.AssetId, target)
                | true, a ->
                    match v.Animation with
                    | Some animation when
                        animation.Length > 0
                        && not (
                            match a.Animations with
                            | None -> false
                            | Some clips -> clips |> List.exists (fun c -> c.Name = animation)
                        )
                        ->
                        sink.Error("graphics.missingAnimation", path + ".animation", sprintf "%s: missing animation %s on %s" label animation a.Name, target)
                    | _ -> ()

                    match v.Frame with
                    | None -> ()
                    | Some f -> frame (path + ".frame") f a.Id label target

        project.CustomAssets
        |> Seq.iteri (fun i a ->
            let path = sprintf "customAssets[%d]" i
            let target = Some(NavigationTarget.Asset a.Id)

            match a.Animations with
            | None -> ()
            | Some clips ->
                let names = HashSet<string>()

                clips
                |> Seq.iteri (fun c clip ->
                    if not (names.Add clip.Name) then
                        sink.Error(
                            "graphics.duplicateClip",
                            sprintf "%s.animations[%d].name" path c,
                            sprintf "%s: duplicate clip %s" a.Name clip.Name,
                            target
                        )

                    clip.Frames
                    |> Seq.iteri (fun k f ->
                        frame (sprintf "%s.animations[%d].frames[%d]" path c k) f a.Id (sprintf "%s / %s / frame %d" a.Name clip.Name (k + 1)) target))

            match a.Sheet, a.Width, a.Height with
            | Some sheet, Some width, Some height when
                sheet.FrameWidth * sheet.Frames > width
                || sheet.FrameHeight * (if sheet.Directional then 4.0 else 1.0) > height
                ->
                sink.Error("graphics.sheetTooLarge", path + ".sheet", sprintf "%s: sprite sheet dimensions exceed the image" a.Name, target)
            | _ -> ())

        visual "playerVisual" project.PlayerVisual "Player" (Some NavigationTarget.Settings)

        project.Npcs
        |> Seq.iteri (fun i n -> visual (sprintf "npcs[%d].visual" i) n.Visual n.Name (Some(NavigationTarget.Npc n.Id)))

        project.Items
        |> Seq.iteri (fun i d -> visual (sprintf "items[%d].visual" i) d.Visual d.Name (Some(NavigationTarget.Item d.Id)))

        match project.CustomCrops with
        | None -> ()
        | Some crops ->
            crops
            |> Seq.iteri (fun i d -> visual (sprintf "customCrops[%d].visual" i) d.Visual d.Name (Some(NavigationTarget.Crop d.Id)))

        project.NodeTypes
        |> Seq.iteri (fun i d -> visual (sprintf "nodeTypes[%d].visual" i) d.Visual d.Name (Some(NavigationTarget.NodeType d.Id)))

        project.MachineTypes
        |> Seq.iteri (fun i d -> visual (sprintf "machineTypes[%d].visual" i) d.Visual d.Name (Some(NavigationTarget.MachineType d.Id)))

        project.AnimalSpecies
        |> Seq.iteri (fun i d -> visual (sprintf "animalSpecies[%d].visual" i) d.Visual d.Name (Some(NavigationTarget.AnimalSpecies d.Id)))

        project.Scenes
        |> Seq.iteri (fun s scene ->
            scene.Tiles
            |> Seq.iteri (fun y row ->
                row
                |> Seq.iteri (fun x tile ->
                    // Most tiles carry no art of their own: no path or label strings for those.
                    if
                        tile.Visuals.IsSome
                        || (match tile.Item with
                            | Some item -> item.Visual.IsSome
                            | None -> false)
                    then
                        let path = sprintf "scenes[%d].tiles[%d][%d]" s y x
                        let target = Some(NavigationTarget.Scene(scene.Id, x, y))
                        let label = sprintf "%s (%d,%d)" scene.Name x y

                        match tile.Visuals with
                        | None -> ()
                        | Some visuals ->
                            visual (path + ".visuals.background") visuals.Background (label + " background") target
                            visual (path + ".visuals.overlay") visuals.Overlay (label + " overlay") target
                            visual (path + ".visuals.object") visuals.Object (label + " object") target

                        match tile.Item with
                        | None -> ()
                        | Some item -> visual (path + ".item.visual") item.Visual (sprintf "%s dropped item" scene.Name) target)))

        match project.GamePanels with
        | None -> ()
        | Some panels ->
            panels
            |> Seq.iteri (fun i panel ->
                panel.Entries
                |> Seq.iteri (fun k entry ->
                    let path = sprintf "gamePanels[%d].entries[%d].value" i k

                    let shown =
                        if entry.Value.Length = 0 then
                            "(not selected)"
                        else
                            entry.Value

                    if
                        entry.Kind = GamePanelEntryKinds.Action
                        && not (context.ActionIds.Contains entry.Value)
                    then
                        sink.Error("interface.missingAction", path, sprintf "%s: missing action %s" panel.Title shown, Some NavigationTarget.Interface)

                    if
                        entry.Kind = GamePanelEntryKinds.Item
                        && not (context.ItemIds.Contains entry.Value)
                    then
                        sink.Error("interface.missingItem", path, sprintf "%s: missing item %s" panel.Title shown, Some NavigationTarget.Interface)))

    /// Warps in enabled packs' events and actions land where the project's own are checked to:
    /// inside an existing scene (the project's, or a pack's under its namespaced id) and not on a
    /// blocked tile. Packs don't namespace scene references, so a pack naming its own scene by its
    /// local id is pointed at the full one.
    let private packWarps (context: Context) (sink: Sink) =
        let enabled =
            context.Project.ContentPacks |> List.filter (fun install -> install.Enabled)

        let packScenes = Dictionary<string, Scene>()

        for install in enabled do
            for scene in (PackRules.namespacePack install.Pack).Content.Scenes do
                if not (packScenes.ContainsKey scene.Id) then
                    packScenes.[scene.Id] <- scene

        let sceneById (id: string) =
            match context.Scenes.TryGetValue id with
            | true, scene -> Some scene
            | _ ->
                match packScenes.TryGetValue id with
                | true, scene -> Some scene
                | _ -> None

        context.Project.ContentPacks
        |> List.iteri (fun i install ->
            if install.Enabled then
                let manifest = install.Pack.Manifest
                let pack = PackRules.namespacePack install.Pack
                let target = Some(NavigationTarget.Pack manifest.Id)

                let check (path: string) (owner: string) (outcomes: EventOutcome list) =
                    outcomes
                    |> List.iteri (fun k outcome ->
                        let opath = sprintf "%s.outcomes[%d]" path k

                        match outcome.SceneId with
                        | Some id when outcome.Type = EventOutcomeTypes.WarpPlayer && id.Length > 0 ->
                            let x = defaultArg outcome.X 0.0
                            let y = defaultArg outcome.Y 0.0

                            match sceneById id with
                            | None ->
                                let full = PackRules.namespacedId manifest.Id id

                                let hint =
                                    if packScenes.ContainsKey full then
                                        sprintf " (write \"%s\": packs don't namespace scene references)" full
                                    else
                                        ""

                                sink.Warning(
                                    "pack.warpUnknownScene",
                                    opath + ".sceneId",
                                    sprintf "Pack \"%s\": %s warps to missing scene \"%s\"%s" manifest.Name owner id hint,
                                    target
                                )
                            | Some scene when x < 0.0 || y < 0.0 || x >= scene.Width || y >= scene.Height ->
                                sink.Warning(
                                    "pack.warpOutOfBounds",
                                    opath + ".x",
                                    sprintf
                                        "Pack \"%s\": %s warps the player to (%g,%g), outside \"%s\"; the game moves the player to the nearest open tile"
                                        manifest.Name
                                        owner
                                        x
                                        y
                                        scene.Name,
                                    target
                                )
                            | Some scene ->
                                match ChecksWorld.tileAt scene x y with
                                | Some tile when ChecksWorld.tileBlocks context.Project tile ->
                                    sink.Warning(
                                        "pack.warpBlocked",
                                        opath + ".x",
                                        sprintf
                                            "Pack \"%s\": %s warps the player onto a blocked tile (%g,%g) in \"%s\"; the game moves the player to the nearest open tile"
                                            manifest.Name
                                            owner
                                            x
                                            y
                                            scene.Name,
                                        target
                                    )
                                | _ -> ()
                        | _ -> ())

                pack.Content.Events
                |> List.iteri (fun e event ->
                    check (sprintf "contentPacks[%d].pack.content.events[%d]" i e) (sprintf "event \"%s\"" event.Name) event.Outcomes)

                pack.Content.Actions
                |> List.iteri (fun a action ->
                    check (sprintf "contentPacks[%d].pack.content.actions[%d]" i a) (sprintf "action \"%s\"" action.Name) action.Outcomes))

    /// ModsEditor: the compatibility badge and the "already installed" rule.
    let private packs (context: Context) (sink: Sink) =
        let seen = HashSet<string>()

        context.Project.ContentPacks
        |> Seq.iteri (fun i install ->
            let manifest = install.Pack.Manifest
            let path = sprintf "contentPacks[%d]" i
            let target = Some(NavigationTarget.Pack manifest.Id)

            if not (seen.Add manifest.Id) then
                sink.Error("pack.duplicate", path + ".pack.manifest.id", sprintf "Pack \"%s\" is installed twice" manifest.Id, target)

            if not (PackRules.isEngineCompatible (Some manifest.EngineCompatibility) PackRules.EngineVersion) then
                sink.Warning(
                    "pack.incompatible",
                    path + ".pack.manifest.engineCompatibility",
                    sprintf "Pack \"%s\" wants engine %s, this is %s" manifest.Name manifest.EngineCompatibility PackRules.EngineVersion,
                    target
                )

            manifest.Permissions.Mutations
            |> Option.defaultValue []
            |> List.iteri (fun k entry ->
                if not (PackRules.isMutationCapability entry) then
                    sink.Warning(
                        "pack.unknownMutation",
                        sprintf "%s.pack.manifest.permissions.mutations[%d]" path k,
                        sprintf "Pack \"%s\" asks for plugin capability \"%s\", which this engine does not know (it is ignored)" manifest.Name entry,
                        target
                    ))
            // Minigame kinds: names like pack ids; a plugin defines only what the manifest allows.
            let permitted =
                manifest.Permissions.Minigames
                |> Option.defaultValue []
                |> List.choose (PackRules.minigameLocalName manifest.Id)

            manifest.Permissions.Minigames
            |> Option.defaultValue []
            |> List.iteri (fun k entry ->
                if (PackRules.minigameLocalName manifest.Id entry).IsNone then
                    sink.Warning(
                        "pack.badMinigameName",
                        sprintf "%s.pack.manifest.permissions.minigames[%d]" path k,
                        sprintf
                            "Pack \"%s\" asks for minigame kind \"%s\": names are lowercase letters, digits and dashes, of this pack only (it is ignored)"
                            manifest.Name
                            entry,
                        target
                    ))

            install.Pack.Plugins
            |> List.iteri (fun p plugin ->
                plugin.Minigames
                |> Option.defaultValue []
                |> List.iteri (fun k entry ->
                    match PackRules.minigameLocalName manifest.Id entry with
                    | Some name when not (List.contains name permitted) ->
                        sink.Warning(
                            "pack.minigameNotPermitted",
                            sprintf "%s.pack.plugins[%d].minigames[%d]" path p k,
                            sprintf
                                "Plugin \"%s\" of pack \"%s\" defines minigame kind \"%s\", which the manifest's permissions.minigames does not allow: the game does not run it"
                                (defaultArg plugin.Name plugin.Id)
                                manifest.Name
                                name,
                            target
                        )
                    | Some _ -> ()
                    | None ->
                        sink.Warning(
                            "pack.badMinigameName",
                            sprintf "%s.pack.plugins[%d].minigames[%d]" path p k,
                            sprintf
                                "Plugin \"%s\" of pack \"%s\" defines minigame kind \"%s\": names are lowercase letters, digits and dashes, of this pack only (it is ignored)"
                                (defaultArg plugin.Name plugin.Id)
                                manifest.Name
                                entry,
                            target
                        )))
            // Plugins answer onWeatherRoll after the night: the day's watering and storm damage
            // already used the rolled weather (docs/PLUGINS.md).
            if install.Enabled && manifest.Permissions.Hooks |> List.contains "onWeatherRoll" then
                install.Pack.Plugins
                |> List.iteri (fun k plugin ->
                    if plugin.Hooks |> List.contains "onWeatherRoll" then
                        sink.Warning(
                            "pack.weatherRollAfterwards",
                            sprintf "%s.pack.plugins[%d].hooks" path k,
                            sprintf
                                "Plugin \"%s\" of pack \"%s\" listens to onWeatherRoll, which plugins cannot answer in time: a setWeather answer changes the weather after the night's rain watering and storm damage"
                                (defaultArg plugin.Name plugin.Id)
                                manifest.Name,
                            target
                        )))

        packWarps context sink

    /// Whole-number fields (money, counts) with a fraction: the engine keeps them as integers and
    /// rounds them when it loads the game (docs/dev/NUMERICS.md), so say so.
    let private offGridNumbers (context: Context) (sink: Sink) =
        let p = context.Project

        let check (path: string) (what: string) (value: float) (target: NavigationTarget option) =
            if
                not (System.Double.IsNaN value || System.Double.IsInfinity value)
                && value <> System.Math.Truncate value
            then
                sink.Warning(
                    "numbers.offGrid",
                    path,
                    sprintf
                        "%s is %s, but the game only uses whole numbers here: it plays as %s"
                        what
                        (JsNumber.format value)
                        (JsNumber.format (JsNumber.roundHalfAway value)),
                    target
                )

        let checkSome path what (value: float option) target =
            value |> Option.iter (fun v -> check path what v target)

        p.Items
        |> List.iteri (fun i item ->
            let target = Some(NavigationTarget.Item item.Id)
            check (sprintf "items[%d].value" i) (sprintf "The value of \"%s\"" item.Name) item.Value target
            check (sprintf "items[%d].maxStack" i) (sprintf "The stack size of \"%s\"" item.Name) item.MaxStack target)

        p.CustomCrops
        |> Option.defaultValue []
        |> List.iteri (fun i crop ->
            let target = Some(NavigationTarget.Crop crop.Id)
            let path = sprintf "customCrops[%d]" i
            check (path + ".seedCost") (sprintf "The seed cost of \"%s\"" crop.Name) crop.SeedCost target
            check (path + ".baseHarvestValue") (sprintf "The harvest value of \"%s\"" crop.Name) crop.BaseHarvestValue target
            check (path + ".yieldMin") (sprintf "The minimum yield of \"%s\"" crop.Name) crop.YieldMin target
            check (path + ".yieldMax") (sprintf "The maximum yield of \"%s\"" crop.Name) crop.YieldMax target)

        p.Shops
        |> List.iteri (fun i shop ->
            let target = Some(NavigationTarget.Shop shop.Id)

            shop.Stock
            |> List.iteri (fun k entry ->
                let path = sprintf "shops[%d].stock[%d]" i k
                checkSome (path + ".price") (sprintf "The price of \"%s\" in \"%s\"" entry.ItemId shop.Name) entry.Price target
                checkSome (path + ".dailyLimit") (sprintf "The daily limit of \"%s\" in \"%s\"" entry.ItemId shop.Name) entry.DailyLimit target))

        p.Recipes
        |> List.iteri (fun i recipe ->
            let target = Some(NavigationTarget.Recipe recipe.Id)

            let ingredients (key: string) (list: RecipeIngredient list) =
                list
                |> List.iteri (fun k ingredient ->
                    check (sprintf "recipes[%d].%s[%d].quantity" i key k) (sprintf "A quantity in \"%s\"" recipe.Name) ingredient.Quantity target)

            ingredients "inputs" recipe.Inputs
            ingredients "outputs" recipe.Outputs)

        p.Quests
        |> List.iteri (fun i quest ->
            let target = Some(NavigationTarget.Quest quest.Id)
            checkSome (sprintf "quests[%d].rewards.money" i) (sprintf "The money reward of \"%s\"" quest.Name) quest.Rewards.Money target

            quest.Rewards.Items
            |> Option.defaultValue []
            |> List.iteri (fun k reward ->
                check (sprintf "quests[%d].rewards.items[%d].quantity" i k) (sprintf "A reward quantity of \"%s\"" quest.Name) reward.Quantity target))

        p.AnimalSpecies
        |> List.iteri (fun i species ->
            check
                (sprintf "animalSpecies[%d].purchaseCost" i)
                (sprintf "The price of \"%s\"" species.Name)
                species.PurchaseCost
                (Some(NavigationTarget.AnimalSpecies species.Id)))

        p.NodeTypes
        |> List.iteri (fun i node ->
            let target = Some(NavigationTarget.NodeType node.Id)

            node.Drops
            |> List.iteri (fun k drop ->
                check (sprintf "nodeTypes[%d].drops[%d].min" i k) (sprintf "A drop amount of \"%s\"" node.Name) drop.Min target
                check (sprintf "nodeTypes[%d].drops[%d].max" i k) (sprintf "A drop amount of \"%s\"" node.Name) drop.Max target))

    let private festivalPrograms (context: Context) (sink: Sink) =
        let target = Some NavigationTarget.Settings

        context.Project.Settings.Calendar.Festivals
        |> List.iteri (fun i festival ->
            match festival.Program with
            | None -> ()
            | Some program ->
                let path = sprintf "settings.calendar.festivals[%d].program" i

                let point field scene x y =
                    if not (context.Scenes.ContainsKey scene) then
                        sink.Error("festival.unknownScene", path + "." + field, sprintf "Missing festival scene %s" scene, target)
                    elif not (Context.tileInScene context scene x y) then
                        sink.Error("festival.outOfBounds", path + "." + field, "Festival location is outside its scene", target)
                    else
                        match ChecksWorld.tileAt context.Scenes.[scene] x y with
                        | Some tile when ChecksWorld.tileBlocks context.Project tile ->
                            sink.Warning("festival.blocked", path + "." + field, "Festival location is blocked", target)
                        | _ -> ()

                point "entranceSceneId" program.EntranceSceneId program.EntranceX program.EntranceY
                point "sceneId" program.SceneId program.ArrivalX program.ArrivalY

                if
                    program.OpenMinute < 0.0
                    || program.CloseMinute <= program.OpenMinute
                    || program.CloseMinute > context.Project.Settings.Time.DayEndMinute
                then
                    sink.Error("festival.hours", path + ".openMinute", "Festival hours must open before closing within the playable day", target)

                conditions context sink "festival" path festival.Name program.EntranceSceneId target program.Conditions
                outcomes context sink "festival" (path + ".onMiss") festival.Name program.EntranceSceneId target program.OnMiss
                let guests = HashSet<string>()

                program.Attendance
                |> List.iteri (fun j guest ->
                    let guestPath = sprintf "%s.attendance[%d]" path j

                    if not (context.NpcIds.Contains guest.NpcId) then
                        sink.Error("festival.unknownNpc", guestPath + ".npcId", sprintf "Missing guest %s" guest.NpcId, target)

                    if not (guests.Add guest.NpcId) then
                        sink.Error("festival.duplicateGuest", guestPath + ".npcId", "A guest can attend only once", target)

                    point (sprintf "attendance[%d]" j) program.SceneId guest.X guest.Y

                    match guest.DialogueId with
                    | Some id when
                        not (context.Project.Dialogues |> List.exists (fun d -> d.Id = id))
                        && not (
                            context.Project.Npcs
                            |> List.exists (fun n -> n.Id = guest.NpcId && n.Dialogue |> List.exists (fun d -> d.Id = id))
                        )
                        ->
                        sink.Error("festival.unknownDialogue", guestPath + ".dialogueId", sprintf "Missing festival dialogue %s" id, target)
                    | _ -> ())

                match program.ShopId, program.ShopTile with
                | Some id, Some tile ->
                    if not (context.ShopIds.Contains id) then
                        sink.Error("festival.unknownShop", path + ".shopId", sprintf "Missing festival shop %s" id, target)

                    point "shopTile" program.SceneId tile.X tile.Y
                | None, None -> ()
                | _ -> sink.Error("festival.shopTile", path + ".shopTile", "A festival shop needs both a shop and a tile", target)

                match program.Activity with
                | None -> ()
                | Some activity ->
                    point "activity" program.SceneId activity.X activity.Y

                    let kinds =
                        [
                            activity.MinigameId.IsSome
                            activity.CutsceneId.IsSome
                            activity.ActionId.IsSome
                            activity.Contest.IsSome
                        ]
                        |> List.filter id
                        |> List.length

                    if kinds <> 1 then
                        sink.Error("festival.activityKind", path + ".activity", "Choose exactly one action, cutscene, minigame or contest", target)

                    if activity.X = program.ArrivalX && activity.Y = program.ArrivalY then
                        sink.Error("festival.activityAtExit", path + ".activity", "The activity overlaps the festival exit", target)

                    match activity.MinigameId with
                    | Some id when not (context.MinigameIds.Contains id) ->
                        sink.Error("festival.unknownMinigame", path + ".activity.minigameId", sprintf "Missing minigame %s" id, target)
                    | _ -> ()

                    match activity.CutsceneId with
                    | Some id when not (context.CutsceneIds.Contains id) ->
                        sink.Error("festival.unknownCutscene", path + ".activity.cutsceneId", sprintf "Missing cutscene %s" id, target)
                    | _ -> ()

                    match activity.ActionId with
                    | Some id when not (context.ActionIds.Contains id) ->
                        sink.Error("festival.unknownAction", path + ".activity.actionId", sprintf "Missing action %s" id, target)
                    | _ -> ()

                    match activity.Contest with
                    | Some contest ->
                        if not (context.ItemIds.Contains contest.ItemId) then
                            sink.Error("festival.unknownItem", path + ".activity.contest.itemId", sprintf "Missing contest item %s" contest.ItemId, target)

                        if missing context.QualityIds contest.MinimumQuality then
                            sink.Error("festival.unknownQuality", path + ".activity.contest.minimumQuality", "The minimum quality does not exist", target)

                        outcomes context sink "festival" (path + ".activity.contest.rewards") festival.Name program.SceneId target contest.Rewards
                    | None -> ())

    let run (context: Context) (sink: Sink) =
        festivalPrograms context sink

        context.Project.Quests
        |> List.iteri (fun i quest ->
            outcomes context sink "quest" ($"quests[{i}].onFail") quest.Name "" (Some(NavigationTarget.Quest quest.Id)) (defaultArg quest.OnFail []))

        duplicates context sink
        items context sink
        crops context sink
        npcs context sink
        quests context sink
        events context sink
        heartEvents context sink
        lines context sink
        relationships context sink
        collections context sink
        bundles context sink
        achievements context sink
        mail context sink
        monsters context sink
        actions context sink
        actionCycles context sink
        minigames context sink
        shops context sink
        recipes context sink
        machineTypes context sink
        nodeTypes context sink
        wildlife context sink
        graphics context sink
        packs context sink
        offGridNumbers context sink
