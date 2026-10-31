// Converts a single-file project to a project folder: dotnet fsi tofolder.fsx <project.json> <folder>
#r "../../../src/FarmEngine.Cli/bin/Release/net10.0/FarmEngine.Authoring.dll"
open FarmEngine.Authoring
open System.IO
let loaded = ProjectLoad.loadProjectText (File.ReadAllText fsi.CommandLineArgs.[1])
if not loaded.Ok then
    for e in loaded.Errors do printfn "LOAD ERROR: %s" e
    exit 1
let out = fsi.CommandLineArgs.[2]
if Directory.Exists out then Directory.Delete(out, true)
for (path, bytes) in ProjectFolder.toFiles loaded.Data.Value do
    let f = Path.Combine(out, path)
    Directory.CreateDirectory(Path.GetDirectoryName f) |> ignore
    File.WriteAllBytes(f, bytes)
printfn "wrote %s" out
