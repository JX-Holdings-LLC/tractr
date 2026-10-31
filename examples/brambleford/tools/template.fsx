// Writes a Tractr sample template as a single-file project: dotnet fsi template.fsx <template> <out.json>
#r "../../../src/FarmEngine.Cli/bin/Release/net10.0/FarmEngine.Authoring.dll"
open FarmEngine.Authoring
open System.IO
let project = ProjectCatalog.CreateProjectForTemplate(fsi.CommandLineArgs.[1], 1761900000000.0)
let files = readOnlyDict (ProjectFolder.toFiles project)
match ProjectFolder.joinText files with
| Ok text -> File.WriteAllText(fsi.CommandLineArgs.[2], text)
| Error errors -> failwithf "%A" errors
