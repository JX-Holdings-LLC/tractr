namespace FarmEngine.Authoring

/// The built-in art an NPC without its own art is drawn with, chosen by its `appearance`: the
/// F# mirror of `BuiltinArt::npc_entry` in `crates/farm-render/src/builtin_art.rs`, so Problems
/// can say which sheet an appearance falls back to. Looks drawn by recolouring the base character
/// (fisher, miner, …) are listed in `CharacterArt.colouredAppearances`. **Add a new built-in sheet here** (to
/// `Entries`, and a rule to `NpcRules` if names should guess it) when it is added to
/// `assets/builtin-art/manifest.json`: the authoring tests compare `Entries` with the manifest,
/// and the Rust and F# tests both check the cases in `fixtures/art/npc-appearances.json`.
module BuiltinLooks =
    /// Every entry of the built-in art pack (`assets/builtin-art/manifest.json`), by name.
    let Entries: string list =
        [
            "animal-chicken"
            "animal-cow"
            "char-player"
            "crop-blueberry"
            "crop-carrot"
            "crop-cauliflower"
            "crop-corn"
            "crop-generic"
            "crop-potato"
            "crop-pumpkin"
            "crop-strawberry"
            "crop-tomato"
            "crop-wheat"
            "crop-withered"
            "fx-ready"
            "item-generic"
            "item-material"
            "item-seed"
            "machine-altar"
            "machine-furnace"
            "machine-generic"
            "machine-kitchen"
            "machine-preserves"
            "machine-workbench"
            "node-boulder"
            "node-copper-ore"
            "node-iron-ore"
            "node-mine-rock"
            "node-quartz"
            "node-rock"
            "node-stump"
            "node-tree"
            "node-weeds"
            "npc-farmer"
            "npc-merchant"
            "npc-villager"
            "tile-door"
            "tile-floor"
            "tile-grass"
            "tile-grass-fall"
            "tile-grass-summer"
            "tile-grass-winter"
            "tile-ladder"
            "tile-path"
            "tile-soil"
            "tile-soil-fertilized"
            "tile-soil-watered"
            "tile-wall"
            "tile-water"
        ]

    /// The built-in NPC looks: the entries an appearance can name without the `npc-` prefix
    /// (`"farmer"` is `npc-farmer`).
    let NpcSheets: string list =
        Entries
        |> List.filter (fun name -> name.StartsWith("npc-", System.StringComparison.Ordinal))

    /// The guesses for an appearance that names no entry, tried in order: the first keyword the
    /// lower-cased appearance contains picks the sheet.
    let NpcRules: (string * string) list =
        [
            "farm", "npc-farmer"
            "elder", "npc-farmer"
            "merchant", "npc-merchant"
            "shop", "npc-merchant"
            "vendor", "npc-merchant"
        ]

    /// The sheet an NPC without its own art falls back to when nothing else fits.
    [<Literal>]
    let NpcFallback = "npc-villager"

    let private entrySet = Set.ofList Entries

    /// The entry `appearance` names exactly (`npc-<appearance>` or the entry's own name), if any.
    let exactNpcSheet (appearance: string) : string option =
        if appearance.Length = 0 then
            None
        elif entrySet.Contains("npc-" + appearance) then
            Some("npc-" + appearance)
        elif entrySet.Contains appearance then
            Some appearance
        else
            None

    /// The sheet an NPC with `appearance` is drawn with: the exact entry, else the first rule
    /// whose keyword it contains, else `NpcFallback`.
    let npcSheet (appearance: string) : string =
        match exactNpcSheet appearance with
        | Some sheet -> sheet
        | None ->
            let lower = appearance.ToLowerInvariant()

            NpcRules
            |> List.tryFind (fun (keyword, _) -> lower.Contains keyword)
            |> Option.map snd
            |> Option.defaultValue NpcFallback
