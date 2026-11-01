namespace FarmEngine.Authoring

open FarmEngine.Schemas

/// What the built-in character art understands (#216, #234): the appearances the renderer draws
/// without creator art and the colour format of looks and layers. The renderer's own lists are in
/// `crates/farm-render` (`builtin_art.rs`, `look.rs`); keep them in step.
module CharacterArt =
    /// Appearances drawn from hand-drawn built-in art.
    let handDrawnAppearances = [ "farmer"; "merchant"; "villager" ]

    /// Appearances drawn as the base character in built-in colours (`look::APPEARANCE_LOOKS`). An
    /// appearance that contains one of these names picks it.
    let colouredAppearances =
        [
            "fisher"
            "miner"
            "baker"
            "doctor"
            "blacksmith"
            "noble"
            "gardener"
            "sailor"
        ]

    /// Every built-in appearance, hand-drawn first.
    let builtinAppearances = handDrawnAppearances @ colouredAppearances

    /// `#rrggbb`.
    let isHexColor (value: string) =
        value.Length = 7
        && value.[0] = '#'
        && value.Substring 1 |> Seq.forall System.Uri.IsHexDigit

    /// The colours of a look with their JSON keys.
    let lookParts (look: PlayerLook) =
        [
            "skin", look.Skin
            "hair", look.Hair
            "shirt", look.Shirt
            "pants", look.Pants
            "accent", look.Accent
        ]
