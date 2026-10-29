/// Sound and music files the player cannot decode (`AudioFiles`, the `asset.audioUnreadable`
/// problem) and renaming audio assets (M11).
module FarmEngine.Authoring.Tests.AudioFilesTests

open System
open Xunit
open FarmEngine.Authoring
open FarmEngine.Authoring.Tests.TestProjects
open FarmEngine.Schemas

/// A quarter second of a 440 Hz tone, stereo at 22050 Hz (`fixtures/audio/tone.ogg`, the
/// player's own decoder test file).
let private tone () =
    IO.File.ReadAllBytes(testFile [ "Fixtures"; "audio"; "tone.ogg" ])

let private le16 (value: int) = [| byte value; byte (value >>> 8) |]

let private le32 (value: int64) =
    [| byte value; byte (value >>> 8); byte (value >>> 16); byte (value >>> 24) |]

let private chunk (tag: string) (body: byte[]) =
    Array.concat [ Text.Encoding.ASCII.GetBytes tag; le32 (int64 body.Length); body ]

let private riff (chunks: byte[] list) =
    let body = Array.concat (Text.Encoding.ASCII.GetBytes "WAVE" :: chunks)
    Array.concat [ Text.Encoding.ASCII.GetBytes "RIFF"; le32 (int64 body.Length); body ]

/// A `fmt ` chunk body: format tag, channels, rate, bytes per sample and bits (WAVEFORMAT plus
/// the bits field, 16 bytes).
let private fmt (tag: int) (channels: int) (rate: int) (bytes: int) (bits: int) =
    Array.concat
        [
            le16 tag
            le16 channels
            le32 (int64 rate)
            le32 (int64 (rate * channels * bytes))
            le16 (channels * bytes)
            le16 bits
        ]

/// A PCM WAV file of `frames` silent frames.
let private wav (channels: int) (rate: int) (bytes: int) (frames: int) =
    riff
        [
            chunk "fmt " (fmt 1 channels rate bytes (bytes * 8))
            chunk "data" (Array.zeroCreate (frames * channels * bytes))
        ]

let private ok (bytes: byte[]) =
    match AudioFiles.check bytes with
    | Ok info -> info
    | Error reason -> failwithf "expected a playable file, got: %s" reason

let private refused (expected: string) (bytes: byte[]) =
    match AudioFiles.check bytes with
    | Error reason -> Assert.Contains(expected, reason)
    | Ok info -> failwithf "expected %A to be refused, got %A" expected info

/// The Ogg CRC-32 of a page (checksum field zeroed), to edit a page and keep it valid.
let private oggCrc (page: byte[]) =
    let mutable crc = 0u

    for i in 0 .. page.Length - 1 do
        let value = if i >= 22 && i < 26 then 0uy else page.[i]
        crc <- crc ^^^ (uint32 value <<< 24)

        for _ in 0..7 do
            crc <-
                if crc &&& 0x80000000u <> 0u then
                    (crc <<< 1) ^^^ 0x04c11db7u
                else
                    crc <<< 1

    crc

/// Where each page of an Ogg file starts.
let private pageStarts (bytes: byte[]) =
    let mutable at = 0

    [
        while at < bytes.Length do
            yield at
            let segments = int bytes.[at + 26]

            at <-
                at
                + 27
                + segments
                + Seq.sum (seq { for k in 0 .. segments - 1 -> int bytes.[at + 27 + k] })
    ]

/// `bytes` with the page at `start` changed by `edit` and its checksum made right again.
let private rewritePage (bytes: byte[]) (start: int) (edit: byte[] -> unit) =
    let copy = Array.copy bytes
    let segments = int copy.[start + 26]

    let length =
        27
        + segments
        + Seq.sum (seq { for k in 0 .. segments - 1 -> int copy.[start + 27 + k] })

    let page = Array.sub copy start length
    edit page
    let crc = oggCrc page
    page.[22] <- byte crc
    page.[23] <- byte (crc >>> 8)
    page.[24] <- byte (crc >>> 16)
    page.[25] <- byte (crc >>> 24)
    Array.blit page 0 copy start length
    copy

[<Fact>]
let ``the player's tone and plain WAV files pass`` () =
    let info = ok (tone ())
    Assert.Equal(("ogg", 2, 22050), (info.Format, info.Channels, info.SampleRate))
    Assert.InRange(info.Seconds, 0.24, 0.26)
    let info = ok (wav 1 8000 2 4000)
    Assert.Equal(("wav", 1, 8000, 0.5), (info.Format, info.Channels, info.SampleRate, info.Seconds))
    // 8-, 24- and 32-bit PCM, 24 bits in 4 bytes, and 32-bit float.
    for bytes in [ 1; 3; 4 ] do
        ok (wav 2 44100 bytes 10) |> ignore

    ok (riff [ chunk "fmt " (fmt 1 1 8000 4 24); chunk "data" (Array.zeroCreate 8) ])
    |> ignore

    ok (riff [ chunk "fmt " (fmt 3 2 48000 4 32); chunk "data" (Array.zeroCreate 16) ])
    |> ignore
    // Chunks hound skips before the data, and WAVE_FORMAT_EXTENSIBLE with the PCM subformat.
    let extensible =
        Array.concat
            [
                fmt 0xFFFE 2 22050 2 16
                le16 22
                le16 16
                le32 3L
                [|
                    0x01uy
                    0x00uy
                    0x00uy
                    0x00uy
                    0x00uy
                    0x00uy
                    0x10uy
                    0x00uy
                    0x80uy
                    0x00uy
                    0x00uy
                    0xAAuy
                    0x00uy
                    0x38uy
                    0x9Buy
                    0x71uy
                |]
            ]

    ok (
        riff
            [
                chunk "LIST" (Array.zeroCreate 10)
                chunk "fmt " extensible
                chunk "fact" (le32 4L)
                chunk "data" (Array.zeroCreate 16)
            ]
    )
    |> ignore

[<Fact>]
let ``files that are not Ogg Vorbis or WAV are refused`` () =
    refused "empty" [||]
    refused "not an Ogg Vorbis or WAV file" [| 0x89uy; byte 'P'; byte 'N'; byte 'G'; 13uy; 10uy; 26uy; 10uy |]
    refused "not an Ogg Vorbis or WAV file" (Text.Encoding.ASCII.GetBytes "ID3\x03 an mp3")

[<Fact>]
let ``damaged Ogg files are refused`` () =
    let bytes = tone ()
    let starts = pageStarts bytes
    Assert.True(starts.Length >= 3, sprintf "%A" starts)
    // Cut inside a page (the decoder reads to the end, so any page counts), or before the
    // setup header.
    refused "cut short" bytes.[.. bytes.Length - 10]
    refused "cut short" bytes.[.. starts.[1] + 30]
    refused "cut short" bytes.[.. starts.[1] - 1]
    // One byte changed: the page's checksum no longer matches.
    let flipped = Array.copy bytes
    flipped.[bytes.Length - 5] <- flipped.[bytes.Length - 5] ^^^ 0xFFuy
    refused "checksum" flipped
    refused "after its last page" (Array.append bytes (Text.Encoding.ASCII.GetBytes "junk"))
    // Valid pages around headers that are not Vorbis.
    refused "unknown version" (rewritePage bytes 0 (fun page -> page.[4] <- 1uy))
    refused "not Vorbis" (rewritePage bytes 0 (fun page -> page.[28] <- byte 'X'))
    refused "Ogg Opus" (rewritePage bytes 0 (fun page -> Array.blit (Text.Encoding.ASCII.GetBytes "OpusHead") 0 page 28 8))
    refused "Vorbis headers are damaged" (rewritePage bytes 0 (fun page -> page.[28 + 11] <- 0uy)) // no channels
    refused "Vorbis headers are damaged" (rewritePage bytes 0 (fun page -> Array.blit (le32 0L) 0 page (28 + 12) 4))
    refused "sample rate (500 Hz)" (rewritePage bytes 0 (fun page -> Array.blit (le32 500L) 0 page (28 + 12) 4))
    refused "starts in the middle" (rewritePage bytes 0 (fun page -> page.[5] <- 0uy))
    // Longer than the player decodes: the last page's position says 11 minutes.
    let last = List.last starts
    refused "longer than 10 minutes" (rewritePage bytes last (fun page -> Array.blit (le32 (22050L * 660L)) 0 page 6 4))

[<Fact>]
let ``damaged WAV files are refused`` () =
    refused "no format chunk" (riff [])
    refused "no format chunk before its data" (riff [ chunk "data" (Array.zeroCreate 4) ])
    refused "no data chunk" (riff [ chunk "fmt " (fmt 1 1 8000 2 16) ])
    refused "compressed" (riff [ chunk "fmt " (fmt 2 1 8000 2 16); chunk "data" (Array.zeroCreate 4) ]) // ADPCM
    refused "no channels" (riff [ chunk "fmt " (fmt 1 0 8000 2 16); chunk "data" (Array.zeroCreate 4) ])
    refused "format chunk is damaged" (riff [ chunk "fmt " (fmt 1 1 8000 2 12); chunk "data" (Array.zeroCreate 4) ])
    refused "sample size" (riff [ chunk "fmt " (fmt 1 1 8000 4 16); chunk "data" (Array.zeroCreate 8) ])
    refused "data chunk is damaged" (riff [ chunk "fmt " (fmt 1 2 8000 2 16); chunk "data" (Array.zeroCreate 6) ])
    refused "no sound" (wav 1 8000 2 0)
    refused "sample rate (500 Hz)" (wav 1 500 2 10)
    let whole = wav 1 8000 2 100
    refused "cut short" whole.[.. whole.Length - 2]
    refused "longer than 10 minutes" (wav 1 1000 1 (601 * 1000))
    Assert.True(Result.isOk (AudioFiles.check (wav 1 1000 1 (600 * 1000))))

let private audioAsset (id: string) (kind: string) (url: string) =
    { CustomAsset.Default with
        Id = id
        Name = id + ".ogg"
        Type = kind
        DataUrl = url
    }

let private dataUrl (mime: string) (bytes: byte[]) =
    sprintf "data:%s;base64,%s" mime (Convert.ToBase64String bytes)

[<Fact>]
let ``Problems name sound and music files the game would skip`` () =
    let good =
        audioAsset "sound-1" CustomAssetTypes.Sound (dataUrl "audio/ogg" (tone ()))

    let wavMusic =
        audioAsset "music-1" CustomAssetTypes.Music (dataUrl "audio/wav" (wav 2 22050 2 100))

    let project =
        starter () |> apply (Batch("audio", [ UpsertAsset good; UpsertAsset wavMusic ]))

    let unreadable (project: GameProject) =
        Problems.collect project
        |> List.filter (fun p -> p.Code = "asset.audioUnreadable")

    Assert.Empty(unreadable project)

    let cut =
        audioAsset "music-2" CustomAssetTypes.Music (dataUrl "audio/ogg" (tone ()).[..999])

    let notBase64 =
        audioAsset "sound-2" CustomAssetTypes.Sound "data:audio/wav;base64,@@@@"
    // Already reported as not audio at all: one problem for it, not two.
    let png = audioAsset "sound-3" CustomAssetTypes.Sound "data:image/png;base64,AAAA"

    let broken =
        project
        |> apply (Batch("broken", [ UpsertAsset cut; UpsertAsset notBase64; UpsertAsset png ]))

    let problems = unreadable broken
    Assert.Equal<string list>([ "music-2"; "sound-2" ], problems |> List.map (fun p -> p.TargetId))
    let first = problems.Head
    Assert.True first.IsError
    Assert.Equal("asset", first.TargetKind)
    Assert.Equal(sprintf "customAssets[%d].dataUrl" (broken.CustomAssets.Length - 3), first.Path)
    Assert.Equal("Music \"music-2.ogg\" won't play: the Ogg file is cut short", first.Message)
    Assert.Contains("not a base64 data: URL", problems.[1].Message)

    Assert.Single(Problems.collect broken, fun p -> p.TargetId = "sound-3")
    |> ignore

    Assert.True(Problems.blocksExport (Problems.collect broken))
    Assert.False(Problems.blocksExport (Problems.collect project))

[<Fact>]
let ``data URLs are read as leniently as the player reads them`` () =
    let bytes = wav 1 8000 1 3
    let unpadded = (Convert.ToBase64String bytes).TrimEnd('=')
    Assert.Equal<byte[]>(bytes, (AudioFiles.dataUrlBytes ("DATA:audio/wav;BASE64," + unpadded)).Value)
    Assert.Equal<byte[]>(bytes, (AudioFiles.dataUrlBytes ("data:audio/wav;base64,\n" + Convert.ToBase64String bytes)).Value)
    Assert.True((AudioFiles.dataUrlBytes "data:audio/wav,plain").IsNone)
    Assert.True((AudioFiles.dataUrlBytes "asset:abc").IsNone)

[<Fact>]
let ``renaming an asset keeps its id and what plays it`` () =
    let sound =
        audioAsset "sound-1" CustomAssetTypes.Sound (dataUrl "audio/ogg" (tone ()))

    let project = starter () |> apply (UpsertAsset sound)

    let doc =
        Document.create project
        |> Document.apply (RenameAsset("sound-1", "  Door creak  "))

    let renamed = doc.Project.CustomAssets |> List.find (fun a -> a.Id = "sound-1")
    Assert.Equal("Door creak", renamed.Name)
    Assert.Equal({ sound with Name = "Door creak" }, renamed)
    Assert.Same(project, (Document.undo doc).Project)
    // Blank names, the same name and unknown ids change nothing.
    for edit in
        [
            RenameAsset("sound-1", "   ")
            RenameAsset("sound-1", sound.Name)
            RenameAsset("nobody", "X")
        ] do
        Assert.Same(project, project |> apply edit)
