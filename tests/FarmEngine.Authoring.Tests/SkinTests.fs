/// The interface skin (#236): its checks, its fonts, and what the cartridge carries.
module FarmEngine.Authoring.Tests.SkinTests

open System
open System.IO
open Xunit
open FarmEngine.Authoring
open FarmEngine.Authoring.Tests.TestProjects
open FarmEngine.Schemas

let private fontFile (name: string) =
    File.ReadAllBytes(Path.Combine(__SOURCE_DIRECTORY__, "..", "..", "assets", "fonts", name))

let private fontAsset (id: string) (file: string) =
    { CustomAsset.Default with
        Id = id
        Name = file
        Type = CustomAssetTypes.Font
        DataUrl = "data:font/ttf;base64," + Convert.ToBase64String(fontFile file)
    }

let private art (id: string) =
    { CustomAsset.Default with
        Id = id
        Name = id
        Type = CustomAssetTypes.Art
        DataUrl = "data:image/png;base64,AAAA"
        Width = Some 12.0
        Height = Some 12.0
    }

let private slice (id: string) =
    { UiNineSlice.Default with
        Visual = { VisualRef.Default with AssetId = id }
        Left = 3.0
        Top = 3.0
        Right = 3.0
        Bottom = 3.0
    }

let private skinned (skin: UiSkin) =
    let project = starter ()

    { project with
        CustomAssets = project.CustomAssets @ [ fontAsset "font-body" "AtkinsonHyperlegible-Regular.ttf"; art "panel-art" ]
    }
    |> apply (SetUiSkin(Some skin))

let private skinProblems (project: GameProject) =
    Problems.collect project |> List.filter (fun p -> p.Code.StartsWith("skin.", StringComparison.Ordinal))

[<Fact>]
let ``font files are read for their characters`` () =
    match FontFile.coverage (fontFile "Inter-Regular.ttf") with
    | Ok ranges ->
        Assert.Equal(None, FontFile.firstMissing ranges "Harvest day: 12 coins")
        Assert.True((FontFile.firstMissing ranges "收获").IsSome)
    | Error why -> failwith why

    Assert.True(Result.isError (FontFile.coverage [| 1uy; 2uy; 3uy |]))
    Assert.True(Result.isError (FontFile.coverage (Text.Encoding.ASCII.GetBytes "ttcf0000000000000000")))
    // Cut short anywhere: an error or a smaller map, never an exception.
    let bytes = fontFile "AtkinsonHyperlegible-Bold.ttf"

    for length in [ 12; 100; 400; 4000 ] do
        FontFile.coverage (Array.sub bytes 0 length) |> ignore

[<Fact>]
let ``a complete skin has no problems and ships in the presentation`` () =
    let skin =
        { UiSkin.Default with
            BodyFont = Some "font-body"
            Colors = Some [ "text", "#102030"; "warning", "#ff000080" ]
            Panels = Some [ "panel", slice "panel-art"; "button", slice "panel-art" ]
            Icons = Some [ "money", { VisualRef.Default with AssetId = "panel-art" } ]
            Layout =
                Some
                    { UiLayout.Default with
                        HudPosition = Some UiHudPositions.Bottom
                        ClockStyle = Some UiClockStyles.TwentyFourHour
                        PortraitSize = Some 64.0
                    }
            Title =
                Some
                    { UiTitleScreen.Default with
                        Scene = Some "scene-farm"
                        MenuPosition = Some UiMenuPositions.Left
                    }
        }

    let project = skinned skin
    Assert.Empty(skinProblems project)
    let presentation = CartridgeCompiler.PresentationSection project
    Assert.Equal(Some(SchemaJson.encodeUiSkin skin), Json.tryGet "uiSkin" presentation)
    // The skin's art and font ship with the game.
    let used = AssetUsage.used project |> List.map (fun a -> a.Id)
    Assert.Contains("panel-art", used)
    Assert.Contains("font-body", used)
    // Reset brings the built-in look back.
    let reset = project |> apply (SetUiSkin None)
    Assert.Equal(None, reset.UiSkin)
    Assert.Equal(None, Json.tryGet "uiSkin" (CartridgeCompiler.PresentationSection reset))

[<Fact>]
let ``problems name the skin parts that would keep the built-in look`` () =
    let skin =
        { UiSkin.Default with
            HeadingFont = Some "panel-art"
            NumberFont = Some "missing"
            Colors = Some [ "text", "blue-ish"; "sparkle", "#ffffff" ]
            Panels = Some [ "panel", { slice "nothing" with Scale = Some 12.0 }; "lid", slice "panel-art"; "slot", { slice "panel-art" with Left = 10.0 } ]
            Icons = Some [ "rocket", { VisualRef.Default with AssetId = "panel-art" } ]
            Layout =
                Some
                    { UiLayout.Default with
                        ClockStyle = Some "sundial"
                        PortraitSize = Some 500.0
                    }
            Title = Some { UiTitleScreen.Default with Scene = Some "nowhere" }
        }

    let codes =
        skinProblems (skinned skin) |> List.map (fun p -> p.Code, p.Path) |> List.sort

    let expected =
        [
            "skin.badColor", "uiSkin.colors.text"
            "skin.badLayout", "uiSkin.layout.clockStyle"
            "skin.badLayout", "uiSkin.layout.portraitSize"
            "skin.badScale", "uiSkin.panels.panel"
            "skin.missingArt", "uiSkin.panels.panel"
            "skin.missingFont", "uiSkin.numberFont"
            "skin.notFont", "uiSkin.headingFont"
            "skin.sliceEdges", "uiSkin.panels.slot"
            "skin.titleScene", "uiSkin.title.scene"
            "skin.unknownColor", "uiSkin.colors.sparkle"
            "skin.unknownIcon", "uiSkin.icons.rocket"
            "skin.unknownPart", "uiSkin.panels.lid"
        ]
        |> List.sort

    Assert.Equal<(string * string) list>(expected, codes)
    Assert.True(skinProblems (skinned skin) |> List.forall (fun p -> p.Target = Some NavigationTarget.Interface))

[<Fact>]
let ``the font coverage check reads the skin's font`` () =
    let project =
        skinned { UiSkin.Default with BodyFont = Some "font-body" }
        |> apply (UpsertItem(item "item-tea" "Matcha 抹茶"))

    let coverage =
        Problems.collect project |> List.filter (fun p -> p.Code = "text.fontCoverage")

    Assert.NotEmpty coverage
    Assert.Contains("AtkinsonHyperlegible-Regular.ttf", coverage.Head.Message)

[<Fact>]
let ``import records do not keep unused art in the game`` () =
    let project = starter ()

    let project =
        { project with
            CustomAssets = project.CustomAssets @ [ art "loose" ]
            Imports =
                Some
                    [
                        { ImportSource.Default with
                            Kind = ImportKinds.Aseprite
                            TargetId = "loose"
                            Path = "/a/loose.aseprite"
                        }
                    ]
        }

    Assert.Contains("loose", AssetUsage.unused project |> List.map (fun a -> a.Id))
