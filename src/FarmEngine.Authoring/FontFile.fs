namespace FarmEngine.Authoring

open System

/// TrueType and OpenType font files a game brings for its interface skin (#236): enough of the
/// file to know it is a font and which characters it has (its `cmap`), read in managed code
/// with every offset checked. The player reads the same files with `ttf-parser`.
[<CompilationRepresentation(CompilationRepresentationFlags.ModuleSuffix)>]
module FontFile =
    /// The largest font file a skin may use.
    [<Literal>]
    let MaxBytes = 16777216

    let private u16 (b: byte[]) (at: int) =
        if at < 0 || at + 2 > b.Length then
            None
        else
            Some((int b.[at] <<< 8) ||| int b.[at + 1])

    let private u32 (b: byte[]) (at: int) =
        if at < 0 || at + 4 > b.Length then
            None
        else
            Some(
                (int64 b.[at] <<< 24)
                ||| (int64 b.[at + 1] <<< 16)
                ||| (int64 b.[at + 2] <<< 8)
                ||| int64 b.[at + 3]
            )

    /// The character ranges (first, last code point) a font maps to glyphs, or why the file is
    /// not a font Tractr can use: TrueType (`.ttf`) or OpenType (`.otf`) with a Unicode `cmap`
    /// of format 4 or 12. Font collections (`.ttc`) are refused.
    let coverage (bytes: byte[]) : Result<(int * int)[], string> =
        let fail message = Error message

        if bytes.Length < 12 then
            fail "it is not a font file"
        elif bytes.Length > MaxBytes then
            fail "it is larger than 16 MB"
        else
            match u32 bytes 0 with
            | Some 0x74746366L -> fail "font collections (.ttc) are not supported: use one .ttf or .otf"
            | Some version when version = 0x00010000L || version = 0x4F54544FL || version = 0x74727565L ->
                let tables = defaultArg (u16 bytes 4) 0

                let cmap =
                    [ 0 .. tables - 1 ]
                    |> List.tryPick (fun i ->
                        let record = 12 + i * 16

                        match u32 bytes record, u32 bytes (record + 8) with
                        | Some 0x636D6170L, Some offset -> Some(int offset)
                        | _ -> None)

                match cmap with
                | None -> fail "it has no character map"
                | Some cmap ->
                    let count = defaultArg (u16 bytes (cmap + 2)) 0

                    let subtables =
                        [
                            for i in 0 .. count - 1 do
                                let record = cmap + 4 + i * 8

                                match u16 bytes record, u16 bytes (record + 2), u32 bytes (record + 4) with
                                | Some platform, Some encoding, Some offset ->
                                    let at = cmap + int offset

                                    match u16 bytes at with
                                    | Some format when (platform = 0 || (platform = 3 && (encoding = 1 || encoding = 10))) ->
                                        yield format, at
                                    | _ -> ()
                                | _ -> ()
                        ]

                    let format12 = subtables |> List.tryFind (fun (f, _) -> f = 12)
                    let format4 = subtables |> List.tryFind (fun (f, _) -> f = 4)

                    match format12, format4 with
                    | Some(_, at), _ ->
                        let groups = defaultArg (u32 bytes (at + 12)) 0L

                        if groups > 100000L then
                            fail "its character map is damaged"
                        else
                            let ranges =
                                [|
                                    for g in 0 .. int groups - 1 do
                                        match u32 bytes (at + 16 + g * 12), u32 bytes (at + 20 + g * 12) with
                                        | Some first, Some last when first <= last && last <= 0x10FFFFL -> yield int first, int last
                                        | _ -> ()
                                |]

                            Ok ranges
                    | None, Some(_, at) ->
                        let segments = defaultArg (u16 bytes (at + 6)) 0 / 2
                        let ends = at + 14
                        let starts = ends + segments * 2 + 2

                        let ranges =
                            [|
                                for s in 0 .. segments - 1 do
                                    match u16 bytes (starts + s * 2), u16 bytes (ends + s * 2) with
                                    | Some first, Some last when first <= last && not (first = 0xFFFF) -> yield first, last
                                    | _ -> ()
                            |]

                        Ok ranges
                    | None, None -> fail "it has no Unicode character map"
            | _ -> fail "it is not a TrueType or OpenType font"

    /// The first character of `text` outside `ranges` (whitespace and control characters
    /// aside): it would draw as a box.
    let firstMissing (ranges: (int * int)[]) (text: string) : Text.Rune option =
        text.EnumerateRunes()
        |> Seq.tryFind (fun rune ->
            not (Text.Rune.IsWhiteSpace rune || Text.Rune.IsControl rune)
            && not (ranges |> Array.exists (fun (first, last) -> rune.Value >= first && rune.Value <= last)))
