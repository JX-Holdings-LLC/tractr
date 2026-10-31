// Lists the Problems (errors, warnings, tips) of a project folder: dotnet fsi check.fsx <folder>
#r "../../../src/FarmEngine.Cli/bin/Release/net10.0/FarmEngine.Authoring.dll"
open FarmEngine.Authoring
open System.IO
let folder = fsi.CommandLineArgs.[1]
let files = System.Collections.Generic.Dictionary<string, byte[]>()
for f in Directory.EnumerateFiles(folder, "*", SearchOption.AllDirectories) do
    let rel = Path.GetRelativePath(folder, f).Replace('\\','/')
    if ProjectFolder.isProjectFile rel then files.[rel] <- File.ReadAllBytes f
let loaded = ProjectFolder.load files
if not loaded.Ok then
    for e in loaded.Errors do printfn "LOAD ERROR: %s" e
else
    let p = loaded.Data.Value
    for pr in Problems.collect p do printfn "%s %s | %s | %s" pr.SeverityName pr.Code pr.Path pr.Message
