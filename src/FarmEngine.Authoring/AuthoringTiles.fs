namespace FarmEngine.Authoring

open FarmEngine.Schemas

/// Tile construction and layer edits from game-helpers.ts. These transform authored maps;
/// collision queries and movement during play remain in Rust.
[<AbstractClass; Sealed>]
type AuthoringTiles =
    static member ClassifyTileType(tileType: string) =
        match tileType with
        | "path" -> "overlay"
        | "wall"
        | "door" -> "object"
        | _ -> "background"

    static member CreateEmptyTile(x: float, y: float, tileType: string) : Tile =
        let layer = AuthoringTiles.ClassifyTileType tileType

        { Tile.Default with
            X = x
            Y = y
            Type = tileType
            Background = (if layer = "background" then tileType else "grass")
            Overlay = (if layer = "overlay" then Some tileType else None)
            Object = (if layer = "object" then Some tileType else None)
            Collision = (tileType = "wall")
            SoilMoisture = 0.0
            SoilFertility = 0.0
        }

    static member SetTileLayer(tile: Tile, tileType: string, visual: VisualRef option) : Tile =
        let layer = AuthoringTiles.ClassifyTileType tileType

        let tile =
            match layer with
            | "background" -> { tile with Background = tileType }
            | "overlay" -> { tile with Overlay = Some tileType }
            | _ ->
                { tile with
                    Object = Some tileType
                    Collision = (tileType = "wall")
                }

        let tile = { tile with Type = tileType }

        if visual.IsSome || tile.Visuals.IsSome then
            let visuals = defaultArg tile.Visuals TileVisuals.Default

            let visuals =
                match layer with
                | "background" -> { visuals with Background = visual }
                | "overlay" -> { visuals with Overlay = visual }
                | _ -> { visuals with Object = visual }

            { tile with Visuals = Some visuals }
        else
            tile

    static member SetTileLayer(tile: Tile, tileType: string) =
        AuthoringTiles.SetTileLayer(tile, tileType, None)

    /// The layer of `tileType` among a game's terrains (M1): its definition's, else the
    /// built-in rule (Rust `vocabulary::layer_of`).
    static member ClassifyIn(terrains: TerrainDefinition list, tileType: string) =
        match terrains |> List.tryFind (fun t -> t.Id = tileType) with
        | Some terrain when
            terrain.Layer = "background"
            || terrain.Layer = "overlay"
            || terrain.Layer = "object"
            ->
            terrain.Layer
        | Some _ -> "background"
        | None -> AuthoringTiles.ClassifyTileType tileType

    /// Whether a terrain can be walked on (an unknown one can; the built-in wall cannot).
    static member WalkableIn(terrains: TerrainDefinition list, tileType: string) =
        match terrains |> List.tryFind (fun t -> t.Id = tileType) with
        | Some terrain -> terrain.Walkable
        | None -> tileType <> "wall"

    /// `SetTileLayer` among a game's terrains (Rust `tiles::set_tile_layer_in`): an object sets
    /// the tile's collision; a background or overlay that can't be walked on sets it, and one
    /// that can clears what the terrain it replaces had set, unless the object still blocks.
    static member SetTileLayerIn(terrains: TerrainDefinition list, tile: Tile, tileType: string, visual: VisualRef option) : Tile =
        let layer = AuthoringTiles.ClassifyIn(terrains, tileType)
        let walkable = AuthoringTiles.WalkableIn(terrains, tileType)

        let blocks (id: string option) =
            id |> Option.exists (fun id -> not (AuthoringTiles.WalkableIn(terrains, id)))

        let painted =
            match layer with
            | "background" ->
                let collision =
                    if not walkable then true
                    elif blocks (Some tile.Background) then blocks tile.Object
                    else tile.Collision

                { tile with
                    Background = tileType
                    Collision = collision
                }
            | "overlay" ->
                let collision =
                    if not walkable then true
                    elif blocks tile.Overlay then blocks tile.Object
                    else tile.Collision

                { tile with
                    Overlay = Some tileType
                    Collision = collision
                }
            | _ ->
                { tile with
                    Object = Some tileType
                    Collision = not walkable
                }

        let painted = { painted with Type = tileType }

        if visual.IsSome || painted.Visuals.IsSome then
            let visuals = defaultArg painted.Visuals TileVisuals.Default

            let visuals =
                match layer with
                | "background" -> { visuals with Background = visual }
                | "overlay" -> { visuals with Overlay = visual }
                | _ -> { visuals with Object = visual }

            { painted with Visuals = Some visuals }
        else
            painted

    static member CreateEmptyScene(id: string, name: string, width: float, height: float) : Scene =
        let rows =
            [
                let mutable y = 0.0

                while y < height do
                    yield
                        [
                            let mutable x = 0.0

                            while x < width do
                                yield AuthoringTiles.CreateEmptyTile(x, y, "grass")
                                x <- x + 1.0
                        ]

                    y <- y + 1.0
            ]

        { Scene.Default with
            Id = id
            Name = name
            Width = width
            Height = height
            Tiles = rows
        }
