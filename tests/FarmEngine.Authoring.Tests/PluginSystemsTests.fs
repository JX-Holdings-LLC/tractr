/// The plugin systems on the authoring side (#237–#241): granted hooks, the install review, the
/// Problems checks of plugin conditions and outcomes, project scripts in folders and carts.
module FarmEngine.Authoring.Tests.PluginSystemsTests

open System.Collections.Generic
open Xunit
open FarmEngine.Authoring
open FarmEngine.Authoring.Tests.TestProjects
open FarmEngine.Schemas

let private pack (enabled: bool) : PackInstallation =
    let plugin =
        { PackPlugin.Default with
            Id = "logic"
            Hooks = [ "onDayStart"; "onAction" ]
            Source = "api.condition('rich', (a, s) => s.player().money >= a.min);"
            Conditions =
                Some
                    [
                        { PluginFunction.Default with
                            Name = "rich"
                            Label = Some "Is rich"
                            Args =
                                [
                                    { PluginArgument.Default with
                                        Name = "min"
                                        Type = PluginArgumentTypes.Integer
                                    }
                                    { PluginArgument.Default with
                                        Name = "item"
                                        Type = PluginArgumentTypes.Id
                                        Kind = Some "item"
                                        Optional = true
                                    }
                                ]
                        }
                    ]
            Outcomes =
                Some
                    [
                        { PluginFunction.Default with
                            Name = "tip"
                        }
                    ]
            Entities = Some [ "lamp"; "not-allowed" ]
            ItemUses = Some [ "wand" ]
            Screens =
                Some
                    [
                        { PluginScreen.Default with
                            Name = "ledger"
                        }
                    ]
        }

    {
        Enabled = enabled
        Pack =
            { ContentPack.Default with
                Manifest =
                    { PackManifest.Default with
                        Id = "pets"
                        Name = "Pets"
                        Version = "1.0.0"
                        Permissions =
                            { PackPermissions.Default with
                                Hooks = [ "onDayStart" ]
                                Mutations = Some [ "message"; "startTimer" ]
                                Read = Some [ "player"; "people" ]
                                Entities = Some [ "lamp" ]
                                ItemUses = Some [ "wand" ]
                                Screens = Some [ "ledger" ]
                            }
                    }
                Plugins = [ plugin ]
            }
    }

[<Fact>]
let ``a plugin hears its granted hooks and the hooks of its systems`` () =
    let install = pack true

    Assert.Equal<string list>(
        [
            "onDayStart"
            "condition:pets:rich"
            "outcome:pets:tip"
            "entity:pets:lamp"
            "use:pets:wand"
            "screen:pets:ledger"
        ],
        PluginExtensions.grantedHooks install.Pack install.Pack.Plugins.[0]
    )

[<Fact>]
let ``the install review names reads and systems`` () =
    let install = pack true

    let reads =
        PluginExtensions.describeReads install.Pack.Manifest.Permissions
        |> Option.defaultValue ""

    Assert.Contains("the player", reads)
    Assert.Contains("people and story", reads)

    let systems =
        PluginExtensions.describeSystems install.Pack |> Option.defaultValue ""

    Assert.Contains("entity types lamp", systems)
    Assert.Contains("conditions rich", systems)
    Assert.Contains("timers", systems)
    Assert.Equal(None, PluginExtensions.describeReads PackPermissions.Default)

let private withCondition (install: PackInstallation) (condition: EventCondition) : GameProject =
    let project = starter ()

    { project with
        ContentPacks = [ install ]
        Actions =
            [
                { ActionDef.Default with
                    Id = "check"
                    Name = "Check"
                    Conditions = [ condition ]
                    Outcomes =
                        [
                            { EventOutcome.Default with
                                Type = EventOutcomeTypes.Message
                                Message = Some "hi"
                            }
                        ]
                }
            ]
    }

let private codes (project: GameProject) =
    Problems.collect project |> List.map (fun problem -> problem.Code)

[<Fact>]
let ``plugin conditions are checked like built-ins`` () =
    let condition args =
        EventCondition.Plugin
            {
                Condition = "pets:rich"
                Args = Some args
            }

    let good = withCondition (pack true) (condition (JObject [ "min", JNumber 5.0 ]))
    Assert.DoesNotContain("action.conditionPluginArgType", codes good)
    Assert.DoesNotContain("action.conditionPluginArgMissing", codes good)

    let wrong =
        withCondition (pack true) (condition (JObject [ "min", JString "five"; "item", JString "nothing" ]))

    Assert.Contains("action.conditionPluginArgType", codes wrong)
    Assert.Contains("action.conditionPluginArgUnknown", codes wrong)
    let missing = withCondition (pack true) (condition (JObject []))
    Assert.Contains("action.conditionPluginArgMissing", codes missing)
    let off = withCondition (pack false) (condition (JObject [ "min", JNumber 5.0 ]))
    Assert.Contains("action.conditionPluginPackOff", codes off)

    let unknown =
        withCondition
            (pack true)
            (EventCondition.Plugin
                {
                    Condition = "gone:thing"
                    Args = None
                })

    Assert.Contains("action.conditionPluginUnknown", codes unknown)

[<Fact>]
let ``plugin outcomes are checked too`` () =
    let project =
        withCondition (pack true) (EventCondition.Flag { Flag = ""; Value = true })

    let outcome name =
        { EventOutcome.Default with
            Type = EventOutcomeTypes.Plugin
            Extra = [ "pluginOutcome", JString name ]
        }

    let uses name =
        { project with
            Actions =
                [
                    { project.Actions.[0] with
                        Outcomes = [ outcome name ]
                    }
                ]
        }

    Assert.DoesNotContain("action.outcomePluginUnknown", codes (uses "pets:tip"))
    Assert.Contains("action.outcomePluginUnknown", codes (uses "pets:nope"))

[<Fact>]
let ``the new pack and project fields round-trip`` () =
    let project =
        { (starter ()) with
            ContentPacks = [ pack true ]
            Scripts =
                Some
                    [
                        { ProjectScript.Default with
                            Id = "economy"
                            Source = "api.on('onDayStart', () => []);"
                        }
                    ]
            Settings =
                { (starter ()).Settings with
                    Scripts =
                        Some
                            { ScriptSettings.Default with
                                FuelPerCall = Some 100000000.0
                            }
                }
        }

    let json = ProjectLoad.toJson project

    match ProjectLoad.loadProject json with
    | { Data = Some loaded } -> Assert.Equal(project, loaded)
    | result -> failwithf "%A" result.Errors

[<Fact>]
let ``project scripts get their own files in a project folder`` () =
    let project =
        { (starter ()) with
            Scripts =
                Some
                    [
                        { ProjectScript.Default with
                            Id = "economy"
                            Source = "api.on('onDayStart', () => []);\n"
                        }
                    ]
        }

    let files = ProjectFolder.toFiles project

    let script =
        files |> List.find (fun (path, _) -> path = "scripts/economy.js") |> snd

    Assert.Equal("api.on('onDayStart', () => []);\n", System.Text.Encoding.UTF8.GetString script)

    let folder =
        Dictionary<string, byte[]>(dict files) :> IReadOnlyDictionary<string, byte[]>

    match ProjectFolder.load folder with
    | { Data = Some loaded } -> Assert.Equal(project.Scripts, loaded.Scripts)
    | result -> failwithf "%A" result.Errors

[<Fact>]
let ``carts carry project scripts and read permissions`` () =
    let project =
        { (starter ()) with
            ContentPacks = [ pack true ]
            Scripts =
                Some
                    [
                        { ProjectScript.Default with
                            Id = "economy"
                            Source = "api.on('onDayStart', () => []);"
                        }
                    ]
        }

    let bytes = CartridgeCompiler.Compile project

    let cart =
        CartridgeReader.read bytes |> Result.defaultWith (fun error -> failwith error)

    Assert.Equal("script/economy", cart.Plugins.[0].Id)
    Assert.Equal("", cart.Plugins.[0].PackId)
    Assert.Equal<string list>([ "player"; "people" ], cart.Plugins.[1].GrantedReads)
    Assert.Contains("entity:pets:lamp", cart.Plugins.[1].GrantedHooks)
