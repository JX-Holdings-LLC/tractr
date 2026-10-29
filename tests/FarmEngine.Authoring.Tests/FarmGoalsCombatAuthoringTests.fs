module FarmEngine.Authoring.Tests.FarmGoalsCombatAuthoringTests

// M5–M7 in the authoring core: the new content types and their edits, what removing one takes
// with it, the Problems the new records raise, the compiled content and start state, the farm,
// combat and mine band settings, a scene's greenhouse flag and monsters, and Keep changes for the
// new state.

open Xunit
open FarmEngine.Authoring
open FarmEngine.Authoring.Tests.TestProjects
open FarmEngine.Schemas

let private codes (project: GameProject) =
    Problems.collect project |> List.map (fun p -> p.Code)

let private parse (text: string) =
    match Json.parse text with
    | Ok json -> json
    | Error error -> failwith error

/// The starter farm with a barn (an interior scene), a placed barn, a cow housed in it, a
/// builder shop, a monster in the mine and in a scene, a collection, a bundle, an achievement and
/// a letter, all consistent.
let private farmGame () =
    let project = starter ()
    let item = project.Items.Head.Id
    let interior = Defaults.newScene project "Barn inside" 8 6
    let project = project |> apply (AddScene interior)

    let barn =
        { Defaults.newBuildingType project with
            Id = "barn"
            Name = "Barn"
            Width = 4.0
            Height = 3.0
            InteriorSceneId = Some interior.Id
            Capacity = 4.0
            Materials = [ ({ ItemId = item; Quantity = 10.0 }: RecipeIngredient) ]
        }

    let bigBarn =
        { barn with
            Id = "big-barn"
            Name = "Big Barn"
            UpgradeOf = Some "barn"
            Capacity = 8.0
        }

    let placed =
        { BuildingState.Default with
            Id = "building-1"
            BuildingId = "barn"
            SceneId = "scene-farm"
            X = 1.0
            Y = 1.0
            InteriorSceneId = interior.Id
        }

    let cow =
        { Defaults.newAnimalSpecies project with
            Id = "cow"
            Name = "Cow"
            ProductItemId = item
            Housing = Some [ "barn" ]
            Products =
                Some
                    [
                        { AnimalProduct.Default with
                            ItemId = item
                            IntervalDays = 2.0
                        }
                    ]
            MoodQualities =
                Some
                    [
                        {
                            MinMood = 80.0
                            Quality = CropQualities.Gold
                        }
                    ]
            Breeding = Some AnimalBreeding.Default
        }

    let slime =
        { Defaults.newMonster project with
            Id = "slime"
            Name = "Slime"
            Drops =
                [
                    { Defaults.newDrop project with
                        ItemId = item
                    }
                ]
        }

    let letter =
        { Defaults.newMail project with
            Id = "mail-welcome"
            Attachments = [ ({ ItemId = item; Quantity = 1.0 }: RecipeIngredient) ]
        }

    let museum =
        { Defaults.newCollection project with
            Id = "museum"
            Items = [ item ]
            Rewards =
                [
                    { CollectionReward.Default with
                        Count = 1.0
                        Message = "Thanks!"
                    }
                ]
        }

    let pantry =
        { Defaults.newBundle project with
            Id = "pantry"
            Items =
                [
                    { BundleItem.Default with
                        ItemId = item
                        Quality = Some CropQualities.Silver
                    }
                ]
            Required = Some 1.0
        }

    let rich =
        { Defaults.newAchievement project with
            Id = "rich"
            Conditions = [ EventCondition.Bundle { BundleId = "pantry" } ]
        }

    let shop =
        { project.Shops.Head with
            Buildings = Some [ "barn"; "big-barn" ]
            Animals = Some [ "cow" ]
        }

    let band =
        { MineBand.Default with
            FromFloor = 1.0
            ToFloor = 5.0
            Monsters = Some [ { MonsterId = "slime"; Weight = 1.0 } ]
            MonsterCount = Some 2.0
            Treasure = Some [ { ItemId = item; Weight = 1.0 } ]
            TreasureCount = Some 1.0
        }

    project
    |> apply (UpsertBuildingType barn)
    |> apply (UpsertBuildingType bigBarn)
    |> apply (UpsertPlacedBuilding placed)
    |> apply (UpsertAnimalSpecies cow)
    |> apply (
        UpsertAnimal
            { Defaults.newAnimal project "cow" "scene-farm" 2 2 with
                BuildingId = Some "building-1"
            }
    )
    |> apply (UpsertMonster slime)
    |> apply (UpsertMail letter)
    |> apply (UpsertCollection museum)
    |> apply (UpsertBundle pantry)
    |> apply (UpsertAchievement rich)
    |> apply (UpsertShop shop)
    |> apply (SetMine { project.Mine with Bands = [ band ] })
    |> apply (
        SetSceneMonsters(
            "scene-farm",
            [
                { MonsterSpawn.Default with
                    MonsterId = "slime"
                    Count = 2.0
                }
            ]
        )
    )
    |> fun p -> p |> apply (SettingsForm.Combat(p, SettingsForm.DefaultCombat))

/// The codes of the problems whose code starts with one of the M5–M7 families.
let private newCodes (project: GameProject) =
    let families =
        [
            "building."
            "placedBuilding."
            "monster."
            "collection."
            "bundle."
            "achievement."
            "mail."
            "farm."
            "combat."
            "species."
            "animal."
            "shop.unknown"
            "mine.band"
            "mine.hazard"
            "scene.unknownMonster"
            "scene.monster"
            "quest.unknownMail"
            "machineType.storage"
            "machineType.unknownAreaEffect"
            "machineType.areaRadius"
            "terrain.damage"
            "duplicate."
        ]

    Problems.collect project
    |> List.filter (fun p ->
        families |> List.exists (fun f -> p.Code.StartsWith f)
        || p.Code.Contains "UnknownMail"
        || p.Code.Contains "UnknownBuilding")
    |> List.filter (fun p -> p.Severity <> Severity.Info)

[<Fact>]
let ``a consistent farm, goals and combat setup raises none of the new problems`` () =
    let project = farmGame ()
    Assert.True((newCodes project).IsEmpty, describe (newCodes project))

[<Fact>]
let ``the new content types are added, duplicated and removed, and an empty list is absent`` () =
    let project = starter ()
    let monster = Defaults.newMonster project
    let withMonster = project |> apply (UpsertMonster monster)
    Assert.Equal<MonsterDefinition list>([ monster ], withMonster.Monsters.Value)
    Assert.Contains(monster.Id, Defaults.allIds withMonster)
    let copy = Defaults.duplicateMonster withMonster monster
    Assert.NotEqual<string>(monster.Id, copy.Id)
    Assert.Equal("New Monster (copy)", copy.Name)
    Assert.True((withMonster |> apply (RemoveMonster monster.Id)).Monsters.IsNone)
    // Each new list starts absent, so a project without them encodes as before.
    let json = Json.stringify (SchemaJson.encodeGameProject project)

    for key in
        [
            "buildingTypes"
            "\"buildings\""
            "collections"
            "bundles"
            "achievements"
            "\"mail\""
            "monsters"
            "allSeasons"
            "areaEffects"
        ] do
        Assert.DoesNotContain(key, json)

    Assert.Equal(None, Json.tryGet "health" (SchemaJson.encodePlayer project.Player))
    let letter = Defaults.newMail project
    Assert.Equal(project.Npcs.Head.Id, letter.From)
    Assert.Equal("A letter (copy)", (Defaults.duplicateMail project letter).Subject)

[<Fact>]
let ``removing a building type takes its placed buildings, shop entries, housing and upgrades with it`` () =
    let project = farmGame () |> apply (RemoveBuildingType "barn")
    Assert.True(project.Buildings.IsNone)
    Assert.Equal<string list>([ "big-barn" ], project.Shops.Head.Buildings.Value)
    Assert.True((project.AnimalSpecies |> List.find (fun s -> s.Id = "cow")).Housing.IsNone)
    Assert.Equal(None, (project.BuildingTypes.Value |> List.exactlyOne).UpgradeOf)
    // The cow lived in the removed building: it has no home now.
    Assert.Equal(None, (project.Animals |> List.find (fun a -> a.SpeciesId = "cow")).BuildingId)

[<Fact>]
let ``removing a monster, a letter or an item scrubs what named it`` () =
    let game = farmGame ()
    let noSlime = game |> apply (RemoveMonster "slime")
    Assert.True((farm noSlime).Monsters.IsNone)
    Assert.True(noSlime.Mine.Bands.Head.Monsters.IsNone)

    let sender =
        { Defaults.newEvent game with
            Id = "event-mail"
            Outcomes =
                [
                    { EventOutcome.Default with
                        Type = EventOutcomeTypes.SendMail
                        MailId = Some "mail-welcome"
                    }
                ]
        }

    let quest =
        { game.Quests.Head with
            Rewards =
                { game.Quests.Head.Rewards with
                    MailId = Some "mail-welcome"
                }
        }

    let noMail =
        game
        |> apply (UpsertEvent sender)
        |> apply (UpsertQuest quest)
        |> apply (RemoveMail "mail-welcome")

    Assert.Empty((noMail.Events |> List.find (fun e -> e.Id = "event-mail")).Outcomes)
    Assert.Equal(None, (noMail.Quests |> List.find (fun q -> q.Id = quest.Id)).Rewards.MailId)
    let item = game.Items.Head.Id
    let noItem = game |> apply (RemoveItem item)
    Assert.Empty(noItem.Collections.Value.Head.Items)
    Assert.Empty(noItem.Bundles.Value.Head.Items)
    Assert.Empty(noItem.Mail.Value.Head.Attachments)
    Assert.Empty(noItem.Monsters.Value.Head.Drops)
    Assert.Empty((noItem.BuildingTypes.Value |> List.find (fun b -> b.Id = "barn")).Materials)
    Assert.True(noItem.Mine.Bands.Head.Treasure.IsNone)

[<Fact>]
let ``removing a scene drops the buildings standing in it and the interiors it held`` () =
    let game = farmGame ()
    let interior = game.BuildingTypes.Value.Head.InteriorSceneId.Value
    let removed = game |> apply (RemoveScene interior)
    Assert.True(removed.BuildingTypes.Value |> List.forall (fun b -> b.InteriorSceneId.IsNone))
    Assert.Equal("", removed.Buildings.Value.Head.InteriorSceneId)

[<Fact>]
let ``buildings must fit, open into scenes that exist and upgrade without a circle`` () =
    let game = farmGame ()

    let broken =
        game
        |> apply (
            UpsertBuildingType
                { game.BuildingTypes.Value.Head with
                    Width = 20.0
                    InteriorSceneId = Some "scene-ghost"
                    Scenes = [ "scene-ghost" ]
                    UpgradeOf = Some "big-barn"
                }
        )
        |> apply (
            UpsertBuildingType
                { game.BuildingTypes.Value.[1] with
                    Id = "silo"
                    UpgradeOf = Some "tower"
                }
        )
        |> apply (
            UpsertPlacedBuilding
                { BuildingState.Default with
                    Id = "building-2"
                    BuildingId = "big-barn"
                    SceneId = "scene-farm"
                    X = 999.0
                    Y = 1.0
                }
        )
        |> apply (
            UpsertPlacedBuilding
                { BuildingState.Default with
                    Id = "building-3"
                    BuildingId = "tower"
                    SceneId = "scene-farm"
                }
        )

    let found = codes broken

    for code in
        [
            "building.size"
            "building.unknownInterior"
            "building.unknownScene"
            "building.upgradeCycle"
            "building.unknownUpgrade"
            "placedBuilding.outOfBounds"
            "placedBuilding.unknownType"
        ] do
        Assert.Contains(code, found)

[<Fact>]
let ``housing, products, mood qualities and breeding name what exists`` () =
    let game = farmGame ()
    let cow = game.AnimalSpecies |> List.find (fun s -> s.Id = "cow")

    let broken =
        { cow with
            Housing = Some [ "castle" ]
            Products =
                Some
                    [
                        { AnimalProduct.Default with
                            ItemId = "item-ghost"
                            IntervalDays = 0.0
                        }
                    ]
            MoodQualities = Some [ { MinMood = 50.0; Quality = "platinum" } ]
            Breeding = Some { Chance = 2.0; MinMood = 70.0 }
        }

    let animal = game.Animals |> List.find (fun a -> a.SpeciesId = "cow")

    let found =
        codes (
            game
            |> apply (UpsertAnimalSpecies broken)
            |> apply (
                UpsertAnimal
                    { animal with
                        BuildingId = Some "building-ghost"
                    }
            )
        )

    for code in
        [
            "species.unknownHousing"
            "species.unknownProduct"
            "species.productInterval"
            "species.unknownMoodQuality"
            "species.breedingChance"
            "animal.unknownBuilding"
        ] do
        Assert.Contains(code, found)

[<Fact>]
let ``the farm and combat settings, monsters and mine bands are checked`` () =
    let game = farmGame ()

    let farmRules =
        { FarmSettings.Default with
            SceneId = "scene-ghost"
            PestChance = 1.5
            Layouts =
                [
                    { FarmLayout.Default with
                        Id = "a"
                        SceneId = "scene-ghost"
                    }
                    { FarmLayout.Default with
                        Id = "a"
                        SceneId = "scene-farm"
                    }
                ]
        }

    let combatRules =
        { SettingsForm.DefaultCombat with
            MaxHealth = 0.0
            FaintSceneId = Some "scene-ghost"
        }

    let monster =
        { game.Monsters.Value.Head with
            Health = 0.0
            Movement = "teleport"
            Drops =
                [
                    { Defaults.newDrop game with
                        ItemId = "item-ghost"
                    }
                ]
        }

    let band =
        { game.Mine.Bands.Head with
            Monsters = Some [ { MonsterId = "dragon"; Weight = 1.0 } ]
            Hazards = Some [ { TerrainId = "lava"; Density = 2.0 } ]
            Treasure = Some [ { ItemId = "gold-ghost"; Weight = 1.0 } ]
        }

    let broken =
        game
        |> fun p -> p |> apply (SettingsForm.Farm(p, farmRules))
        |> fun p -> p |> apply (SettingsForm.Combat(p, combatRules))
        |> apply (UpsertMonster monster)
        |> fun p -> p |> apply (SettingsForm.MineBands(p, [ band ]))
        |> apply (
            SetSceneMonsters(
                "scene-farm",
                [
                    { MonsterSpawn.Default with
                        MonsterId = "dragon"
                        X = Some -1.0
                        Y = Some 0.0
                    }
                ]
            )
        )

    let found = codes broken

    for code in
        [
            "farm.unknownScene"
            "farm.pestChance"
            "farm.duplicateLayout"
            "farm.layoutUnknownScene"
            "combat.maxHealth"
            "combat.unknownFaintScene"
            "monster.health"
            "monster.unknownMovement"
            "monster.unknownDrop"
            "mine.bandUnknownMonster"
            "mine.bandUnknownHazard"
            "mine.hazardDensity"
            "mine.bandUnknownTreasure"
            "scene.unknownMonster"
            "scene.monsterAreaOutOfBounds"
        ] do
        Assert.Contains(code, found)
    // Monsters with combat off are only a note.
    let off = game |> fun p -> p |> apply (SettingsForm.Combat(p, null))
    Assert.Contains("combat.off", codes off)

[<Fact>]
let ``collections, bundles, letters and achievements name what exists`` () =
    let game = farmGame ()

    let museum =
        { game.Collections.Value.Head with
            Items = [ "item-ghost" ]
            Kind = "stolen"
            SceneId = Some "scene-ghost"
            Rewards =
                [
                    { CollectionReward.Default with
                        Count = 5.0
                    }
                ]
        }

    let pantry =
        { game.Bundles.Value.Head with
            Items =
                [
                    { BundleItem.Default with
                        ItemId = "item-ghost"
                        Quality = Some "platinum"
                        Quantity = 0.0
                    }
                ]
            Required = Some 3.0
        }

    let letter =
        { game.Mail.Value.Head with
            From = "npc-ghost"
            Attachments =
                [
                    ({
                        ItemId = "item-ghost"
                        Quantity = 1.0
                    }
                    : RecipeIngredient)
                ]
        }

    let rich =
        { game.Achievements.Value.Head with
            Conditions =
                [
                    EventCondition.Bundle { BundleId = "bundle-ghost" }
                    EventCondition.Collection
                        {
                            CollectionId = "museum"
                            Min = Some 9.0
                        }
                    EventCondition.Slain { MonsterId = "dragon"; Min = 1.0 }
                    EventCondition.Building { BuildingId = "castle"; Min = 1.0 }
                    EventCondition.Birthday { NpcId = "npc-ghost" }
                    EventCondition.SkillLevel { Skill = "juggling"; Min = 2.0 }
                ]
        }

    let quest =
        { game.Quests.Head with
            Rewards =
                { game.Quests.Head.Rewards with
                    MailId = Some "mail-ghost"
                }
        }

    let sender =
        { Defaults.newEvent game with
            Id = "event-mail"
            Outcomes =
                [
                    { EventOutcome.Default with
                        Type = EventOutcomeTypes.SendMail
                        MailId = Some "mail-ghost"
                    }
                ]
        }

    let broken =
        game
        |> apply (UpsertCollection museum)
        |> apply (UpsertBundle pantry)
        |> apply (UpsertMail letter)
        |> apply (UpsertAchievement rich)
        |> apply (UpsertQuest quest)
        |> apply (UpsertEvent sender)

    let found = codes broken

    for code in
        [
            "collection.unknownItem"
            "collection.unknownKind"
            "collection.unknownScene"
            "collection.rewardUnreachable"
            "bundle.unknownItem"
            "bundle.unknownQuality"
            "bundle.quantity"
            "bundle.required"
            "mail.unknownNpc"
            "mail.unknownItem"
            "achievement.conditionUnknownBundle"
            "achievement.conditionCollectionMin"
            "achievement.conditionUnknownMonster"
            "achievement.conditionUnknownBuilding"
            "achievement.conditionUnknownNpc"
            "achievement.conditionUnknownSkill"
            "quest.unknownMail"
            "event.outcomeUnknownMail"
        ] do
        Assert.Contains(code, found)
    // A combat skill is known.
    let combat =
        { rich with
            Conditions = [ EventCondition.SkillLevel { Skill = "combat"; Min = 2.0 } ]
        }

    Assert.DoesNotContain("achievement.conditionUnknownSkill", codes (game |> apply (UpsertAchievement combat)))

[<Fact>]
let ``placed objects keep storage and area effects within what the engine applies`` () =
    let project = starter ()

    let sprinkler =
        { Defaults.newMachineType project with
            Storage = Some 1000.0
            AreaEffects = Some [ { Effect = "fire"; Radius = 99.0 } ]
        }

    let found = codes (project |> apply (UpsertMachineType sprinkler))

    for code in
        [
            "machineType.storage"
            "machineType.unknownAreaEffect"
            "machineType.areaRadius"
        ] do
        Assert.Contains(code, found)

[<Fact>]
let ``the content carries the new lists and the start state the buildings`` () =
    let game = farmGame ()
    let content = ContentCompiler.compile game
    Assert.Equal(game.BuildingTypes, content.BuildingTypes)
    Assert.Equal(game.Monsters, content.Monsters)
    Assert.Equal(game.Collections, content.Collections)
    Assert.Equal(game.Bundles, content.Bundles)
    Assert.Equal(game.Achievements, content.Achievements)
    Assert.Equal(game.Mail, content.Mail)
    let start = CartridgeCompiler.StartSection game
    Assert.Equal(Encode.list SchemaJson.encodeBuildingState game.Buildings.Value, Json.get "buildings" start)
    // Without them, the keys are absent, as serde skips empty lists.
    let plain = ContentCompiler.compile (starter ())
    Assert.True(plain.BuildingTypes.IsNone && plain.Monsters.IsNone && plain.Mail.IsNone)
    Assert.Equal(None, Json.tryGet "buildings" (CartridgeCompiler.StartSection(starter ())))
    Assert.Equal(None, Json.tryGet "buildingTypes" (SchemaJson.encodeGameContent plain))

[<Fact>]
let ``the farm, combat and mine band edits are one step each`` () =
    let project = starter ()

    let doc =
        Document.create project
        |> Document.apply (SettingsForm.Farm(project, SettingsForm.DefaultFarm project))

    Assert.Equal(Some project.StartSceneId, doc.Project.Settings.Farm |> Option.map (fun f -> f.SceneId))
    Assert.Equal(project, (Document.undo doc).Project)

    Assert.Same(
        doc.Project,
        (doc
         |> Document.apply (SettingsForm.Farm(doc.Project, SettingsForm.DefaultFarm project)))
            .Project
    )

    let combat =
        project |> apply (SettingsForm.Combat(project, SettingsForm.DefaultCombat))

    Assert.Equal(Some true, combat.Settings.Combat |> Option.map (fun c -> c.Enabled))

    let banded =
        project |> apply (SettingsForm.MineBands(project, [ MineBand.Default ]))

    Assert.Equal<MineBand list>([ MineBand.Default ], banded.Mine.Bands)
    Assert.Equal(project.Mine.Floors, banded.Mine.Floors)

[<Fact>]
let ``a scene's greenhouse flag and monsters are stored only when set`` () =
    let project = starter ()
    let greenhouse = project |> apply (SetSceneAllSeasons("scene-farm", true))
    Assert.Equal(Some true, (farm greenhouse).AllSeasons)
    Assert.Equal(None, (greenhouse |> apply (SetSceneAllSeasons("scene-farm", false)) |> farm).AllSeasons)

    let spawns =
        [
            { MonsterSpawn.Default with
                MonsterId = "slime"
                Count = 3.0
            }
        ]

    let haunted = project |> apply (SetSceneMonsters("scene-farm", spawns))
    Assert.Equal(Some spawns, (farm haunted).Monsters)
    Assert.Equal(None, (haunted |> apply (SetSceneMonsters("scene-farm", [])) |> farm).Monsters)

[<Fact>]
let ``monster spawns read back from their text`` () =
    let spawns =
        [
            { MonsterSpawn.Default with
                MonsterId = "slime"
                Count = 3.0
            }
            { MonsterSpawn.Default with
                MonsterId = "bat"
                Count = 2.0
                X = Some 4.0
                Y = Some 4.0
            }
            { MonsterSpawn.Default with
                MonsterId = "slime"
                Count = 1.0
                X = Some 2.0
                Y = Some 2.0
                X2 = Some 10.0
                Y2 = Some 8.0
            }
            { MonsterSpawn.Default with
                MonsterId = "ghost"
                Count = 1.0
                X2 = Some 9.0
            }
        ]

    let text = SettingsForms.monsterSpawnsText spawns
    Assert.Equal("slime 3\nbat 2 at 4,4\nslime 1 in 2,2-10,8\nghost 1 x2=9", text)
    Assert.Equal<MonsterSpawn list>(spawns, SettingsForms.parseMonsterSpawns text)
    Assert.Empty(SettingsForms.parseMonsterSpawns "  \n")

    Assert.Throws<System.FormatException>(fun () -> SettingsForms.parseMonsterSpawns "slime many" |> ignore)
    |> ignore

    Assert.Throws<System.FormatException>(fun () -> SettingsForms.parseMonsterSpawns "slime 2 at 4" |> ignore)
    |> ignore

[<Fact>]
let ``the new triggers' subjects and fields pick what they name`` () =
    Assert.Equal(Some(FieldRole.Reference(ReferenceKind.Building, Some "(any building)")), Vocabulary.roleFor "GameEvent" "Subject" EventTriggers.Build)
    Assert.Equal(Some(FieldRole.Reference(ReferenceKind.Monster, Some "(any monster)")), Vocabulary.roleFor "GameEvent" "Subject" EventTriggers.Slay)
    Assert.DoesNotContain("Subject", Vocabulary.hiddenProperties "GameEvent" EventTriggers.Bundle)
    let game = farmGame ()
    Assert.Equal<string list>([ "barn"; "big-barn" ], References.options ReferenceKind.Building game |> List.map (fun o -> o.Id))
    Assert.Equal<string list>([ "building-1" ], References.options ReferenceKind.PlacedBuilding game |> List.map (fun o -> o.Id))
    Assert.Equal<string list>([ "mail-welcome" ], References.options ReferenceKind.Mail game |> List.map (fun o -> o.Id))
    Assert.Equal(Some(FieldRole.ReferenceList ReferenceKind.Building), References.roleOf "AnimalSpeciesDefinition" "Housing")

    let spawn =
        Vocabulary.newElement "MonsterSpawn" game (box game) [] |> Option.get :?> MonsterSpawn

    Assert.Equal("slime", spawn.MonsterId)

    let layout =
        Vocabulary.newElement "FarmLayout" game (box game) [ "layout-1" ] |> Option.get :?> FarmLayout

    Assert.Equal("layout-2", layout.Id)
    // A new condition starts from the first thing of its kind.
    match Vocabulary.defaultCondition "bundle" game with
    | EventCondition.Bundle b -> Assert.Equal("pantry", b.BundleId)
    | other -> failwithf "unexpected %A" other

[<Fact>]
let ``keep changes writes the buildings and health back and keeps the goals`` () =
    let project = starter ()

    let state =
        parse
            """{"player":{"x":3,"y":4,"direction":"down","sceneId":"scene-farm","inventory":[],"maxInventorySize":24,"money":5,
                "energy":100,"maxEnergy":100,"skills":{},"activeQuests":[],"completedQuests":[],"health":42},
               "clock":{"tick":0,"timeMinutes":400,"day":1,"season":"spring","year":1,"weatherId":"sun"},
               "world":{"scenes":[]},"npcs":{},"quests":{},"flags":{},"animals":[],"social":{},"quarantinedItems":[],
               "mine":{"deepestFloor":0,"currentFloor":0},"rng":{"algorithm":"xoshiro128ss","s":[1,2,3,4]},
               "buildings":[{"id":"building-1","buildingId":"barn","sceneId":"scene-farm","x":2,"y":3,"interiorSceneId":"building-1","daysLeft":0}],
               "shipped":{"wheat":4},"achievements":{"rich":3},"mailQueue":["mail-welcome"],"nextMonster":0,"farmLayout":"layout-2"}"""

    match Playtest.applyState project state with
    | Error error -> failwith error
    | Ok kept ->
        Assert.Equal(Some 42.0, kept.Player.Health)
        Assert.Equal<string list>([ "building-1" ], kept.Buildings.Value |> List.map (fun b -> b.Id))
        let goals = Json.get "goals" kept.KeptState.Value
        Assert.Equal(parse """{"farmLayout":"layout-2","shipped":{"wheat":4},"achievements":{"rich":3},"mailQueue":["mail-welcome"],"health":42}""", goals)
    // Nothing of M5–M7 in the state: no goals, no buildings, no health.
    let quiet =
        parse
            """{"player":{"x":3,"y":4,"direction":"down","sceneId":"scene-farm","inventory":[],"maxInventorySize":24,"money":5,
                "energy":100,"maxEnergy":100,"skills":{},"activeQuests":[],"completedQuests":[]},
               "clock":{"tick":0,"timeMinutes":400,"day":1,"season":"spring","year":1,"weatherId":"sun"},
               "world":{"scenes":[]},"npcs":{},"quests":{},"flags":{},"animals":[],"social":{},"quarantinedItems":[],
               "mine":{"deepestFloor":0,"currentFloor":0},"rng":{"algorithm":"xoshiro128ss","s":[1,2,3,4]}}"""

    match Playtest.applyState (farmGame ()) quiet with
    | Error error -> failwith error
    | Ok kept ->
        Assert.True(kept.Buildings.IsNone)
        Assert.Equal(None, kept.Player.Health)
        Assert.True(kept.KeptState.IsNone)
