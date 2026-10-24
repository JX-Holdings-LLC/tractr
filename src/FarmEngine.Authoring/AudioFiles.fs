namespace FarmEngine.Authoring

open System
open System.Collections.Generic
open System.Runtime.CompilerServices

/// What a sound or music file holds, as far as its headers tell.
type AudioFileInfo =
    { /// "ogg" (Ogg Vorbis) or "wav".
      Format: string
      Channels: int
      SampleRate: int
      /// The length the headers give (the last Ogg page's position, a WAV file's data size).
      Seconds: float }

/// Whether the player can decode a sound or music file (`farm_player::audio_assets`: lewton for
/// Ogg Vorbis, hound for WAV). The player leaves out a file it cannot decode and plays the
/// built-in sound or music instead, without a word; Problems uses this check to say so first.
///
/// F# does not decode the audio. It walks the file the way the decoders read it: for Ogg every
/// page (capture pattern, version, stream flags, length and CRC) and the three Vorbis headers;
/// for WAV the RIFF chunks, the `fmt ` chunk (the formats and sample sizes hound reads) and the
/// `data` chunk. A file refused here never plays. A file that passes can still fail on a damaged
/// audio packet, which only decoding finds.
module AudioFiles =
    /// The longest file the player decodes, in seconds (`audio_assets::MAX_SECONDS`).
    [<Literal>]
    let MaxSeconds = 600

    /// The sample rates the player accepts (`audio_assets::RATES`).
    let private rateOk (rate: int64) = rate >= 1000L && rate <= 384000L

    let private rateError (rate: int64) =
        sprintf "its sample rate (%d Hz) is not one the game plays (1000 to 384000 Hz)" rate

    let private tooLong = sprintf "it is longer than %d minutes" (MaxSeconds / 60)

    let private u16 (b: byte[]) (i: int) = int b.[i] ||| (int b.[i + 1] <<< 8)

    let private u32 (b: byte[]) (i: int) =
        int64 b.[i] ||| (int64 b.[i + 1] <<< 8) ||| (int64 b.[i + 2] <<< 16) ||| (int64 b.[i + 3] <<< 24)

    let private u64 (b: byte[]) (i: int) = uint64 (u32 b i) ||| (uint64 (u32 b (i + 4)) <<< 32)

    let private tagAt (b: byte[]) (i: int) (tag: string) =
        i >= 0 && b.Length >= i + tag.Length && Seq.forall (fun k -> b.[i + k] = byte tag.[k]) (seq { 0 .. tag.Length - 1 })

    // ---- Ogg Vorbis (the ogg crate's PacketReader and lewton's header readers) ----

    /// The Ogg CRC-32 (polynomial 0x04c11db7, not reflected, no final xor).
    let private crcTable =
        Array.init 256 (fun i ->
            let mutable r = uint32 i <<< 24
            for _ in 0..7 do
                r <- if r &&& 0x80000000u <> 0u then (r <<< 1) ^^^ 0x04c11db7u else r <<< 1
            r)

    /// The CRC of the page at `start` (`length` bytes), its checksum field counted as zeros.
    let private pageCrc (b: byte[]) (start: int) (length: int) =
        let mutable crc = 0u
        for i in start .. start + length - 1 do
            let value = if i >= start + 22 && i < start + 26 then 0uy else b.[i]
            crc <- (crc <<< 8) ^^^ crcTable.[int ((crc >>> 24) ^^^ uint32 value)]
        crc

    /// Where the next capture pattern starts at or after `from`, or -1.
    let private nextCapture (b: byte[]) (from: int) =
        let mutable i = from
        let mutable found = -1
        while found < 0 && i + 4 <= b.Length do
            if b.[i] = byte 'O' && b.[i + 1] = byte 'g' && b.[i + 2] = byte 'g' && b.[i + 3] = byte 'S' then found <- i
            else i <- i + 1
        found

    /// One Ogg page: where its lacing values and body are in the file.
    [<Struct>]
    type private Page =
        { Start: int
          Segments: int
          Flags: int
          Serial: int64
          Granule: uint64 }

    /// Reads every page as the decoder does: it reads to the end of the file, so damage anywhere
    /// stops the whole file.
    let private oggPages (b: byte[]) : Result<Page list, string> =
        let pages = ResizeArray<Page>()
        let mutable error = None
        let mutable pos = 0
        while error.IsNone && pos < b.Length do
            let start = nextCapture b pos
            if start < 0 then error <- Some "the Ogg file has other data after its last page"
            elif start + 27 > b.Length then error <- Some "the Ogg file is cut short"
            elif b.[start + 4] <> 0uy then error <- Some "the Ogg file has an unknown version"
            else
                let segments = int b.[start + 26]
                let mutable body = 0
                if start + 27 + segments <= b.Length then
                    for k in 0 .. segments - 1 do
                        body <- body + int b.[start + 27 + k]
                let length = 27 + segments + body
                if start + 27 + segments > b.Length || start + length > b.Length then error <- Some "the Ogg file is cut short"
                elif uint32 (u32 b (start + 22)) <> pageCrc b start length then
                    error <- Some "the Ogg file is damaged (a page's checksum does not match)"
                else
                    pages.Add { Start = start; Segments = segments; Flags = int b.[start + 5]; Serial = u32 b (start + 14); Granule = u64 b (start + 6) }
                    pos <- start + length
        match error with
        | Some reason -> Error reason
        | None -> Ok(List.ofSeq pages)

    /// The first `wanted` packets of the stream `serial`, after checking how every stream's
    /// pages follow each other (a stream starts on a first page, and a page continues a packet
    /// exactly when the page before it left one open).
    let private oggPackets (b: byte[]) (pages: Page list) (serial: int64) (wanted: int) : Result<byte[] list, string> =
        let open' = Dictionary<int64, bool>()
        let packets = ResizeArray<byte[]>()
        let pending = ResizeArray<byte>()
        let mutable error = None
        for page in pages do
            if error.IsNone then
                let continued = page.Flags &&& 1 <> 0
                let first = page.Flags &&& 2 <> 0
                match open'.TryGetValue page.Serial with
                | false, _ when not first || continued -> error <- Some "the Ogg file is damaged (a stream starts in the middle)"
                | true, _ when first -> error <- Some "the Ogg file is damaged (a stream starts twice)"
                | true, wasOpen when wasOpen <> continued -> error <- Some "the Ogg file is damaged (a packet is missing a part)"
                | _ ->
                    let mutable ends = continued
                    let mutable at = page.Start + 27 + page.Segments
                    for k in 0 .. page.Segments - 1 do
                        let size = int b.[page.Start + 27 + k]
                        if page.Serial = serial && packets.Count < wanted then
                            pending.AddRange(ArraySegment(b, at, size))
                            if size < 255 then
                                packets.Add(pending.ToArray())
                                pending.Clear()
                        at <- at + size
                        ends <- size = 255
                    open'.[page.Serial] <- ends
        match error with
        | Some reason -> Error reason
        | None -> Ok(List.ofSeq packets)

    /// The vendor string and comments must fit the packet (lewton reads them all), the vendor
    /// must be UTF-8, and the framing byte must be 1.
    let private commentOk (packet: byte[]) =
        let fits (at: int64) (count: int64) = at + count <= int64 packet.Length
        if not (fits 7L 4L) then false
        else
            let vendor = u32 packet 7
            let utf8 =
                fits 11L vendor
                && (try
                        Text.UTF8Encoding(false, true).GetString(packet, 11, int vendor) |> ignore
                        true
                    with :? ArgumentException ->
                        false)
            let mutable at = 11L + vendor
            let mutable ok = utf8 && fits at 4L
            if ok then
                let count = u32 packet (int at)
                at <- at + 4L
                let mutable i = 0L
                while ok && i < count do
                    ok <- fits at 4L
                    if ok then
                        at <- at + 4L + u32 packet (int at)
                        ok <- fits at 0L
                    i <- i + 1L
            ok && fits at 1L && packet.[int at] = 1uy

    let private header (packet: byte[]) (kind: int) = packet.Length >= 7 && int packet.[0] = kind && tagAt packet 1 "vorbis"

    let private ogg (b: byte[]) : Result<AudioFileInfo, string> =
        match oggPages b with
        | Error reason -> Error reason
        | Ok [] -> Error "the Ogg file has no pages"
        | Ok pages ->
            let serial = pages.Head.Serial
            match oggPackets b pages serial 4 with
            | Error reason -> Error reason
            | Ok packets ->
                let damaged = Error "the Ogg file's Vorbis headers are damaged"
                match packets with
                | [] -> Error "the Ogg file is cut short"
                | ident :: _ when tagAt ident 0 "OpusHead" -> Error "it is Ogg Opus; the game plays Ogg Vorbis"
                | ident :: _ when not (header ident 1) -> Error "the Ogg file is not Vorbis audio"
                | ident :: _ when ident.Length < 30 -> damaged
                | ident :: rest ->
                    let channels = int ident.[11]
                    let rate = u32 ident 12
                    let small, large = int ident.[28] &&& 0xF, int ident.[28] >>> 4
                    if u32 ident 7 <> 0L || small < 6 || small > 13 || large < 6 || large > 13 || small > large || ident.[29] <> 1uy then damaged
                    elif channels = 0 || rate = 0L then damaged
                    elif not (rateOk rate) then Error(rateError rate)
                    else
                        match rest with
                        | comment :: setup :: audio when header comment 3 && commentOk comment && header setup 5 ->
                            let frames =
                                pages
                                |> List.filter (fun page -> page.Serial = serial && page.Granule <> UInt64.MaxValue)
                                |> List.fold (fun most page -> max most page.Granule) 0UL
                            if audio.IsEmpty then Error "it has no sound"
                            elif frames > uint64 rate * uint64 MaxSeconds then Error tooLong
                            else Ok { Format = "ogg"; Channels = channels; SampleRate = int rate; Seconds = float frames / float rate }
                        | [] | [ _ ] -> Error "the Ogg file is cut short"
                        | _ -> damaged

    // ---- WAV (hound's WavReader) ----

    /// hound's sample layouts: bytes per sample and bits, integer or float.
    let private layoutOk (float': bool) (bytes: int) (bits: int) =
        if float' then (bytes, bits) = (4, 32)
        else List.contains (bytes, bits) [ (1, 8); (2, 16); (3, 24); (4, 24); (4, 32) ]

    /// The `fmt ` chunk at `at` (`length` bytes): channels, rate, bytes per sample, bits, float,
    /// and how many bytes hound reads of it (it does not skip the rest of a long extensible one).
    let private wavFormat (b: byte[]) (at: int) (length: int64) : Result<int * int64 * int * int * bool * int, string> =
        let need (count: int) = at + count <= b.Length
        let unsupported = Error "the WAV file is compressed or in a format the game can't play (use PCM or 32-bit float)"
        if length < 16L then Error "the WAV file's format chunk is damaged"
        elif not (need 16) then Error "the WAV file is cut short"
        else
            let tag = u16 b at
            let channels = u16 b (at + 2)
            let rate = u32 b (at + 4)
            let perSecond = u32 b (at + 8)
            let align = u16 b (at + 12)
            let bits = u16 b (at + 14)
            if channels = 0 then Error "the WAV file has no channels"
            elif bits > align / channels * 8 || perSecond <> int64 align * rate || bits % 8 <> 0 || bits = 0 then
                Error "the WAV file's format chunk is damaged"
            else
                let bytes = align / channels
                match tag with
                | 0x0001 ->
                    match length with
                    | 16L -> Ok(channels, rate, bytes, bits, false, 16)
                    | 18L | 40L when bits = 8 || bits = 16 || bits = 24 -> Ok(channels, rate, bytes, bits, false, int length)
                    | 18L | 40L -> unsupported
                    | _ -> Error "the WAV file's format chunk is damaged"
                | 0x0003 ->
                    if length <> 16L && length <> 18L then Error "the WAV file's format chunk is damaged"
                    elif length = 18L && not (need 18) then Error "the WAV file is cut short"
                    elif length = 18L && u16 b (at + 16) <> 0 then Error "the WAV file's format chunk is damaged"
                    elif bits <> 32 then unsupported
                    else Ok(channels, rate, bytes, bits, true, int length)
                | 0xFFFE ->
                    if length < 40L then Error "the WAV file's format chunk is damaged"
                    elif not (need 40) then Error "the WAV file is cut short"
                    elif u16 b (at + 16) <> 22 then Error "the WAV file's format chunk is damaged"
                    else
                        let valid = u16 b (at + 18)
                        // KSDATAFORMAT_SUBTYPE_PCM or _IEEE_FLOAT: {0000000x-0000-0010-8000-00aa00389b71}.
                        let guidTail = [| 0x00uy; 0x00uy; 0x00uy; 0x00uy; 0x10uy; 0x00uy; 0x80uy; 0x00uy; 0x00uy; 0xAAuy; 0x00uy; 0x38uy; 0x9Buy; 0x71uy |]
                        let sub = u16 b (at + 24)
                        if Array.sub b (at + 26) 14 <> guidTail || (sub <> 1 && sub <> 3) then unsupported
                        else Ok(channels, rate, bytes, (if valid > 0 then valid else bits), (sub = 3), 40)
                | _ -> unsupported

    let private wav (b: byte[]) : Result<AudioFileInfo, string> =
        let cut = Error "the WAV file is cut short"
        // The chunks after "RIFF", size, "WAVE"; hound stops at the first `data` chunk.
        let rec chunks (at: int) (format: (int * int64 * int * int * bool) option) =
            if at + 8 > b.Length then (if format.IsNone then Error "the WAV file has no format chunk" else Error "the WAV file has no data chunk")
            else
                let length = u32 b (at + 4)
                let body = at + 8
                if tagAt b at "fmt " then
                    match wavFormat b body length with
                    | Error reason -> Error reason
                    | Ok(channels, rate, bytes, bits, float', read) -> chunks (body + read) (Some(channels, rate, bytes, bits, float'))
                elif tagAt b at "fact" then
                    // hound reads the sample count and nothing else of it.
                    chunks (body + 4) format
                elif tagAt b at "data" then
                    match format with
                    | None -> Error "the WAV file has no format chunk before its data"
                    | Some(channels, rate, bytes, bits, float') ->
                        let samples = if bytes = 0 then 0L else length / int64 bytes
                        if bytes = 0 || samples * int64 bytes <> length || samples % int64 channels <> 0L then
                            Error "the WAV file's data chunk is damaged"
                        elif not (rateOk rate) then Error(rateError rate)
                        elif int64 body + length > int64 b.Length then cut
                        elif samples = 0L then Error "it has no sound"
                        elif not (layoutOk float' bytes bits) then Error "the WAV file's sample size is not one the game plays (8, 16, 24 or 32-bit PCM, or 32-bit float)"
                        else
                            let frames = samples / int64 channels
                            if frames / rate > int64 MaxSeconds then Error tooLong
                            else Ok { Format = "wav"; Channels = channels; SampleRate = int rate; Seconds = float frames / float rate }
                elif int64 body + length > int64 b.Length then cut
                else chunks (body + int length) format
        chunks 12 None

    /// Whether the player can decode `bytes` (see the module): the file's format, channels, rate
    /// and length, or why it won't play.
    let check (bytes: byte[]) : Result<AudioFileInfo, string> =
        if bytes.Length = 0 then Error "the file is empty"
        elif tagAt bytes 0 "RIFF" && tagAt bytes 8 "WAVE" then wav bytes
        elif tagAt bytes 0 "OggS" then ogg bytes
        else Error "it is not an Ogg Vorbis or WAV file"

    /// The bytes of a base64 `data:` URL, read as leniently as the player reads them (missing
    /// padding is fine); `None` when it is not one.
    let dataUrlBytes (url: string) : byte[] option =
        let comma = url.IndexOf ','
        if comma < 5 || not (url.StartsWith("data:", StringComparison.OrdinalIgnoreCase))
           || not (url.Substring(0, comma).EndsWith(";base64", StringComparison.OrdinalIgnoreCase)) then None
        else
            let payload = url.Substring(comma + 1) |> String.filter (fun c -> not (Char.IsWhiteSpace c))
            let padded = payload.TrimEnd('=') |> fun body -> body + String('=', (4 - body.Length % 4) % 4)
            try Some(Convert.FromBase64String padded) with :? FormatException -> None

    /// What the player makes of an asset's file, by its data URL.
    let private checkUrlUncached (url: string) =
        match dataUrlBytes url with
        | None -> Error "its file is not a base64 data: URL"
        | Some bytes -> check bytes

    /// Results by data URL instance: Problems runs again after every edit, and a project's audio
    /// strings stay the same instances until their asset changes.
    let private cache = ConditionalWeakTable<string, Result<AudioFileInfo, string> ref>()

    /// `check` for an asset's data URL (remembered per URL string).
    let checkDataUrl (url: string) : Result<AudioFileInfo, string> =
        cache.GetValue(url, ConditionalWeakTable<string, Result<AudioFileInfo, string> ref>.CreateValueCallback(fun url -> ref (checkUrlUncached url))).Value
