namespace FarmEngine.Authoring

open System
open System.IO
open System.IO.Compression

/// A minimal PNG encoder for pixels the authoring core makes itself (the frames of an imported
/// `.aseprite` file): 8-bit RGBA, no filtering, one zlib stream. Nothing here decodes images.
[<CompilationRepresentation(CompilationRepresentationFlags.ModuleSuffix)>]
module Png =
    let private crcTable =
        Array.init 256 (fun n ->
            let mutable c = uint32 n

            for _ in 0..7 do
                c <-
                    if c &&& 1u <> 0u then
                        0xEDB88320u ^^^ (c >>> 1)
                    else
                        c >>> 1

            c)

    /// The CRC-32 of `bytes` (the PNG and zlib polynomial).
    let crc32 (bytes: byte[]) : uint32 =
        let mutable c = 0xFFFFFFFFu

        for b in bytes do
            c <- crcTable.[int ((c ^^^ uint32 b) &&& 0xFFu)] ^^^ (c >>> 8)

        c ^^^ 0xFFFFFFFFu

    let private bigEndian (value: uint32) =
        [| byte (value >>> 24); byte (value >>> 16); byte (value >>> 8); byte value |]

    let private chunk (out: MemoryStream) (kind: string) (data: byte[]) =
        let tag = Text.Encoding.ASCII.GetBytes kind
        out.Write(bigEndian (uint32 data.Length), 0, 4)
        out.Write(tag, 0, 4)
        out.Write(data, 0, data.Length)
        out.Write(bigEndian (crc32 (Array.append tag data)), 0, 4)

    /// A PNG of `width`×`height` straight-alpha RGBA pixels (row-major, 4 bytes each).
    let encodeRgba (width: int) (height: int) (pixels: byte[]) : byte[] =
        if width <= 0 || height <= 0 || pixels.Length <> width * height * 4 then
            invalidArg "pixels" "The pixels must be width × height × 4 bytes."

        use out = new MemoryStream()
        out.Write([| 0x89uy; 0x50uy; 0x4Euy; 0x47uy; 0x0Duy; 0x0Auy; 0x1Auy; 0x0Auy |], 0, 8)

        let header =
            Array.concat
                [
                    bigEndian (uint32 width)
                    bigEndian (uint32 height)
                    [| 8uy; 6uy; 0uy; 0uy; 0uy |]
                ]

        chunk out "IHDR" header
        use raw = new MemoryStream()

        do
            use z = new ZLibStream(raw, CompressionLevel.Optimal, true)
            let stride = width * 4

            for y in 0 .. height - 1 do
                z.WriteByte 0uy
                z.Write(pixels, y * stride, stride)

        chunk out "IDAT" (raw.ToArray())
        chunk out "IEND" [||]
        out.ToArray()

    /// `bytes` as a `data:image/png;base64,` URL.
    let dataUrl (bytes: byte[]) : string =
        "data:image/png;base64," + Convert.ToBase64String bytes
