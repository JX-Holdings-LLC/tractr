# Editor architecture (C# app)

The Avalonia editor lives in `src/Tractr.App`.
[ARCHITECTURE.md](ARCHITECTURE.md) defines the language boundary: **F# understands
the project, Rust runs the game, C# is the desktop app**. Within C#, views
collect input and draw controls; view models own editable state, validation,
commands and navigation. They can be tested without constructing Avalonia.

## Views and state

| Area | View | State and commands |
|---|---|---|
| Shell | `MainWindow.axaml`, `AboutWindow.axaml`, `UpdateCenterWindow.axaml`, `ExportGameWindow` | `MainWindowViewModel`, `AboutViewModel`, `UpdateCenterViewModel`, `ExportGameViewModel` |
| Map | `EditModeView`, `EditModeTools`, `EditModeFind`, `MapCanvas` | `MapEditorViewModel`: brush/layer/tool, strokes, selection, clipboard, scene and door drafts, placement, scene/door edits, navigation and Pick on map |
| Content | `ContentEditorView` | `ContentEditorViewModel`: category, entry list, selected record, commands, readouts and unsaved-draft prompts |
| Generated fields | `ContentForm`, its `ListEditor` and `VocabularyEditor` helpers | `ContentDraftViewModel`: JSON draft, field writers, dirty tracking, numeric validation, raw JSON validation and commit |
| Art | `ArtEditorView` and `ArtEditorView.Layout` | `ArtEditorViewModel`: selection, imports, animation frames/clips, bindings, removals and messages |
| Other tabs | `ProblemsView`, `SettingsEditorView`, `ModsEditorView`, `WorkshopView`, `InterfaceEditorView` | Matching classes in `ViewModels/EditMode` |
| Project persistence | `ProjectWorkspace`, `ProjectStore`, project dialogs | Immutable F# `Document`, undo/redo, background autosave and F# project edits |
| Game surface | `GameWorkspaceView`, `PlayModeView`, `DebugDrawer` | Rust simulation, renderer and runtime UI through `FarmEngine.Interop` |

The map canvas, generated content controls and art previews remain code-built:
their structure depends on the project. `ContentForm` delegates collection
rows (including schedules, waypoints and stock cards) to `ListEditor`, and
conditions/outcomes to `VocabularyEditor`. They collect control values into
the shared draft model; they do not own separate JSON or dirty state.
`ContentFormSchema` contains reflection helpers and `ContentFormLayout`
contains reusable layout pieces.

Map pointer and keyboard events supply tile coordinates to
`MapEditorViewModel`. The model performs F# edits and retains the brush across
undo/redo. It owns navigation requests and the pending Pick on map operation;
the view responds by selecting the tab/scene and focusing or scrolling the
canvas. Art import decoding and PNG conversion run on a background task,
then apply one batch on the UI context. A late import cannot modify a
replacement project or a retired view. Debounced autosave serializes and writes
an immutable project snapshot on a worker, then publishes its status on the UI
thread. Edits made during that write stay pending; explicit saves and project
changes drain the active write and save the latest document before proceeding.

## Refresh and rendering

Content list rows and thumbnail controls survive unrelated changes. A map
paint keeps an unchanged content form; a changed record, picker context or
asset refreshes the affected presentation. Hidden tabs catch up when opened.
The map defers sending a project until it is drawn and sends scene deltas for
painting, retaining the project's embedded art on the native side.

One `EditorPreview` serves the map, terrain palette, content thumbnails,
generated-form previews and art studio in an editor. Each caller supplies its
immutable project before drawing, so an older draft cannot leave another
surface on the wrong project. The editor owns its lifetime. Standalone test
views may use their own preview.

Rust builds and decorates only the editor camera's tile window, including
nearby lights and tall objects that extend into view. The native snapshot
cache includes the scene, layout and tile window; repeated frames in the
same window reuse it. Unrestricted snapshot requests retain their existing
serialization contract.

## Shared pieces

| Need | Use |
|---|---|
| Select an Edit Mode tab | `EditModeView.SelectedTab` / `MapEditorViewModel.Navigate`, using `EditorTab` |
| Resolve a Workshop or Problems link | `EditorNavigation` |
| Sizes, relative times and text-field numbers | `DisplayFormat` |
| Atomic file writes | `Tractr.Updates.AtomicFile` |
| Read or change `settings.json` | `SettingsFile` through `AppSettingsStore` / `JsonSettingsStore` |
| Read/write a picked file | `Projects/PickedFiles` |
| Ask about unsaved fields | `DraftPrompt`, shown by `DraftBar` |
| Bind a code-built command button | `Ui.CommandButton` |
| Keep draft rows and controls together | `DraftRows` |
| Accessible controls and shared layout | `Ui` and `ContentFormLayout` |
| Editor labels | `EditorStrings`; tab headers and Play Mode toolbar follow language changes |
| Colours, fonts and surfaces | `App.axaml`, `GameStyles.axaml` and [brand/APP-UI.md](../brand/APP-UI.md) |

Error messages use the visible field label, never a control's internal name.
Test doubles live in the test project: shell placeholders in `Ui/Placeholders`
and `FakeUpdateService` in `Updates/Testing`. Production always uses the real
update service; no environment variable substitutes a fake updater.

## Verification

`tests/Tractr.App.Tests/ViewModels` drives map painting, clipboard, scene and
door validation, Pick on map, content dirty tracking/JSON validation and art
commands over temporary `ProjectWorkspace`s. `EditModeViewModelRulesTests`
rejects Avalonia types in these models. Headless UI tests cover generated
controls, layout, navigation and actual rendering.

The native preview regression paints 50 tiles of a 128×128 map with 4 MB of
embedded art, verifies scene-only updates and shared-preview identity, and
compares the resulting pixels with a fresh preview. Rust tests compare
culled editor views with unrestricted snapshots at multiple camera positions.
