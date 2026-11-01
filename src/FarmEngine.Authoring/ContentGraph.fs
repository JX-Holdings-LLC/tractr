namespace FarmEngine.Authoring

open System
open System.Collections.Generic
open FarmEngine.Schemas

/// What a graph entry is (#231).
module GraphKinds =
    [<Literal>]
    let Item = "item"

    [<Literal>]
    let Flag = "flag"

    [<Literal>]
    let Quest = "quest"

    [<Literal>]
    let Building = "building"

/// One way something is obtained (a source) or consumed (a use).
type GraphLink =
    {
        /// What gives or takes it: `shop`, `crop`, `recipe`, `drop`, `monster`, `fish`, `animal`,
        /// `mine`, `quest`, `outcome`, `dialogue`, `mail`, `start`, `placed`, `upgrade`, `board`,
        /// `autoStart` (sources); `ingredient`, `objective`, `gift`, `collection`, `bundle`,
        /// `building`, `upgrade`, `seed`, `condition`, `prerequisite`, `machine`, `use` (uses).
        Via: string
        /// The id of the giver or taker (a shop, a recipe, a quest…), empty for `start`.
        Id: string
        /// A readable line: "Sold by General Store", "Made by Jam (recipe)".
        Label: string
        /// The seasons the link works in; empty: every season.
        Seasons: string list
        /// What else the link needs, as (kind, id): a recipe's other ingredients, a crop's seed, the
        /// quest whose reward it is.
        Needs: (string * string) list
    }

/// An item, flag, quest or building with where it comes from and what it leads to.
type GraphEntry =
    {
        Kind: string
        Id: string
        Name: string
        Sources: GraphLink list
        Uses: GraphLink list
    }

/// The whole-game content graph (#231): built from the compiled content, so pack content counts.
type ContentGraph =
    {
        Entries: GraphEntry list
        /// (kind, id) of every entry some open source can reach, starting from what needs nothing.
        Obtainable: Set<string * string>
        /// Seasons of the calendar in order.
        Seasons: string list
    }

module ContentGraph =
    let private link via id label seasons needs =
        {
            Via = via
            Id = id
            Label = label
            Seasons = seasons
            Needs = needs
        }

    /// Every JSON object in `json`, depth first.
    let rec private objects (json: Json) : seq<(string * Json) list> =
        seq {
            match json with
            | JObject fields ->
                yield fields

                for _, value in fields do
                    yield! objects value
            | JArray values ->
                for value in values do
                    yield! objects value
            | _ -> ()
        }

    let private text (key: string) (fields: (string * Json) list) =
        match List.tryFind (fun (k, _) -> k = key) fields with
        | Some(_, JString value) when value <> "" -> Some value
        | _ -> None

    let private orEmpty (list: 'T list option) : 'T list = defaultArg list []

    /// The graph of `content` (compiled from `project`, whose start state it also reads).
    let build (project: GameProject) (content: GameContent) : ContentGraph =
        let sources = Dictionary<string * string, ResizeArray<GraphLink>>()
        let uses = Dictionary<string * string, ResizeArray<GraphLink>>()
        let names = Dictionary<string * string, string>()

        let add (table: Dictionary<_, ResizeArray<GraphLink>>) key value =
            match table.TryGetValue key with
            | true, list -> list.Add value
            | _ -> table.[key] <- ResizeArray [ value ]

        let source kind id l = add sources (kind, id) l
        let used kind id l = add uses (kind, id) l
        let item = GraphKinds.Item

        let ingredients (list: RecipeIngredient list) =
            list |> List.map (fun i -> item, i.ItemId)

        for i in content.Items do
            names.[(item, i.Id)] <- i.Name

        for q in content.Quests do
            names.[(GraphKinds.Quest, q.Id)] <- q.Name

        for b in orEmpty content.BuildingTypes do
            names.[(GraphKinds.Building, b.Id)] <- b.Name

        // ── Items: where they come from ──
        for slot in project.Player.Inventory do
            source item slot.Item.Id (link "start" "" "In the starting inventory" [] [])

        for scene in content.Scenes do
            for row in scene.Tiles do
                for tile in row do
                    tile.Item
                    |> Option.iter (fun i -> source item i.Id (link "placed" scene.Id (sprintf "Lies in %s" scene.Name) [] []))

        for shop in content.Shops do
            for entry in shop.Stock do
                source item entry.ItemId (link "shop" shop.Id (sprintf "Sold by %s" shop.Name) (orEmpty entry.Seasons) [])

            for upgrade in orEmpty shop.ToolUpgrades do
                source
                    item
                    upgrade.ToItemId
                    (link "upgrade" shop.Id (sprintf "Upgraded at %s" shop.Name) [] ((item, upgrade.FromItemId) :: ingredients upgrade.Materials))

                used item upgrade.FromItemId (link "upgrade" shop.Id (sprintf "Upgraded at %s" shop.Name) [] [])

                for m in upgrade.Materials do
                    used item m.ItemId (link "upgrade" shop.Id (sprintf "Tool upgrade material at %s" shop.Name) [] [])

            for building in orEmpty shop.Buildings do
                source GraphKinds.Building building (link "shop" shop.Id (sprintf "Built by %s" shop.Name) [] [])

        let seedsOf = Dictionary<string, string list>()

        for i in content.Items do
            match i.Type, i.CropType with
            | t, Some crop when t = ItemTypes.Seed ->
                seedsOf.[crop] <-
                    i.Id
                    :: (match seedsOf.TryGetValue crop with
                        | true, l -> l
                        | _ -> [])
            | _ -> ()

        let itemIds = content.Items |> List.map (fun i -> i.Id) |> HashSet

        let plantedOnMap =
            content.Scenes
            |> Seq.collect (fun s -> s.Tiles |> Seq.concat)
            |> Seq.choose (fun t -> t.Crop |> Option.map (fun c -> c.Type))
            |> HashSet

        for cropId, crop in content.Crops do
            let harvest =
                match crop.HarvestItemId with
                | Some id when id <> "" -> Some id
                | _ ->
                    let candidates =
                        [
                            yield "crop-" + cropId
                            match cropId.Split(':') with
                            | [| pack; local |] when pack <> "" && local <> "" -> yield pack + ":crop-" + local
                            | _ -> ()
                        ]

                    candidates |> List.tryFind itemIds.Contains

            match harvest with
            | None -> ()
            | Some harvest ->
                match seedsOf.TryGetValue cropId with
                | true, seeds ->
                    for seed in seeds do
                        source item harvest (link "crop" cropId (sprintf "Harvested from %s" crop.Name) crop.Seasons [ item, seed ])
                        used item seed (link "seed" cropId (sprintf "Plants %s" crop.Name) crop.Seasons [])
                | _ when plantedOnMap.Contains cropId ->
                    source item harvest (link "crop" cropId (sprintf "Harvested from %s on the map" crop.Name) crop.Seasons [])
                | _ -> ()

        for recipe in content.Recipes do
            let unlockQuest =
                recipe.Unlock
                |> Option.bind (fun u -> u.QuestId)
                |> Option.map (fun q -> GraphKinds.Quest, q)
                |> Option.toList

            let seasons = recipe.Unlock |> Option.bind (fun u -> u.Seasons) |> orEmpty

            let where =
                if recipe.MachineTypeId.IsSome then
                    "machine recipe"
                else
                    "recipe"

            for output in recipe.Outputs do
                source
                    item
                    output.ItemId
                    (link "recipe" recipe.Id (sprintf "Made by %s (%s)" recipe.Name where) seasons (ingredients recipe.Inputs @ unlockQuest))

            for input in recipe.Inputs do
                used item input.ItemId (link "ingredient" recipe.Id (sprintf "Ingredient of %s (%s)" recipe.Name where) seasons [])

        for node in content.NodeTypes do
            for drop in node.Drops do
                let seasons = drop.Seasons |> Option.orElse node.Seasons |> orEmpty
                source item drop.ItemId (link "drop" node.Id (sprintf "Drops from %s" node.Name) seasons [])

        for monster in orEmpty content.Monsters do
            for drop in monster.Drops do
                source item drop.ItemId (link "monster" monster.Id (sprintf "Dropped by %s" monster.Name) (orEmpty drop.Seasons) [])

        for species in content.AnimalSpecies do
            let products =
                (if species.ProductItemId <> "" then
                     [ species.ProductItemId ]
                 else
                     [])
                @ (orEmpty species.Products |> List.map (fun p -> p.ItemId))

            for product in List.distinct products do
                source item product (link "animal" species.Id (sprintf "Produced by %s" species.Name) [] [])

            species.FeedItemId
            |> Option.iter (fun feed -> used item feed (link "use" species.Id (sprintf "Feeds %s" species.Name) [] []))

        for table in content.FishTables do
            for entry in table.Entries do
                source item entry.ItemId (link "fish" table.Id (sprintf "Caught in %s" table.Name) (orEmpty table.Seasons) [])

            table.JunkItemId
            |> Option.iter (fun junk -> source item junk (link "fish" table.Id (sprintf "Fished up in %s" table.Name) [] []))

        let mineConfigs =
            content.Mine :: (orEmpty content.Mines |> List.map (fun m -> m.Config))

        for config in mineConfigs do
            for band in config.Bands do
                for treasure in orEmpty band.Treasure do
                    source item treasure.ItemId (link "mine" "" "Found in the mine" [] [])

        for mail in orEmpty content.Mail do
            for attachment in mail.Attachments do
                source item attachment.ItemId (link "mail" mail.Id (sprintf "Sent in the letter \"%s\"" mail.Subject) [] [])

        // ── Quests ──
        let quest = GraphKinds.Quest

        for q in content.Quests do
            for reward in orEmpty q.Rewards.Items do
                source item reward.ItemId (link "quest" q.Id (sprintf "Reward of %s" q.Name) [] [ quest, q.Id ])

            let prerequisites = orEmpty q.Prerequisites |> List.map (fun p -> quest, p)

            if q.AutoStart = Some true then
                source quest q.Id (link "autoStart" q.Id "Starts on its own" (orEmpty q.AvailableSeasons) prerequisites)

            for p in orEmpty q.Prerequisites do
                used quest p (link "prerequisite" q.Id (sprintf "Needed before %s" q.Name) [] [])

            for objective in q.Objectives do
                objective.TargetItemId
                |> Option.iter (fun target -> used item target (link "objective" q.Id (sprintf "Asked for by %s" q.Name) (orEmpty q.AvailableSeasons) []))

        for id in project.Player.ActiveQuests @ project.Player.CompletedQuests do
            source quest id (link "start" "" "Active when a new game starts" [] [])

        for board in orEmpty content.RequestBoards do
            for entry in board.Entries do
                source quest entry.QuestId (link "board" board.Id (sprintf "Offered on %s" board.Name) [] [])

        // ── Gifts, collections, bundles, buildings, machines, items used directly ──
        for npc in content.Npcs do
            match npc.GiftTastes with
            | None -> ()
            | Some tastes ->
                for id in tastes.Loved @ tastes.Liked do
                    used item id (link "gift" npc.Id (sprintf "A gift %s likes" npc.Name) [] [])

        for collection in orEmpty content.Collections do
            for id in collection.Items do
                used item id (link "collection" collection.Id (sprintf "Part of the %s collection" collection.Name) [] [])

        for bundle in orEmpty content.Bundles do
            for b in bundle.Items do
                used item b.ItemId (link "bundle" bundle.Id (sprintf "Asked for by the %s bundle" bundle.Name) [] [])

        for building in orEmpty content.BuildingTypes do
            for m in building.Materials do
                used item m.ItemId (link "building" building.Id (sprintf "Material of %s" building.Name) [] [])

            building.UpgradeOf
            |> Option.iter (fun previous -> used GraphKinds.Building previous (link "building" building.Id (sprintf "Upgrades to %s" building.Name) [] []))

        for machine in content.MachineTypes do
            machine.ItemId
            |> Option.iter (fun id -> used item id (link "machine" machine.Id (sprintf "Placed as %s" machine.Name) [] []))

        for i in content.Items do
            let direct =
                i.Type = ItemTypes.Tool
                || i.Type = ItemTypes.Fertilizer
                || i.ToolType.IsSome
                || i.UseActionId.IsSome
                || i.Energy.IsSome
                || i.Wear.IsSome
                || i.FishingGear.IsSome
                || i.Health.IsSome
                || not (List.isEmpty (orEmpty i.Buffs))

            if direct then
                used item i.Id (link "use" i.Id "Used, worn or eaten by the player" [] [])

        // ── Outcomes, dialogue and conditions anywhere in the content ──
        let flag = GraphKinds.Flag

        for name, _ in project.EventFlags do
            source flag name (link "start" "" "Set when a new game starts" [] [])

        for event in content.Events do
            source flag (EventsSchema.EventFiredFlag event.Id) (link "outcome" event.Id (sprintf "Set when %s fires" event.Name) [] [])

        for fields in objects (SchemaJson.encodeGameContent content) do
            match text "type" fields with
            | Some t when t = EventOutcomeTypes.GiveItem ->
                text "itemId" fields
                |> Option.iter (fun id -> source item id (link "outcome" "" "Given by an event, action or reward" [] []))
            | Some t when t = EventOutcomeTypes.TakeItem ->
                text "itemId" fields
                |> Option.iter (fun id -> used item id (link "outcome" "" "Taken by an event or action" [] []))
            | Some t when t = EventOutcomeTypes.SetFlag ->
                text "flagName" fields
                |> Option.iter (fun id -> source flag id (link "outcome" "" "Set by an event, action or reward" [] []))
            | Some t when t = EventOutcomeTypes.SetVariable || t = EventOutcomeTypes.AddVariable ->
                text "variable" fields
                |> Option.iter (fun id -> source flag id (link "outcome" "" "Set by an event, action or reward" [] []))
            | Some t when t = EventOutcomeTypes.StartQuest ->
                text "questId" fields
                |> Option.iter (fun id -> source quest id (link "outcome" "" "Started by an event, action or reward" [] []))
            | Some "flag" ->
                text "flag" fields
                |> Option.iter (fun id -> used flag id (link "condition" "" "Checked by a condition" [] []))
            | Some "variable" ->
                text "variable" fields
                |> Option.iter (fun id -> used flag id (link "condition" "" "Checked by a condition" [] []))
            | Some "hasItem" ->
                text "itemId" fields
                |> Option.iter (fun id -> used item id (link "condition" "" "Checked by a condition" [] []))
            | Some "questStatus" ->
                text "questId" fields
                |> Option.iter (fun id -> used quest id (link "condition" "" "Checked by a condition" [] []))
            | _ -> ()

            // Dialogue options (they have `text` and no `type`).
            if (text "type" fields).IsNone then
                text "giveItem" fields
                |> Option.iter (fun id -> source item id (link "dialogue" "" "Given in a conversation" [] []))

                text "requiresItem" fields
                |> Option.iter (fun id -> used item id (link "dialogue" "" "Asked for in a conversation" [] []))

                text "eventFlag" fields
                |> Option.iter (fun id -> source flag id (link "dialogue" "" "Set in a conversation" [] []))

                text "offerQuestId" fields
                |> Option.iter (fun id -> source quest id (link "dialogue" "" "Offered in a conversation" [] []))

                for key in [ "requiresFlag"; "hiddenIfFlag"; "visibleFlag" ] do
                    text key fields
                    |> Option.iter (fun id -> used flag id (link "condition" "" "Checked in a conversation or panel" [] []))

        // ── Entries ──
        let keys =
            Seq.append sources.Keys uses.Keys
            |> Seq.append (names.Keys)
            |> Seq.distinct
            |> Seq.sortWith (fun (k1, i1) (k2, i2) ->
                match String.CompareOrdinal(k1, k2) with
                | 0 -> String.CompareOrdinal(i1, i2)
                | c -> c)
            |> List.ofSeq

        let listOf (table: Dictionary<_, ResizeArray<GraphLink>>) key =
            match table.TryGetValue key with
            | true, list -> List.ofSeq list |> List.distinct
            | _ -> []

        let entries =
            keys
            |> List.map (fun ((kind, id) as key) ->
                {
                    Kind = kind
                    Id = id
                    Name =
                        (match names.TryGetValue key with
                         | true, name when name <> "" -> name
                         | _ -> id)
                    Sources = listOf sources key
                    Uses = listOf uses key
                })

        // What can be reached: sources whose needs are reached, until nothing changes.
        let mutable reached = Set.empty

        let mutable changed = true

        while changed do
            changed <- false

            for entry in entries do
                let key = entry.Kind, entry.Id

                if
                    not (reached.Contains key)
                    && entry.Sources |> List.exists (fun s -> s.Needs |> List.forall reached.Contains)
                then
                    reached <- reached.Add key
                    changed <- true

        {
            Entries = entries
            Obtainable = reached
            Seasons = content.Settings.Calendar.Seasons |> List.map (fun s -> s.Id)
        }

    let tryFind (kind: string) (id: string) (graph: ContentGraph) : GraphEntry option =
        graph.Entries |> List.tryFind (fun e -> e.Kind = kind && e.Id = id)

    /// The graph of a project (compiles it).
    let ofProject (project: GameProject) : ContentGraph =
        build project (ContentCompiler.compile project)

    /// The seasons an entry can first be had in, from its open sources: `None` when some open
    /// source works in every season (or nothing gives it).
    let seasonsOf (graph: ContentGraph) (entry: GraphEntry) : string list option =
        let open' =
            entry.Sources
            |> List.filter (fun s -> s.Needs |> List.forall graph.Obtainable.Contains)

        if List.isEmpty open' || open' |> List.exists (fun s -> List.isEmpty s.Seasons) then
            None
        else
            Some(open' |> List.collect (fun s -> s.Seasons) |> List.distinct)

    /// Quest prerequisite cycles, each as the quest ids around it (the smallest id first, once).
    let prerequisiteCycles (content: GameContent) : string list list =
        let prerequisites =
            content.Quests
            |> List.map (fun q -> q.Id, defaultArg q.Prerequisites [])
            |> Map.ofList

        let found = HashSet<string>()
        let cycles = ResizeArray<string list>()

        let rec walk (path: string list) (id: string) =
            match List.tryFindIndex ((=) id) path with
            | Some index ->
                // `path` is newest first: the cycle is the part up to `id`.
                let cycle = path |> List.take (index + 1) |> List.rev
                let start = List.min cycle
                let at = List.findIndex ((=) start) cycle
                let rotated = (List.skip at cycle) @ (List.take at cycle)
                let key = String.Join("\u0000", rotated)

                if found.Add key then
                    cycles.Add rotated
            | None ->
                if List.length path < 64 then
                    for next in defaultArg (Map.tryFind id prerequisites) [] do
                        if next <> id then
                            walk (id :: path) next

        for q in content.Quests do
            walk [] q.Id

        List.ofSeq cycles
