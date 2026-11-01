/// Aseprite sprite sheets (#232) and Tiled / LDtk maps (#233), from the fixtures in
/// `fixtures/import`.
module FarmEngine.Authoring.Tests.ImportTests

open System
open System.IO
open Xunit
open FarmEngine.Authoring
open FarmEngine.Authoring.Tests.TestProjects
open FarmEngine.Schemas

let private fixture (parts: string list) =
    Path.GetFullPath(Path.Combine(Array.ofList (__SOURCE_DIRECTORY__ :: ".." :: ".." :: "fixtures" :: "import" :: parts)))

let private ok (result: Result<'T, string>) : 'T =
    match result with
    | Ok value -> value
    | Error message -> failwith message

/// Asserts a `Result` is an error whose message contains `expected`.
let private refused (expected: string) (result: Result<'T, string>) =
    match result with
    | Error message -> Assert.Contains(expected, message)
    | Ok _ -> failwithf "expected an error with %A" expected

let private clip (asset: CustomAsset) (name: string) =
    defaultArg asset.Animations [] |> List.find (fun c -> c.Name = name)

let private clipNames (asset: CustomAsset) =
    defaultArg asset.Animations [] |> List.map (fun c -> c.Name)

let private asset (project: GameProject) (id: string) =
    project.CustomAssets |> List.find (fun a -> a.Id = id)

// ── Aseprite ─────────────────────────────────────────────────────────

[<Theory>]
[<InlineData("walk_down", "walk-down")>]
[<InlineData("Walk Down", "walk-down")>]
[<InlineData("walkDown", "walk-down")>]
[<InlineData("idle", "idle")>]
[<InlineData("  ", "clip")>]
[<InlineData("Run.Left-2", "run-left-2")>]
let ``tag names become Tractr clip names`` (tag: string, expected: string) =
    Assert.Equal(expected, AsepriteImport.clipName tag)

[<Fact>]
let ``frame durations become ticks at 20 a second`` () =
    Assert.Equal(2.0, AsepriteImport.ticks 100)
    Assert.Equal(5.0, AsepriteImport.ticks 250)
    Assert.Equal(1.0, AsepriteImport.ticks 10)
    Assert.Equal(1.0, AsepriteImport.ticks 0)

[<Fact>]
let ``tag directions order the frames`` () =
    let tag direction =
        {
            Name = "t"
            From = 1
            To = 4
            Direction = direction
            Repeat = 0
        }

    Assert.Equal<int list>([ 1; 2; 3; 4 ], AsepriteImport.tagFrames 10 (tag "forward"))
    Assert.Equal<int list>([ 4; 3; 2; 1 ], AsepriteImport.tagFrames 10 (tag "reverse"))
    Assert.Equal<int list>([ 1; 2; 3; 4; 3; 2 ], AsepriteImport.tagFrames 10 (tag "pingpong"))
    Assert.Equal<int list>([ 4; 3; 2; 1; 2; 3 ], AsepriteImport.tagFrames 10 (tag "pingpong_reverse"))
    Assert.Equal<int list>([ 1; 2 ], AsepriteImport.tagFrames 3 (tag "forward"))

[<Fact>]
let ``the array and hash JSON exports read the same way`` () =
    let array =
        AsepriteImport.parseJson (File.ReadAllText(fixture [ "aseprite"; "hero-array.json" ]))
        |> ok

    Assert.Equal(Some "hero.png", array.Image)
    Assert.Equal(4, array.Frames.Length)
    Assert.Equal<string list>([ "walk_down"; "idleUp" ], array.Tags |> List.map (fun t -> t.Name))
    Assert.Equal(250, array.Frames.[2].DurationMs)
    Assert.Equal(1, array.Slices.Length)
    Assert.Equal((4, 2, 8, 8), (array.Slices.Head.X, array.Slices.Head.Y, array.Slices.Head.Width, array.Slices.Head.Height))

    let hash =
        AsepriteImport.parseJson (File.ReadAllText(fixture [ "aseprite"; "hero-hash.json" ]))
        |> ok

    Assert.Equal<int list>([ 0; 16; 32 ], hash.Frames |> List.map (fun f -> f.X))
    Assert.Equal("reverse", hash.Tags.Head.Direction)

[<Fact>]
let ``JSON that is not a sheet is refused`` () =
    refused "no frames" (AsepriteImport.parseJson "{\"meta\":{}}")
    refused "could not be read" (AsepriteImport.parseJson "{")

let private request (sheet: SpriteSheetSource) =
    {
        Path = "/art/hero.json"
        Name = "Hero"
        AssetType = CustomAssetTypes.Npc
        Sheet = sheet
        DataUrl = "data:image/png;base64,AAAA"
        Width = sheet.SheetWidth
        Height = sheet.SheetHeight
        Names = []
        TargetId = None
    }

[<Fact>]
let ``an import makes an asset whose tags are clips and whose slices are frames`` () =
    let sheet =
        AsepriteImport.parseJson (File.ReadAllText(fixture [ "aseprite"; "hero-array.json" ]))
        |> ok

    let result = AsepriteImport.import (starter ()) (request sheet)
    Assert.True result.Created
    let made = asset result.Project result.AssetId
    Assert.Equal(CustomAssetTypes.Npc, made.Type)
    Assert.Equal<string list>([ "walk-down"; "idle-up"; "portrait" ], clipNames made)
    let walk = clip made "walk-down"
    Assert.True walk.Loop
    Assert.Equal<float list>([ 0.0; 16.0 ], walk.Frames |> List.map (fun f -> f.X))
    Assert.Equal<float list>([ 2.0; 2.0 ], walk.Frames |> List.map (fun f -> f.Ticks))
    // pingpong over two frames: there and back without repeating the ends.
    Assert.Equal<float list>([ 32.0; 48.0 ], (clip made "idle-up").Frames |> List.map (fun f -> f.X))
    let portrait = clip made "portrait"
    Assert.False portrait.Loop
    Assert.Equal((4.0, 2.0, 8.0, 8.0), (portrait.Frames.Head.X, portrait.Frames.Head.Y, portrait.Frames.Head.Width, portrait.Frames.Head.Height))
    let source = result.Project.Imports.Value |> List.exactlyOne
    Assert.Equal(ImportKinds.Aseprite, source.Kind)
    Assert.Equal(result.AssetId, source.TargetId)
    Assert.Equal<(string * string) list>([ "walk_down", "walk-down"; "idleUp", "idle-up"; "slice:portrait", "portrait" ], source.Names.Value)
    // The import record stays out of the game.
    Assert.DoesNotContain("imports", Json.stringify (CartridgeCompiler.PresentationSection result.Project))

[<Fact>]
let ``re-import updates the clips in place and keeps every assignment`` () =
    let sheet =
        AsepriteImport.parseJson (File.ReadAllText(fixture [ "aseprite"; "hero-array.json" ]))
        |> ok
    // The creator renames a clip in the preview, then assigns the art and adds a clip of their own.
    let first =
        AsepriteImport.import
            (starter ())
            { request sheet with
                Names = [ "idleUp", "idle" ]
            }

    let id = first.AssetId

    let edited =
        first.Project
        |> apply (
            UpsertNpc
                { npc first.Project "npc-farmer" with
                    Visual =
                        Some
                            { VisualRef.Default with
                                AssetId = id
                                Animation = Some "idle"
                            }
                }
        )

    let edited =
        let a = asset edited id

        let clips =
            (defaultArg a.Animations [])
            |> List.map (fun c -> if c.Name = "walk-down" then { c with Loop = false } else c)

        edited
        |> apply (
            UpsertAsset
                { a with
                    Animations =
                        Some(
                            clips
                            @ [
                                { AnimationClip.Default with
                                    Name = "wave"
                                    Frames =
                                        [
                                            { ArtFrame.Default with
                                                Width = 16.0
                                                Height = 16.0
                                            }
                                        ]
                                }
                            ]
                        )
                }
        )

    // The file changes: walk_down gets a third frame, idleUp is gone, run_left is new.
    let changed =
        { sheet with
            Frames =
                sheet.Frames
                @ [
                    { sheet.Frames.[0] with
                        X = 64
                        DurationMs = 50
                    }
                ]
            SheetWidth = 80
            Tags =
                [
                    { sheet.Tags.[0] with To = 4 }
                    {
                        Name = "run_left"
                        From = 0
                        To = 0
                        Direction = "forward"
                        Repeat = 0
                    }
                ]
        }

    let second = AsepriteImport.import edited (request changed)
    Assert.False second.Created
    Assert.Equal(id, second.AssetId)
    Assert.Equal(1, second.Project.CustomAssets |> List.filter (fun a -> a.Id = id) |> List.length)
    let updated = asset second.Project id
    Assert.Equal<string list>([ "walk-down"; "portrait"; "wave"; "run-left" ], clipNames updated)
    let walk = clip updated "walk-down"
    Assert.Equal(5, walk.Frames.Length)
    Assert.False walk.Loop
    Assert.Equal<string list>([ "idle" ], second.Removed)
    Assert.Equal<string list>([ "run-left" ], second.Added)
    Assert.Equal(Some id, (npc second.Project "npc-farmer").Visual |> Option.map (fun v -> v.AssetId))
    Assert.Equal(80.0, updated.Width.Value)
    Assert.Equal(1, second.Project.Imports.Value.Length)

[<Fact>]
let ``the preview shows each tag with its clip and whether it is new`` () =
    let sheet =
        AsepriteImport.parseJson (File.ReadAllText(fixture [ "aseprite"; "hero-array.json" ]))
        |> ok

    let before = AsepriteImport.preview (starter ()) "/art/hero.json" None sheet []
    Assert.Equal<string list>([ "new"; "new"; "new" ], before |> List.map (fun m -> m.Status))
    Assert.Equal<string list>([ "walk_down"; "idleUp"; "portrait" ], before |> List.map (fun m -> m.Source))
    let project = (AsepriteImport.import (starter ()) (request sheet)).Project
    let after = AsepriteImport.preview project "/art/hero.json" None sheet []
    Assert.Equal<string list>([ "kept"; "kept"; "kept" ], after |> List.map (fun m -> m.Status))

[<Fact>]
let ``an aseprite file is composited onto one sheet with its tags and slices`` () =
    let sheet =
        AsepriteImport.parseFile (File.ReadAllBytes(fixture [ "aseprite"; "hero.aseprite" ]))
        |> ok

    Assert.Equal((12, 4), (sheet.SheetWidth, sheet.SheetHeight))
    Assert.Equal<int list>([ 100; 150; 200 ], sheet.Frames |> List.map (fun f -> f.DurationMs))
    Assert.Equal<string list>([ "walk_down"; "Idle Blink" ], sheet.Tags |> List.map (fun t -> t.Name))
    Assert.Equal("pingpong", sheet.Tags.[1].Direction)
    Assert.Equal("icon", sheet.Slices.Head.Name)
    Assert.True sheet.Png.IsSome
    let png = sheet.Png.Value
    Assert.Equal<byte[]>([| 0x89uy; 0x50uy; 0x4Euy; 0x47uy |], Array.sub png 0 4)
    // Re-encoded by the PNG writer: check pixels through a decode of our own (no filter, zlib).
    let idat =
        let rec find i =
            if Text.Encoding.ASCII.GetString(png, i + 4, 4) = "IDAT" then
                let length =
                    (int png.[i] <<< 24)
                    ||| (int png.[i + 1] <<< 16)
                    ||| (int png.[i + 2] <<< 8)
                    ||| int png.[i + 3]

                Array.sub png (i + 8) length
            else
                let length =
                    (int png.[i] <<< 24)
                    ||| (int png.[i + 1] <<< 16)
                    ||| (int png.[i + 2] <<< 8)
                    ||| int png.[i + 3]

                find (i + 12 + length)

        find 8

    use z =
        new Compression.ZLibStream(new MemoryStream(idat), Compression.CompressionMode.Decompress)

    use out = new MemoryStream()
    z.CopyTo out
    let raw = out.ToArray()
    let stride = 12 * 4 + 1

    let pixel x y =
        raw.[y * stride + 1 + x * 4], raw.[y * stride + 1 + x * 4 + 1], raw.[y * stride + 1 + x * 4 + 2], raw.[y * stride + 1 + x * 4 + 3]
    // Frame 0: red 2×2 at the top left; the hidden blue layer is not drawn.
    Assert.Equal((255uy, 0uy, 0uy, 255uy), pixel 1 1)
    Assert.Equal((0uy, 0uy, 0uy, 0uy), pixel 3 3)
    // Frame 1: the compressed green cel at (2, 2).
    Assert.Equal((0uy, 255uy, 0uy, 255uy), pixel 7 3)
    Assert.Equal((0uy, 0uy, 0uy, 0uy), pixel 4 0)
    // Frame 2 links frame 0's cel.
    Assert.Equal((255uy, 0uy, 0uy, 255uy), pixel 8 0)

    let result =
        AsepriteImport.import
            (starter ())
            { request sheet with
                Path = "/art/hero.aseprite"
                DataUrl = Png.dataUrl png
            }

    let made = asset result.Project result.AssetId
    Assert.Equal<string list>([ "walk-down"; "idle-blink"; "icon" ], clipNames made)
    Assert.Equal<float list>([ 2.0; 3.0 ], (clip made "walk-down").Frames |> List.map (fun f -> f.Ticks))

[<Fact>]
let ``damaged aseprite files are refused, never thrown`` () =
    let bytes = File.ReadAllBytes(fixture [ "aseprite"; "hero.aseprite" ])
    refused "not an Aseprite file" (AsepriteImport.parseFile [| 1uy; 2uy |])
    // Cut short at every length: an error message each time.
    for length in [ 128; 140; 200; 300; bytes.Length - 1 ] do
        match AsepriteImport.parseFile (Array.sub bytes 0 length) with
        | Error _ -> ()
        | Ok _ -> failwithf "a file cut at %d bytes was read" length

    // A sprite larger than the import limits.
    let big = Array.copy bytes
    big.[8] <- 0xFFuy
    big.[9] <- 0x7Fuy
    refused "8192" (AsepriteImport.parseFile big)

// ── Maps ─────────────────────────────────────────────────────────────

let private reader (path: string) =
    if File.Exists path then
        Some(File.ReadAllText path)
    else
        None

let private images (path: string) =
    if path.EndsWith ".png" then
        Some("data:image/png;base64,AAAA", 32, 32)
    else
        None

let private mapRequest (path: string) kind : MapImport.MapImportRequest =
    {
        Path = path
        Kind = kind
        Level = None
        SceneId = None
        SceneName = "Imported Farm"
        Image = images
    }

let private withTown (project: GameProject) =
    project
    |> apply (AddScene(AuthoringTiles.CreateEmptyScene("town", "Town", 8.0, 8.0)))

let private importTmx (project: GameProject) =
    let path = fixture [ "tiled"; "farm.tmx" ]
    let source = MapImport.parseTiled path (File.ReadAllText path) reader |> ok
    MapImport.import project (mapRequest path ImportKinds.Tiled) source |> ok

let private sceneOf (result: MapImportResult) = scene result.Project result.SceneId

[<Fact>]
let ``a Tiled map with an external tileset becomes a scene`` () =
    let project = withTown (starter ())
    let result = importTmx project
    Assert.True result.Created
    let s = sceneOf result
    Assert.Equal("imported-farm", s.Id)
    Assert.Equal((4.0, 3.0), (s.Width, s.Height))
    let t x y = s.Tiles.[y].[x]
    // terrain=soil matches the built-in soil.
    Assert.Equal("soil", (t 0 0).Background)
    // A tile without a terrain property gets a terrain of its own, with its art.
    let water =
        result.Project.Terrains.Value |> List.find (fun t -> t.Id = "farm-tiles-1")

    Assert.True water.WaterSource
    Assert.False water.Walkable
    Assert.Equal(16.0, water.Visual.Value.Frame.Value.X)
    Assert.Equal("farm-tiles-1", (t 1 1).Background)
    Assert.True (t 1 1).Collision
    // terrain=Tall Hedge is created on the object layer.
    Assert.Equal(Some "tall-hedge", (t 3 0).Object)
    Assert.True (t 3 0).Collision
    // The Treetops layer is an extra layer drawn above.
    let layer = s.Layers.Value |> List.find (fun l -> l.Id = "treetops")
    Assert.Equal(Some true, layer.Above)
    Assert.Equal(Some [ "treetops", "tall-hedge" ], (t 3 1).Decor)
    // Gid 9 is in no tileset: the report says where.
    let unmapped = result.Report |> List.find (fun p -> p.Code = "import.unmappedTile")
    Assert.Contains("farm.tmx layer 'Ground' (3, 2)", unmapped.Path)
    Assert.Equal(Some(NavigationTarget.Scene("imported-farm", 3, 2)), unmapped.Target)

[<Fact>]
let ``Tiled objects become doors, the start, NPCs, stops, nodes, lights and events`` () =
    let project = withTown (starter ())

    let mira =
        { (npc project "npc-farmer") with
            Id = "mira"
            Name = "Mira"
        }

    let project = project |> apply (UpsertNpc mira)
    let result = importTmx project
    let s = sceneOf result
    let door = s.Transitions |> List.exactlyOne
    Assert.Equal((3.0, 2.0, "town", 5.0, 6.0), (door.FromX, door.FromY, door.ToSceneId, door.ToX, door.ToY))
    Assert.Equal((1.0, 1.0, s.Id), (result.Project.Player.X, result.Project.Player.Y, result.Project.Player.SceneId))
    Assert.Equal(s.Id, result.Project.StartSceneId)
    let mira = npc result.Project "mira"
    Assert.Equal((s.Id, 0.0, 2.0), (mira.SceneId, mira.X, mira.Y))
    Assert.Contains("mira", s.Npcs)
    let stop = mira.Schedule.Value |> List.exactlyOne
    Assert.Equal((510.0, 2.0, 0.0), (stop.Minute, stop.X, stop.Y))
    Assert.Equal(Some "node-rock", s.Tiles.[1].[2].Node |> Option.map (fun n -> n.TypeId))
    Assert.Equal(3.0, s.Tiles.[1].[2].Node.Value.RemainingHealth)

    let light =
        s.Tiles.[0].[0].Decor.Value
        |> List.find (fun (layer, _) -> layer = "lights")
        |> snd

    let lightTerrain =
        result.Project.Terrains.Value |> List.find (fun t -> t.Id = light)

    Assert.Equal(4.0, lightTerrain.Light.Value.Radius)
    Assert.Equal("transparent", lightTerrain.Color)
    let welcome = result.Project.Events |> List.find (fun e -> e.Id = "welcome")
    Assert.Equal(s.Id, welcome.SceneId)

    match welcome.Conditions with
    | [ EventCondition.EnterTile area ] -> Assert.Equal((0.0, 0.0, Some 1.0, None), (area.X, area.Y, area.X2, area.Y2))
    | other -> failwithf "%A" other

    let statue = result.Report |> List.find (fun p -> p.Code = "import.unmappedObject")
    Assert.Contains("mystery", statue.Message)
    Assert.Contains("(1, 0)", statue.Path)

[<Fact>]
let ``re-import updates tiles and markers and leaves play state alone`` () =
    let project = withTown (starter ())
    let first = importTmx project
    let sceneId = first.SceneId
    // Play changed the scene: a crop on the soil, the rock half broken, wet soil.
    let played =
        let s = scene first.Project sceneId

        let tiles =
            s.Tiles
            |> List.map (
                List.map (fun t ->
                    match int t.X, int t.Y with
                    | 0, 0 ->
                        { t with
                            Crop = Some { Crop.Default with Type = "wheat" }
                            SoilMoisture = 80.0
                        }
                    | 2, 1 ->
                        { t with
                            Node = t.Node |> Option.map (fun n -> { n with RemainingHealth = 1.0 })
                        }
                    | _ -> t)
            )

        { first.Project with
            Scenes =
                first.Project.Scenes
                |> List.map (fun x ->
                    if x.Id = sceneId then
                        { s with
                            Tiles = tiles
                            Music = Some "song"
                        }
                    else
                        x)
        }

    let again = importTmx played
    Assert.False again.Created
    Assert.Equal(sceneId, again.SceneId)
    Assert.Equal(project.Scenes.Length + 1, again.Project.Scenes.Length)
    let s = sceneOf again
    Assert.Equal(Some "wheat", s.Tiles.[0].[0].Crop |> Option.map (fun c -> c.Type))
    Assert.Equal(80.0, s.Tiles.[0].[0].SoilMoisture)
    Assert.Equal(1.0, s.Tiles.[1].[2].Node.Value.RemainingHealth)
    Assert.Equal(Some "song", s.Music)
    Assert.Equal(1, s.Transitions.Length)
    Assert.Equal(1, again.Project.Imports.Value.Length)
    Assert.Equal(1, again.Project.Events |> List.filter (fun e -> e.Id = "welcome") |> List.length)

[<Fact>]
let ``a Tiled JSON map with an inline tileset, a group and flipped tiles`` () =
    let path = fixture [ "tiled"; "farm.tmj" ]
    let source = MapImport.parseTiled path (File.ReadAllText path) reader |> ok

    let result =
        MapImport.import (starter ()) (mapRequest path ImportKinds.Tiled) source |> ok

    let s = sceneOf result
    Assert.Equal("inline-0", s.Tiles.[0].[0].Background)

    let plantable =
        result.Project.Terrains.Value |> List.find (fun t -> t.Id = "inline-2")

    Assert.True plantable.Plantable
    // The flipped gid is imported as its tile, with a warning.
    Assert.Equal("inline-0", s.Tiles.[1].[1].Background)
    Assert.Contains(result.Report, fun p -> p.Code = "import.flippedTile")
    Assert.Equal(Some [ "rugs", "inline-3" ], s.Tiles.[1].[0].Decor)
    Assert.Equal("farm", (s.Transitions |> List.exactlyOne).ToSceneId)

[<Fact>]
let ``an LDtk level imports its tiles, IntGrid values and entities`` () =
    let path = fixture [ "ldtk"; "world.ldtk" ]
    let text = File.ReadAllText path
    Assert.Equal<string list>([ "Level_0"; "Level_1" ], MapImport.ldtkLevels text)
    let source = MapImport.parseLdtk path text (Some "Level_0") reader |> ok

    let result =
        MapImport.import
            (starter ())
            { mapRequest path ImportKinds.Ldtk with
                Level = Some "Level_0"
            }
            source
        |> ok

    let s = sceneOf result
    Assert.Equal((3.0, 2.0), (s.Width, s.Height))
    // The enum tag Soil matches the built-in soil; customData sets a terrain's properties.
    Assert.Equal("soil", s.Tiles.[0].[0].Background)
    let fence = result.Project.Terrains.Value |> List.find (fun t -> t.Id = "farm-2")
    Assert.Equal("Fence", fence.Name)
    Assert.True s.Tiles.[0].[1].Collision
    // IntGrid values are terrains by their identifier: built-in wall and water.
    Assert.Equal("wall", s.Tiles.[0].[2].Background)
    Assert.Equal("water", s.Tiles.[1].[1].Background)
    Assert.Equal((1.0, 1.0), (result.Project.Player.X, result.Project.Player.Y))
    Assert.Contains(result.Report, fun p -> p.Code = "import.unknownMachine")
    Assert.Equal(Some "Level_0", (result.Project.Imports.Value |> List.exactlyOne).Level)

[<Fact>]
let ``maps past the scene limits and broken files are refused`` () =
    let tooBig =
        {
            File = "big.tmx"
            Width = 513
            Height = 4
            TileWidth = 16
            TileHeight = 16
            Tilesets = []
            Layers = []
            Objects = []
            Warnings = []
        }

    refused "at most 512" (MapImport.import (starter ()) (mapRequest "big.tmx" ImportKinds.Tiled) tooBig)
    refused "Tiled map" (MapImport.parseTiled "x.tmx" "<other/>" reader)
    refused "Infinite" (MapImport.parseTiled "x.tmx" "<map orientation=\"orthogonal\" infinite=\"1\"/>" reader)
    refused "tileset" (MapImport.parseTiled "x.tmx" "<map width=\"1\" height=\"1\"><tileset firstgid=\"1\" source=\"missing.tsx\"/></map>" reader)
    // A DTD is refused (no entity expansion, nothing fetched).
    refused "XML" (MapImport.parseTiled "x.tmx" "<!DOCTYPE map [<!ENTITY a \"b\">]><map/>" reader)

[<Fact>]
let ``schedule times read as clock times or minutes`` () =
    Assert.Equal(Some 510.0, MapImport.minuteOf "8:30")
    Assert.Equal(Some 1230.0, MapImport.minuteOf "8:30pm")
    Assert.Equal(Some 30.0, MapImport.minuteOf "12:30am")
    Assert.Equal(Some 600.0, MapImport.minuteOf "600")
    Assert.Equal(None, MapImport.minuteOf "25:00")
