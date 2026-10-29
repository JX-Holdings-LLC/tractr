namespace FarmEngine.Authoring

open FarmEngine.Schemas

/// Additive gameplay features use version 3. Classic projects retain their version-2 bytes,
/// while saving or compiling a project using a new feature prevents older editors dropping it.
module ProjectVersion =
    let private featureFields =
        Set.ofList
            [
                "weekdays"
                "weekdayOffset"
                "schedules"
                "forageSpawns"
                "fishingGear"
                "fishProduction"
                "layered"
                "buffs"
                "skillDefinitions"
                "progression"
                "deadlineDays"
                "deadlineDay"
                "onFail"
                "targetMonsterId"
                "targetBundleId"
                "targetMineId"
                "mines"
                "requestBoards"
                "keeperNpcId"
                "closedOn"
                "priceModifiers"
                "upgradesTools"
                "toolUpgrades"
                "perks"
                "hours"
                "saturation"
                "upgrade"
                "filter"
                "rules"
                "mineId"
                "program"
                "purchasedUpgrades"
                "questDeadlines"
                "fishCatches"
                "shopSales"
                "festivalVisits"
                "currentMineId"
            ]

    let private featureTypes =
        Set.ofList
            [
                "weekday"
                "buff"
                "perk"
                "applyBuff"
                "setInventorySize"
                "addInventorySize"
                "addMaxEnergy"
                "addMaxHealth"
            ]

    let rec private hasFeatures json =
        match json with
        | JObject fields ->
            let keys = fields |> List.map fst |> Set.ofList
            // Seasons existed on crops and shops in v2; they are new only on nodes/drops.
            (keys.Contains "seasons"
             && (keys.Contains "drops" || (keys.Contains "min" && keys.Contains "weight")))
            || (keys.Contains "maxHealth" && keys.Contains "maxInventorySize")
            || (keys.Contains "progress"
                && (Json.asString (Json.get "type" json)
                    |> Option.exists (fun kind -> List.contains kind [ "mineFloor"; "hearts"; "bundle"; "ship"; "slay"; "catch" ])))
            || (fields
                |> List.exists (fun (key, value) ->
                    (featureFields.Contains key && value <> JNull)
                    || (key = "skills"
                        && (match value with
                            | JArray(_ :: _) -> true
                            | _ -> false))
                    || (key = "type" && (Json.asString value |> Option.exists featureTypes.Contains))
                    || hasFeatures value))
        | JArray items -> List.exists hasFeatures items
        | _ -> false

    /// Upgrade only when necessary; never downgrade a project that already uses version 3.
    let forSave json =
        match json with
        | JObject fields when hasFeatures json ->
            JObject(
                fields
                |> List.map (fun (key, value) ->
                    if key = "schemaVersion" then
                        let version =
                            match value with
                            | JNumber version -> version
                            | _ -> 0.0

                        key, JNumber(max version ProjectSchema.CurrentProjectSchemaVersion)
                    else
                        key, value)
            )
        | _ -> json

    /// The cartridge header must use the same feature version as a saved project.
    let required (project: GameProject) =
        let json = SchemaJson.encodeGameProject project

        if hasFeatures json then
            max project.SchemaVersion ProjectSchema.CurrentProjectSchemaVersion
        else
            project.SchemaVersion
