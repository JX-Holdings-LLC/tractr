namespace FarmEngine.Authoring

open System.Text

/// CSV (RFC 4180) for content tables and translation files.
module Csv =
    /// A field as CSV writes it: quoted when it holds a comma, a quote, a line break or edge spaces.
    let field (text: string) =
        if text.IndexOfAny([| ','; '"'; '\n'; '\r' |]) >= 0 || text.Trim() <> text then
            "\"" + text.Replace("\"", "\"\"") + "\""
        else
            text

    /// CSV text (RFC 4180, comma separated, CRLF ends kept as written) as rows of fields.
    let parse (text: string) : Result<string list list, string> =
        let rows = ResizeArray<string list>()
        let row = ResizeArray<string>()
        let field = StringBuilder()
        let mutable quoted = false
        let mutable i = 0
        let mutable error = None
        let mutable line = 1
        let text = if text.StartsWith "﻿" then text.Substring 1 else text

        let endField () =
            row.Add(field.ToString())
            field.Clear() |> ignore

        let endRow () =
            endField ()

            if not (row.Count = 1 && row.[0] = "") then
                rows.Add(List.ofSeq row)

            row.Clear()

        while error.IsNone && i < text.Length do
            let c = text.[i]

            if quoted then
                if c = '"' then
                    if i + 1 < text.Length && text.[i + 1] = '"' then
                        field.Append '"' |> ignore
                        i <- i + 1
                    else
                        quoted <- false
                else
                    if c = '\n' then
                        line <- line + 1

                    field.Append c |> ignore
            elif c = '"' then
                if field.Length = 0 then
                    quoted <- true
                else
                    error <- Some(sprintf "line %d: a quote inside a field that does not start with one" line)
            elif c = ',' then
                endField ()
            elif c = '\n' then
                endRow ()
                line <- line + 1
            elif c = '\r' then
                ()
            else
                field.Append c |> ignore

            i <- i + 1

        if quoted then
            error <- Some(sprintf "line %d: a quoted field is not closed" line)

        match error with
        | Some e -> Error e
        | None ->
            if field.Length > 0 || row.Count > 0 then
                endRow ()

            Ok(List.ofSeq rows)
