![Tractr, the farm game engine](media/tractr-hero.png)

# The Tractr handbook

Everything you need to plant a game, from the first project to the last export. Pick the page that matches what you
are doing.

## Making games

- [Creator guide](CREATOR-GUIDE.md): start a project, build the world in Edit Mode, playtest, and export. The editor
  shows it under **Help → Creator Guide** (F1).
- [Export Game](EXPORT.md): standalone Windows, Linux (Steam Deck too) and web games, and what goes in the box.
- [Pack plugins](PLUGINS.md): small JavaScript programs that react to what happens in a game, and the sandbox they
  run in.
- [The game player](PLAYER.md): the program inside every exported game, its settings, saves and screenshots.

## Working on Tractr

- [Contributing](../CONTRIBUTING.md): build, test, and regenerate the checked-in files.
- [Editor architecture](EDITOR-ARCHITECTURE.md): how the C# app's views fit together.
- [Languages](LANGUAGES.md): why the engine is Rust, the authoring core F# and the editor C#.
- [Native numerics](NUMERICS.md): integer arithmetic and the goldens.
- [Code conventions](CONVENTIONS.md): schema records, state, determinism rules and tests.
- [SkiaSharp and Avalonia migration](SKIA-MIGRATION.md): what draws the editor.
- [Releasing](RELEASING.md): cutting a release and how the Update Center finds it.

## The look

- [Brand](brand/README.md): the Seed Catalog logos, illustrations and Vixl build kit.
- [Style guide](brand/STYLE.md): voice, colour, type, motifs and logo use.
- [How the editor builds it](brand/APP-UI.md): theme tokens, fonts and the few places it departs from the guide.

The look is for Tractr itself: the editor, the installer and these pages. The games you make keep their own.
