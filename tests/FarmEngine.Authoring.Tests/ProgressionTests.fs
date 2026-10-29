module FarmEngine.Authoring.Tests.ProgressionTests

open Xunit
open FarmEngine.Authoring
open FarmEngine.Authoring.Tests.TestProjects
open FarmEngine.Schemas

[<Fact>]
let ``progression records and nested buffs roundtrip and compile`` () =
    let project = starter ()

    let buff =
        { BuffDefinition.Default with
            Id = "coffee"
            Stat = "moveSpeed"
            Amount = 20.0
            DurationMinutes = 60.0
        }

    let skill =
        { SkillDefinition.Default with
            Id = "gardening"
            Name = "Gardening"
            Training =
                [
                    {
                        Action = "harvest"
                        Subject = None
                        Experience = 12.0
                    }
                ]
            Perks =
                Some
                    [
                        { SkillPerk.Default with
                            Id = "profession"
                            Choices =
                                [
                                    { SkillPerkChoice.Default with
                                        Id = "grower"
                                        Name = "Grower"
                                        Stat = "yield"
                                        Amount = 2.0
                                    }
                                ]
                        }
                    ]
        }

    let draft =
        { ProgressionSettings.Default with
            Limits = Some ProgressionLimits.Default
            Skills = Some [ skill ]
        }

    let project = project |> apply (SettingsForm.Progression(project, draft))

    let project =
        { project with
            Items =
                { Item.Default with
                    Id = "coffee"
                    Name = "Coffee"
                    Buffs = Some [ buff ]
                }
                :: project.Items
        }

    let encoded = SchemaJson.encodeGameProject project

    match Decode.run SchemaJson.decodeGameProject encoded with
    | Error error -> failwith error
    | Ok restored -> Assert.Equal(project, restored)

    Assert.Equal(project.Settings.SkillDefinitions, (ContentCompiler.compile project).Settings.SkillDefinitions)

    Assert.Contains(
        "gardening",
        References.options ReferenceKind.Skill project
        |> List.map (fun option -> option.Id)
    )

[<Fact>]
let ``progression limits and invalid tool chains are reported`` () =
    let project = starter ()

    let project =
        { project with
            Settings =
                { project.Settings with
                    Progression =
                        Some
                            { ProgressionLimits.Default with
                                MaxInventorySize = 2000.0
                            }
                }
            Shops =
                [
                    { ShopDefinition.Default with
                        Id = "smith"
                        ToolUpgrades =
                            Some
                                [
                                    { ToolUpgrade.Default with
                                        FromItemId = "missing"
                                        ToItemId = "missing-too"
                                    }
                                ]
                    }
                ]
        }

    Assert.Contains(SchemaChecks.projectIssues project, fun issue -> issue.Path = "settings.progression.maxInventorySize")
    Assert.Contains(SchemaChecks.projectLints project, fun issue -> issue.Path = "shops.0.toolUpgrades.0")

[<Fact>]
let ``keep playtest preserves optional player progression without adding empty fields`` () =
    let project = SchemaJson.encodeGameProject (starter ())

    let player =
        JObject
            [
                "health", JNumber 100.0
                "maxHealth", JNumber 150.0
                "buffs", JArray [ JObject [ "source", JString "coffee" ] ]
                "perks", JObject [ "farming", JString "grower" ]
                "purchasedUpgrades", JArray [ JString "bag" ]
                "toolUpgrades", JArray [ JObject [ "shopId", JString "smith" ] ]
            ]

    let changed = Playtest.applyStateJson project (JObject [ "player", player ])
    let goals = changed |> Json.get "keptState" |> Json.get "goals"

    for key in [ "health"; "maxHealth"; "buffs"; "perks"; "purchasedUpgrades"; "toolUpgrades" ] do
        Assert.Equal(Json.get key player, Json.get key goals)

    let legacy = SchemaJson.encodeProjectSettings ProjectSettings.Default
    Assert.Equal(None, Json.tryGet "progression" legacy)
    Assert.Equal(None, Json.tryGet "skillDefinitions" legacy)

[<Fact>]
let ``skill packs namespace definitions and event skill references`` () =
    let skill =
        { SkillDefinition.Default with
            Id = "gardening"
            Name = "Gardening"
        }

    let action =
        { ActionDef.Default with
            Id = "train"
            Outcomes =
                [
                    { EventOutcome.Default with
                        Type = "giveExperience"
                        Skill = Some "gardening"
                        Amount = Some 5.0
                    }
                ]
        }

    let pack =
        { ContentPack.Default with
            Manifest =
                { PackManifest.Default with
                    Id = "garden"
                }
            Content =
                { PackContent.Default with
                    Skills = Some [ skill ]
                    Actions = [ action ]
                }
        }

    let rewritten = PackRules.namespacePack pack
    Assert.Equal("garden:gardening", rewritten.Content.Skills.Value.Head.Id)
    Assert.Equal(Some "garden:gardening", rewritten.Content.Actions.Head.Outcomes.Head.Skill)


[<Fact>]
let ``classic skill preset materializes the legacy curve and action rewards`` () =
    let project = starter ()
    let preset = SettingsForm.ClassicSkills project
    let skills = preset.Skills.Value

    let skill id =
        skills |> List.find (fun skill -> skill.Id = id)

    let sumAt id stat level =
        (skill id).Rewards
        |> List.filter (fun reward -> reward.Stat = stat && reward.Level <= float level)
        |> List.sumBy _.Amount

    for level in 0 .. project.Settings.SkillLevelCurve.Length - 1 do
        Assert.Equal(float (level / 4), sumAt "farming" "yield" level)
        Assert.Equal(float (level / 3), sumAt "farming" "quality" level)
        Assert.Equal(float level, sumAt "combat" "attack" level)

    Assert.Equal(8.0, (skill "farming").Training.Head.Experience)
    Assert.Equal(8.0, (skill "fishing").Training.Head.Experience)
    Assert.Equal(4.0, (skill "social").Training.Head.Experience)
    let configured = project |> apply (SettingsForm.Progression(project, preset))
    Assert.Equal(preset.Skills, configured.Settings.SkillDefinitions)
    Assert.Empty(SchemaChecks.projectIssues configured)

[<Fact>]
let ``event buff payloads receive the same bounds checks as edible items`` () =
    let project = starter ()

    let project =
        { project with
            Events =
                [
                    { GameEvent.Default with
                        Id = "bad-buff"
                        Trigger = EventTriggers.Enter
                        Outcomes =
                            [
                                { EventOutcome.Default with
                                    Type = "applyBuff"
                                    Buff =
                                        Some
                                            { BuffDefinition.Default with
                                                Id = "bad"
                                                Stat = "unknown"
                                                DurationMinutes = 0.0
                                            }
                                }
                            ]
                    }
                ]
        }

    let issues = SchemaChecks.projectIssues project
    Assert.Contains(issues, fun issue -> issue.Path = "events.0.outcomes.0.buff.stat")
    Assert.Contains(issues, fun issue -> issue.Path = "events.0.outcomes.0.buff.durationMinutes")
