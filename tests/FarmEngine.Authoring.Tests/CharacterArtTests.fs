module FarmEngine.Authoring.Tests.CharacterArtTests

// Character art sizes, looks, layers and action clips (#215, #216, #234, #235): the schema keys,
// the Problems checks and what the cartridge carries.

open Xunit
open FarmEngine.Authoring
open FarmEngine.Authoring.Tests.TestProjects
open FarmEngine.Schemas

let private codes (project: GameProject) =
    Problems.collect project |> List.map (fun p -> p.Code, p.Path)

let private parse (text: string) =
    match Json.parse text with
    | Ok json -> json
    | Error message -> failwith message

let private asset id =
    { CustomAsset.Default with
        Id = id
        Name = id
        Type = CustomAssetTypes.Art
        DataUrl = "data:image/png;base64,"
        Width = Some 32.0
        Height = Some 48.0
    }

[<Fact>]
let ``new character art keys round-trip and default to absent`` () =
    let npc =
        { Npc.Default with
            Id = "n"
            Look =
                Some
                    { PlayerLook.Default with
                        Shirt = Some "#ff0000"
                    }
            Layers =
                Some
                    [
                        { CharacterLayer.Default with
                            AssetId = "body"
                            Color = Some "#808080"
                        }
                    ]
        }

    let json = SchemaJson.encodeNpc npc |> Json.stringify
    Assert.Contains("\"look\":{\"shirt\":\"#ff0000\"}", json)
    Assert.Contains("\"layers\":[{\"assetId\":\"body\",\"color\":\"#808080\"}]", json)
    Assert.Equal(npc, SchemaJson.decodeNpc [] (parse json))
    let plain = SchemaJson.encodeNpc Npc.Default |> Json.stringify
    Assert.DoesNotContain("look", plain)
    Assert.DoesNotContain("layers", plain)

    let clip =
        { AnimationClip.Default with
            Name = "use-till-down"
            LockMovement = Some true
            ImpactFrame = Some 2.0
            Effect = Some ClipEffects.Squash
            HoldItem = Some false
        }

    let clipJson = SchemaJson.encodeAnimationClip clip |> Json.stringify
    Assert.Contains("\"impactFrame\":2", clipJson)
    Assert.Equal(clip, SchemaJson.decodeAnimationClip [] (parse clipJson))

[<Fact>]
let ``layers, looks and action clips are checked`` () =
    let project = starter ()
    let npc = project.Npcs.Head

    let broken =
        { project with
            CustomAssets =
                project.CustomAssets
                @ [
                    { asset "body" with
                        PixelsPerTile = Some 64.0
                        Animations =
                            Some
                                [
                                    { AnimationClip.Default with
                                        Name = "use-till"
                                        Frames =
                                            [
                                                { ArtFrame.Default with
                                                    Width = 32.0
                                                    Height = 48.0
                                                    Ticks = 4.0
                                                }
                                            ]
                                        ImpactFrame = Some 3.0
                                        Effect = Some "wobble"
                                    }
                                ]
                    }
                ]
            PlayerLayers =
                Some
                    [
                        { CharacterLayer.Default with
                            AssetId = "ghost"
                        }
                    ]
            Npcs =
                { npc with
                    Look =
                        Some
                            { PlayerLook.Default with
                                Hair = Some "blue"
                            }
                    Layers =
                        Some
                            [
                                { CharacterLayer.Default with
                                    AssetId = "body"
                                    Color = Some "#12345"
                                }
                            ]
                }
                :: project.Npcs.Tail
        }

    let found = codes broken

    let expect code path =
        Assert.True(List.contains (code, path) found, sprintf "%s at %s in %A" code path found)

    expect "graphics.missingAsset" "playerLayers[0].assetId"
    expect "graphics.badColor" "npcs[0].look.hair"
    expect "graphics.badColor" "npcs[0].layers[0].color"
    let assetIndex = List.length project.CustomAssets
    expect "graphics.impactOutside" (sprintf "customAssets[%d].animations[0].impactFrame" assetIndex)
    expect "graphics.unknownEffect" (sprintf "customAssets[%d].animations[0].effect" assetIndex)
    // A layer of known art with a good colour is fine.
    Assert.DoesNotContain(("graphics.missingAsset", "npcs[0].layers[0].assetId"), found)

[<Fact>]
let ``an animate step needs its actor and clip`` () =
    let project = starter ()

    let cutscene =
        { Cutscene.Default with
            Id = "show"
            Name = "Show"
            Steps =
                [
                    { CutsceneStep.Default with
                        Type = CutsceneStepTypes.Animate
                        Actor = Some "player"
                    }
                ]
        }

    let found =
        codes
            { project with
                Cutscenes = Some [ cutscene ]
            }

    Assert.True(found |> List.exists (fun (_, path) -> path.EndsWith "steps[0].animation"), sprintf "%A" found)
    Assert.Contains("Animation", Vocabulary.stepFields CutsceneStepTypes.Animate)

[<Fact>]
let ``the cartridge presentation carries the player's layers`` () =
    let project =
        { starter () with
            CustomAssets = (starter ()).CustomAssets @ [ asset "body" ]
            PlayerLayers =
                Some
                    [
                        { CharacterLayer.Default with
                            AssetId = "body"
                        }
                    ]
        }

    let presentation = CartridgeCompiler.PresentationSection project |> Json.stringify
    Assert.Contains("\"playerLayers\":[{\"assetId\":\"body\"}]", presentation)
    Assert.Contains("\"id\":\"body\"", presentation)
    Assert.Contains("sailor", CharacterArt.builtinAppearances)
