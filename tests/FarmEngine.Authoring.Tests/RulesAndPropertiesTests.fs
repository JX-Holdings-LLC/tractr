/// Game rules (#243) and properties (#244) in the authoring core: the checks, the edits, the
/// settings forms and what the cartridge carries.
module FarmEngine.Authoring.Tests.RulesAndPropertiesTests

open Xunit
open FarmEngine.Authoring
open FarmEngine.Schemas
open FarmEngine.Authoring.Tests.TestProjects

let private codes (prefix: string) (project: GameProject) =
    Problems.collect project
    |> List.filter (fun p -> p.Code.StartsWith(prefix, System.StringComparison.Ordinal))

let private withRules (rules: GameRules) (project: GameProject) =
    { project with
        Settings =
            { project.Settings with
                Rules = Some rules
            }
    }

let private workshop = SettingsForm.WorkshopRules

let private pressure =
    { PropertyDefinition.Default with
        Id = "pressure"
        Name = "Pressure"
        Scope = PropertyScopes.Scene
        Unit = Some "kPa"
        Initial = 90.0
        Rates = [ { PerHour = -6.0; Conditions = [] } ]
        Thresholds =
            [
                { PropertyThreshold.Default with
                    Compare = PropertyComparisons.Lt
                    Value = 50.0
                    Message = Some "The greenhouse is losing pressure!"
                }
            ]
        Hud = Some PropertyHudStyles.Meter
    }

let private oxygen =
    { PropertyDefinition.Default with
        Id = "oxygen"
        Name = "Oxygen"
        Unit = Some "%"
        Rates =
            [
                {
                    PerHour = -30.0
                    Conditions =
                        [
                            EventCondition.Variable
                                {
                                    Name = "pressure"
                                    Compare = VariableComparisons.Lt
                                    Value = Some 50.0
                                    Text = None
                                }
                        ]
                }
            ]
        Hud = Some PropertyHudStyles.Meter
    }

let private withProperties (properties: PropertyDefinition list) (project: GameProject) =
    apply (SetProperties properties) project

// ── Rules ────────────────────────────────────────────────────────────────────

[<Fact>]
let ``a project without rules, the classic rules and the workshop preset raise no problems`` () =
    let project = starter ()
    Assert.Empty(codes "rules." project)
    Assert.Empty(codes "rules." (withRules SettingsForm.DefaultRules project))
    // The starter farm has seeds, so the workshop preset says what farming being off leaves unused.
    let off = codes "rules." (withRules workshop project)
    let note = List.exactlyOne off
    Assert.Equal("rules.farmingOff", note.Code)
    Assert.Equal(Severity.Info, note.Severity)
    Assert.Contains("seed items", note.Message)
    // A game with nothing to farm with gets no note.
    let seedless =
        { blank () with
            Items = (blank ()).Items |> List.filter (fun item -> item.Type <> ItemTypes.Seed)
        }

    Assert.Empty(codes "rules." (withRules workshop seedless))
    Assert.Empty(errors (withRules workshop project))

[<Fact>]
let ``rules the engine cannot follow are reported`` () =
    let project = starter ()

    let broken =
        withRules
            { GameRules.Default with
                Sleep = Some "sometimes"
                DayEnd = Some "explode"
                RolloverEnergyFraction = Some 1.5
                CollapseWake =
                    Some
                        {
                            SceneId = "nowhere"
                            X = 1.0
                            Y = 1.0
                        }
            }
            project

    let found = codes "rules." broken |> List.map (fun p -> p.Code, p.Path)
    Assert.Contains(("rules.unknownWord", "settings.rules.sleep"), found)
    Assert.Contains(("rules.unknownWord", "settings.rules.dayEnd"), found)
    Assert.Contains(("rules.rolloverEnergy", "settings.rules.rolloverEnergyFraction"), found)
    Assert.Contains(("rules.unknownWakeScene", "settings.rules.collapseWake.sceneId"), found)

    // A wake spot off the map, one that nothing uses, and a day's end without a day.
    let spot x =
        Some
            {
                SceneId = project.StartSceneId
                X = x
                Y = 1.0
            }

    let outside =
        withRules
            { GameRules.Default with
                CollapseWake = spot 9999.0
            }
            project

    Assert.Contains(codes "rules." outside, fun p -> p.Code = "rules.wakeOutOfBounds" && p.Severity = Severity.Error)

    let unused =
        withRules
            { GameRules.Default with
                Sleep = Some SleepPolicies.None
                DayEnd = Some DayEndRules.Collapse
                CollapseWake = spot 1.0
            }
            { project with
                Settings =
                    { project.Settings with
                        EnergyEnabled = false
                    }
            }

    let found = codes "rules." unused
    Assert.Contains(found, fun p -> p.Code = "rules.dayEndWithoutDay" && p.Severity = Severity.Warning)
    Assert.Contains(found, fun p -> p.Code = "rules.wakeWithoutCollapse" && p.Severity = Severity.Info)

    // The classic rules do collapse the player: a wake spot on the map is fine.
    Assert.Empty(
        codes
            "rules."
            (withRules
                { GameRules.Default with
                    CollapseWake = spot 1.0
                }
                project)
    )

[<Fact>]
let ``the rules reach the cartridge and round-trip through the project file`` () =
    let project = withRules workshop (starter ())

    match Decode.run SchemaJson.decodeGameProject (SchemaJson.encodeGameProject project) with
    | Ok loaded -> Assert.Equal(project, loaded)
    | Error message -> failwith message

    let content = ContentCompiler.compile project
    Assert.Equal(Some workshop, content.Settings.Rules)

    let json =
        SchemaJson.encodeGameContent content |> Json.get "settings" |> Json.get "rules"

    Assert.Equal(JString "none", Json.get "sleep" json)
    Assert.Equal(JBool false, Json.get "farming" json)
    // A classic project writes no rules at all.
    Assert.False(Json.has "rules" (Json.get "settings" (SchemaJson.encodeGameProject (starter ()))))
    // The settings form's edit is one undo step, and nothing changed is an empty batch.
    Assert.Equal(project, apply (SettingsForm.Rules(starter (), workshop)) (starter ()))
    Assert.Equal(Batch("Rules", []), SettingsForm.Rules(project, workshop))
    Assert.Equal(None, (apply (SettingsForm.Rules(project, null)) project).Settings.Rules)

// ── Properties ───────────────────────────────────────────────────────────────

[<Fact>]
let ``properties compile into the content and are known variables`` () =
    let project = starter () |> withProperties [ pressure; oxygen ]
    Assert.Empty(codes "property." project)
    // The oxygen rate's `variable pressure` condition names a property: not an unknown variable.
    Assert.DoesNotContain(Problems.collect project, fun p -> p.Code.EndsWith "conditionUnknownVariable")
    let content = ContentCompiler.compile project
    Assert.Equal(Some [ pressure; oxygen ], content.Properties)
    // A project that uses properties needs the version that knows them.
    Assert.True(Json.has "properties" (SchemaJson.encodeGameProject project))

    match Decode.run SchemaJson.decodeGameProject (SchemaJson.encodeGameProject project) with
    | Ok loaded -> Assert.Equal(project, loaded)
    | Error message -> failwith message
    // Removing them all removes the key.
    let none = apply (SetProperties []) project
    Assert.Equal(None, none.Properties)
    Assert.Equal(None, (ContentCompiler.compile none).Properties)
    // The settings form: nothing changed is an empty batch, and a new property gets a free id.
    Assert.Equal(Batch("Properties", []), SettingsForm.Properties(project, [ pressure; oxygen ]))
    Assert.Equal("property-1", SettingsForm.NewProperty(project).Id)

    let taken = project |> withProperties [ { oxygen with Id = "property-1" } ]

    Assert.Equal("property-2", SettingsForm.NewProperty(taken).Id)

[<Fact>]
let ``property mistakes are reported where they are`` () =
    let project = starter ()

    let found (properties: PropertyDefinition list) =
        codes "property." (project |> withProperties properties)
        |> List.map (fun p -> p.Code, p.Severity)

    Assert.Contains(("property.noId", Severity.Error), found [ { oxygen with Id = "" } ])
    Assert.Contains(("property.id", Severity.Error), found [ { oxygen with Id = "o2@lab" } ])
    Assert.Contains(("property.duplicateId", Severity.Error), found [ oxygen; oxygen ])
    Assert.Contains(("property.scope", Severity.Error), found [ { oxygen with Scope = "planet" } ])
    Assert.Contains(("property.hud", Severity.Error), found [ { oxygen with Hud = Some "dial" } ])
    Assert.Contains(("property.bounds", Severity.Error), found [ { oxygen with Min = 100.0; Max = 0.0 } ])
    Assert.Contains(("property.initial", Severity.Warning), found [ { oxygen with Initial = 140.0 } ])

    Assert.Contains(
        ("property.unknownScene", Severity.Error),
        found
            [
                { pressure with
                    Scenes = Some [ "nowhere" ]
                }
            ]
    )

    Assert.Contains(
        ("property.unknownMachine", Severity.Error),
        found
            [
                { oxygen with
                    Scope = PropertyScopes.Object
                    Objects = Some [ "no-machine" ]
                    Hud = None
                    Rates = []
                }
            ]
    )

    Assert.Contains(
        ("property.scenesUnused", Severity.Info),
        found
            [
                { oxygen with
                    Scenes = Some [ project.StartSceneId ]
                }
            ]
    )

    let threshold compare value =
        { PropertyThreshold.Default with
            Compare = compare
            Value = value
            Message = Some "!"
        }

    Assert.Contains(
        ("property.compare", Severity.Error),
        found
            [
                { oxygen with
                    Thresholds = [ threshold "<" 5.0 ]
                }
            ]
    )
    // 0 to 100 is never above 100, and always at least 0.
    Assert.Contains(
        ("property.thresholdNeverCrossed", Severity.Warning),
        found
            [
                { oxygen with
                    Thresholds = [ threshold "gt" 100.0 ]
                }
            ]
    )

    Assert.Contains(
        ("property.thresholdNeverCrossed", Severity.Warning),
        found
            [
                { oxygen with
                    Thresholds = [ threshold "ge" 0.0 ]
                }
            ]
    )

    Assert.DoesNotContain(
        ("property.thresholdNeverCrossed", Severity.Warning),
        found
            [
                { oxygen with
                    Thresholds = [ threshold "le" 0.0 ]
                }
            ]
    )

    Assert.Contains(
        ("property.rateZero", Severity.Warning),
        found
            [
                { oxygen with
                    Rates = [ { PerHour = 0.0; Conditions = [] } ]
                }
            ]
    )
    // A threshold's outcomes are checked like any others.
    let badOutcome =
        { oxygen with
            Thresholds =
                [
                    { threshold "le" 0.0 with
                        Outcomes =
                            [
                                { EventOutcome.Default with
                                    Type = EventOutcomeTypes.GiveItem
                                    ItemId = Some "no-such-item"
                                }
                            ]
                    }
                ]
        }

    Assert.Contains(
        codes "property." (project |> withProperties [ badOutcome ]),
        fun p -> p.Severity = Severity.Error && p.Path.StartsWith "properties[0].thresholds[0]"
    )

[<Fact>]
let ``scenes start their properties and crops need them in a range`` () =
    let project = starter () |> withProperties [ pressure; oxygen ]
    let sceneId = project.StartSceneId
    let started = apply (SetSceneProperties(sceneId, [ "pressure", 40.0 ])) project
    Assert.Equal(Some [ "pressure", 40.0 ], (scene started sceneId).Properties)
    Assert.Empty(codes "property." started)
    // The start values reach the cartridge's scenes, and an empty list clears them.
    let content = ContentCompiler.compile started
    Assert.Equal(Some [ "pressure", 40.0 ], (content.Scenes |> List.find (fun s -> s.Id = sceneId)).Properties)
    Assert.Equal(None, (scene (apply (SetSceneProperties(sceneId, [])) started) sceneId).Properties)
    // The Surroundings field's text.
    Assert.Equal("pressure 40", SettingsForm.ScenePropertiesText(scene started sceneId))
    Assert.Equal<struct (string * float) seq>([ struct ("pressure", 40.0); struct ("power", 2.5) ], SettingsForm.SceneProperties "pressure 40, power=2.5")
    Assert.Empty(SettingsForm.SceneProperties "")

    Assert.Throws<System.FormatException>(fun () -> SettingsForm.SceneProperties "pressure lots" |> ignore)
    |> ignore

    Assert.Throws<System.FormatException>(fun () -> SettingsForm.SceneProperties "pressure" |> ignore)
    |> ignore

    let start values =
        codes "property." (apply (SetSceneProperties(sceneId, values)) project)
        |> List.map (fun p -> p.Code)

    Assert.Contains("property.unknownInScene", start [ "gravity", 1.0 ])
    Assert.Contains("property.notOfScenes", start [ "oxygen", 50.0 ])
    Assert.Contains("property.sceneInitial", start [ "pressure", 500.0 ])

    // A crop that needs pressure.
    let crop (range: PropertyRange) =
        { CustomCropDefinition.Default with
            Id = "moss"
            Name = "Moss"
            GrowthDays = Some 3.0
            Stages = 3.0
            Seasons = [ "spring" ]
            Requirements =
                Some
                    { GrowthRequirements.Default with
                        Properties = Some [ range ]
                    }
        }

    let withCrop range =
        codes
            "property."
            { project with
                CustomCrops = Some [ crop range ]
            }
        |> List.map (fun p -> p.Code)

    Assert.Empty(
        withCrop
            {
                Property = "pressure"
                Min = Some 60.0
                Max = None
            }
    )

    Assert.Contains(
        "property.unknownInCrop",
        withCrop
            {
                Property = "gravity"
                Min = None
                Max = None
            }
    )

    Assert.Contains(
        "property.cropRange",
        withCrop
            {
                Property = "pressure"
                Min = Some 80.0
                Max = Some 20.0
            }
    )
