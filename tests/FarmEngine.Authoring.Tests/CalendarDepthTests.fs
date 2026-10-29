module FarmEngine.Authoring.Tests.CalendarDepthTests

open Xunit
open FarmEngine.Authoring
open FarmEngine.Authoring.Tests.TestProjects
open FarmEngine.Schemas

[<Fact>]
let ``calendar routines and shop rules roundtrip and compile`` () =
    let project = starter ()

    let calendar =
        { project.Settings.Calendar with
            Weekdays = Some(SettingsSchema.ClassicCalendarWeekdays())
            WeekdayOffset = Some 2.0
        }

    let condition = EventCondition.Weekday { WeekdayIds = [ "wednesday" ] }

    let variant =
        { NpcScheduleVariant.Default with
            Id = "weekly"
            Name = "Weekly"
            Conditions = [ condition ]
            Stops =
                [
                    { NpcScheduleEntry.Default with
                        Minute = 540.0
                        SceneId = "scene-farm"
                        X = 3.0
                        Y = 3.0
                    }
                ]
        }

    let shop =
        { ShopDefinition.Default with
            Id = "weekly-shop"
            Hours = Some { Open = 540.0; Close = 1020.0 }
            ClosedOn =
                Some
                    { ShopClosedOn.Default with
                        Weekdays = [ "sunday" ]
                    }
            PriceModifiers =
                Some
                    [
                        { ShopPriceModifier.Default with
                            Conditions = [ condition ]
                            BuyMultiplier = Some 0.8
                        }
                    ]
            Saturation = Some ShopSaturation.Default
        }

    let project =
        { project with
            Settings =
                { project.Settings with
                    Calendar = calendar
                }
            Npcs =
                project.Npcs
                |> List.map (fun npc ->
                    { npc with
                        Schedules = Some [ variant ]
                    })
            Shops = shop :: project.Shops
        }

    match Decode.run SchemaJson.decodeGameProject (SchemaJson.encodeGameProject project) with
    | Error error -> failwith error
    | Ok restored ->
        Assert.Equal(calendar, restored.Settings.Calendar)
        Assert.Equal<Npc list>(project.Npcs, restored.Npcs)
        Assert.Equal<ShopDefinition list>(project.Shops, restored.Shops)

    let content = ContentCompiler.compile project
    Assert.Equal(calendar, content.Settings.Calendar)
    Assert.Equal(Some "Wednesday", SettingsSchema.weekdayName calendar 1.0 "en")
    Assert.Contains("weekday", Vocabulary.conditionTypes |> List.map fst)

[<Fact>]
let ``new calendar fields stay absent in old projects`` () =
    let calendar =
        SchemaJson.encodeCalendarConfig SettingsSchema.DefaultProjectSettings.Calendar

    Assert.Equal(None, Json.tryGet "weekdays" calendar)
    Assert.Equal(None, Json.tryGet "weekdayOffset" calendar)
    Assert.Equal(None, Json.tryGet "schedules" (SchemaJson.encodeNpc Npc.Default))
    Assert.Equal(None, Json.tryGet "program" (SchemaJson.encodeCalendarFestival CalendarFestival.Default))
    Assert.Equal(None, Json.tryGet "hours" (SchemaJson.encodeShopDefinition ShopDefinition.Default))

[<Fact>]
let ``new projects start with a classic seven day week`` () =
    for project in [ StarterContent.initial 0.0; StarterContent.blank 0.0 ] do
        Assert.Equal(ProjectSchema.CurrentProjectSchemaVersion, project.SchemaVersion)
        Assert.Equal(Some(SettingsSchema.ClassicCalendarWeekdays()), project.Settings.Calendar.Weekdays)
        Assert.Equal(Some "Monday", SettingsSchema.weekdayName project.Settings.Calendar 1.0 "en")
        Assert.Equal(Some "Monday", SettingsSchema.weekdayName project.Settings.Calendar 8.0 "en")

[<Fact>]
let ``Problems checks unknown weekdays duplicate routes and festival times`` () =
    let project = starter ()

    let variant =
        { NpcScheduleVariant.Default with
            Id = "route"
            Conditions = [ EventCondition.Weekday { WeekdayIds = [ "missing-day" ] } ]
        }

    let festival =
        { CalendarFestival.Default with
            Id = "fair"
            Name = "Fair"
            SeasonId = "spring"
            Day = 1.0
            Program =
                Some
                    { FestivalProgram.Default with
                        SceneId = "missing"
                        EntranceSceneId = "scene-farm"
                    }
        }

    let project =
        { project with
            Npcs =
                project.Npcs
                |> List.map (fun npc ->
                    { npc with
                        Schedules = Some [ variant; variant ]
                    })
            Settings =
                { project.Settings with
                    Calendar =
                        { project.Settings.Calendar with
                            Festivals = [ festival ]
                        }
                }
        }

    let codes = Problems.collect project |> List.map (fun problem -> problem.Code)
    Assert.Contains("npc.conditionUnknownWeekday", codes)
    Assert.Contains("npc.scheduleId", codes)
    Assert.Contains("festival.hours", codes)
    Assert.Contains("festival.unknownScene", codes)

[<Fact>]
let ``Keep changes retains festival shop demand and daily route selection`` () =
    let parse text =
        match Json.parse text with
        | Ok value -> value
        | Error error -> failwith error

    let project = SchemaJson.encodeGameProject (starter ())

    let state =
        parse
            """{"npcs":{"npc-farmer":{"scheduleSelection":{"day":2,"weather":"rain","variant":"rainy"}}},"festival":{"festivalId":"fair"},"festivalVisits":{"fair":{"day":2,"activityDone":true}},"shopSales":{"shop":{"crop":{"2":20}}}}"""

    let kept = Playtest.applyStateJson project state |> Json.get "keptState"

    for key in [ "festival"; "festivalVisits"; "shopSales" ] do
        Assert.Equal(Json.get key state, Json.get key kept)

    Assert.Equal(
        JString "rainy",
        Json.get "npcs" kept
        |> Json.get "npc-farmer"
        |> Json.get "scheduleSelection"
        |> Json.get "variant"
    )
