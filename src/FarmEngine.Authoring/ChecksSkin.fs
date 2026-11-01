namespace FarmEngine.Authoring

open System
open System.Text.RegularExpressions
open FarmEngine.Schemas

/// The interface skin's checks (#236): its fonts, colours, panel art, icons, layout and title
/// screen. A part that does not check out keeps the built-in look in the game.
module internal ChecksSkin =
    let private colorPattern =
        Regex(@"^(#([0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})|rgba?\([^)]*\))$", RegexOptions.CultureInvariant)

    /// Whether `value` is a colour the game reads (`#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`).
    let isColor (value: string) = colorPattern.IsMatch(value.Trim())

    /// The character ranges of the skin's text fonts (body and heading) that read as fonts.
    let fontRanges (project: GameProject) : (string * (int * int)[]) list =
        match project.UiSkin with
        | None -> []
        | Some skin ->
            [ skin.BodyFont; skin.HeadingFont ]
            |> List.choose id
            |> List.distinct
            |> List.choose (fun id ->
                project.CustomAssets
                |> List.tryFind (fun a -> a.Id = id)
                |> Option.bind (fun asset ->
                    AudioFiles.dataUrlBytes asset.DataUrl
                    |> Option.bind (fun bytes ->
                        match FontFile.coverage bytes with
                        | Ok ranges -> Some(asset.Name, ranges)
                        | Error _ -> None)))

    let run (project: GameProject) (sink: Sink) =
        match project.UiSkin with
        | None -> ()
        | Some skin ->
            let target = Some NavigationTarget.Interface
            let error code path message = sink.Error("skin." + code, "uiSkin." + path, message, target)
            let warning code path message = sink.Warning("skin." + code, "uiSkin." + path, message, target)

            let asset (id: string) =
                project.CustomAssets |> List.tryFind (fun a -> a.Id = id)

            let checkVisual (path: string) (what: string) (visual: VisualRef) =
                match asset visual.AssetId with
                | None -> error "missingArt" path (sprintf "The %s uses art '%s', which the game doesn't have." what visual.AssetId)
                | Some a when a.Type = CustomAssetTypes.Font || a.Type = CustomAssetTypes.Sound || a.Type = CustomAssetTypes.Music ->
                    error "notArt" path (sprintf "The %s uses '%s', which is not an image." what a.Name)
                | Some _ -> ()

            for path, id in
                [
                    "headingFont", skin.HeadingFont
                    "bodyFont", skin.BodyFont
                    "numberFont", skin.NumberFont
                ] do
                match id with
                | None -> ()
                | Some id ->
                    match asset id with
                    | None -> error "missingFont" path (sprintf "The font '%s' is not in the game's art." id)
                    | Some font when font.Type <> CustomAssetTypes.Font ->
                        error "notFont" path (sprintf "'%s' is not a font: import a .ttf or .otf file." font.Name)
                    | Some font ->
                        match AudioFiles.dataUrlBytes font.DataUrl |> Option.map FontFile.coverage with
                        | None -> error "badFont" path (sprintf "The font '%s' has no file." font.Name)
                        | Some(Error why) -> error "badFont" path (sprintf "The font '%s' can't be used: %s." font.Name why)
                        | Some(Ok ranges) ->
                            if path = "numberFont" then
                                match FontFile.firstMissing ranges "0123456789" with
                                | Some rune ->
                                    warning "fontCoverage" path (sprintf "The number font '%s' has no %s." font.Name (rune.ToString()))
                                | None -> ()

            for role, value in defaultArg skin.Colors [] do
                if not (List.contains role UiPaletteRoles.All) then
                    warning "unknownColor" ("colors." + role) (sprintf "'%s' is not a colour of the interface." role)
                elif not (isColor value) then
                    error "badColor" ("colors." + role) (sprintf "'%s' is not a colour: use #rrggbb or #rrggbbaa." value)

            for part, slice in defaultArg skin.Panels [] do
                let path = "panels." + part

                if not (List.contains part UiSkinParts.All) then
                    warning "unknownPart" path (sprintf "'%s' is not a part of the interface." part)

                checkVisual path (sprintf "%s art" part) slice.Visual

                if slice.Left < 0.0 || slice.Top < 0.0 || slice.Right < 0.0 || slice.Bottom < 0.0 then
                    error "badSlice" path "Panel art edges can't be negative."

                match slice.Scale with
                | Some s when s < 1.0 || s > 8.0 -> error "badScale" path "The panel art's scale is 1 to 8."
                | _ -> ()

                let size =
                    match slice.Visual.Frame with
                    | Some frame -> Some(frame.Width, frame.Height)
                    | None ->
                        asset slice.Visual.AssetId
                        |> Option.bind (fun a ->
                            match a.Width, a.Height with
                            | Some w, Some h -> Some(w, h)
                            | _ -> None)

                match size with
                | Some(w, h) when slice.Left + slice.Right > w || slice.Top + slice.Bottom > h ->
                    warning "sliceEdges" path (sprintf "The edges are wider than the %g×%g art; the middle won't show." w h)
                | _ -> ()

            for name, visual in defaultArg skin.Icons [] do
                if not (List.contains name UiSkinIcons.All) then
                    warning "unknownIcon" ("icons." + name) (sprintf "'%s' is not an icon of the interface." name)

                checkVisual ("icons." + name) (sprintf "%s icon" name) visual

            match skin.Layout with
            | None -> ()
            | Some layout ->
                let oneOf path (value: string option) (all: string list) =
                    match value with
                    | Some v when not (List.contains v all) ->
                        error "badLayout" ("layout." + path) (sprintf "'%s' is not one of %s." v (String.Join(", ", all)))
                    | _ -> ()

                oneOf "hudPosition" layout.HudPosition UiHudPositions.All
                oneOf "clockStyle" layout.ClockStyle UiClockStyles.All
                oneOf "dialoguePosition" layout.DialoguePosition UiDialoguePositions.All

                match layout.PortraitSize with
                | Some size when size < 32.0 || size > 160.0 || Math.Floor size <> size ->
                    error "badLayout" "layout.portraitSize" "The portrait size is a whole number from 32 to 160."
                | _ -> ()

            match skin.Title with
            | None -> ()
            | Some title ->
                title.Background |> Option.iter (checkVisual "title.background" "title background")
                title.Logo |> Option.iter (checkVisual "title.logo" "title logo")

                match title.Scene with
                | Some scene when not (project.Scenes |> List.exists (fun s -> s.Id = scene)) ->
                    error "titleScene" "title.scene" (sprintf "The title screen shows scene '%s', which the game doesn't have." scene)
                | _ -> ()

                match title.MenuPosition with
                | Some v when not (List.contains v UiMenuPositions.All) ->
                    error "badLayout" "title.menuPosition" (sprintf "'%s' is not one of %s." v (String.Join(", ", UiMenuPositions.All)))
                | _ -> ()
