# How the editor wears the Seed Catalog look

[STYLE.md](STYLE.md) says what Tractr should look like. This page says where that lives in the app, so a new view
picks it up for free and an exception is written down rather than discovered.

**Scope.** The look applies to the editor's own interface: windows, menus, tabs, forms, dialogs and the Update
Center. It does not apply to the game. The Play Mode surface, the in-game HUD, dialogue, shops and toasts (drawn by
the Rust player), the map's tile colours and sprites, and the default icon of exported games are the game's own and
stay as they were. A creator's game must never change because the editor changed.

## Where things are

| What | Where |
| --- | --- |
| Inks, roles, status tints, fonts, Fluent control overrides, buttons, tabs, lists, Markdown | `src/Tractr.App/App.axaml` |
| Panels, chips, toasts, modals, the map frame, form groups, tool buttons | `src/Tractr.App/Game/GameStyles.axaml` |
| The four fonts (OFL) | `src/Tractr.App/Assets/Fonts` |
| App icon, header logo, About banner, paper grain | `src/Tractr.App/Assets` (made by `scripts/generate-app-icon.py` and `docs/brand/vixl/kit/build_marketing.py`) |
| Capital letters for labels | `src/Tractr.App/Controls/KickerText.cs` |

## Colour tokens

The views use role names (the older `Farm*` keys) so they never name an ink. Use a role, not a hex value.

| Resource | Ink | Use |
| --- | --- | --- |
| `FarmBackgroundBrush` | Paper `#F2E5C6` | Window canvas (with the paper grain: `PaperGrainBrush`) |
| `FarmSurfaceBrush` | Light paper `#F7EED6` | Cards, headers, dialogs, inputs sit on `#FBF5E3` |
| `FarmCardBrush` | Sand `#E9DAB6` | Side panels, the menu bar, the status bar |
| `FarmForegroundBrush` | Ink `#3A2A1E` | Text (11:1 on Paper) |
| `FarmMutedForegroundBrush` | Saddle `#6D5A40` | Secondary and small text (5.3:1) |
| `FarmBorderBrush` | Kraft `#C9B48A` | Hairlines and dividers |
| `FarmPrimaryBrush` | Barn Red `#B5332A` | The one primary action, "you are here" (selected tab, headings), links |
| `FarmSecondaryBrush`, `FieldGreenBrush` | Field Green `#4F6B3A` | Confirmation, success, secondary actions (Play Mode), checked boxes, progress |
| `FarmAccentBrush`, `MustardBrush` | Mustard `#E3A83A` | Selection and highlight, always with Ink text |
| `FarmDestructiveBrush` | Hub Red `#8F2A22` | Errors |
| `FarmSuccessTintBrush`, `FarmUpdateTintBrush`, `FarmWarningTintBrush`, `FarmErrorTintBrush`, `FarmInfoTintBrush` | flat washes of green, sun, wheat, Hub Red and Pond | Status cards, toasts and notices, each with its own border brush |

Rules that follow from the style guide:

- No pure white or black, no gradients, no soft shadows. Depth is a Kraft hairline, a heavier rule or a flat shape.
  Modals and the map frame use a 2 px Barn Red rule, the header a 3 px one.
- Never colour alone: every status carries its own icon or a word.
- Mustard is for selection only. A selected tab is Barn Red text under a Barn Red rule; a selected list row or tool
  is Mustard with Ink text.
- Hover lifts a control to a pale sun wash (`#F6E7BC`) and its border turns Barn Red.

## Type

| Voice | Font | Where |
| --- | --- | --- |
| Display | Alfa Slab One | Screen titles (`TextBlock.h1`), big numbers (`stat-value`), the money chip, Markdown `#` headings |
| Script | Lobster | Rare, celebratory (the tagline in About). Never navigation (`TextBlock.script`) |
| Label | Oswald 700 | Buttons, toggle buttons, tabs, `h2`/`h3`, `section` and `category` labels, quantity chips |
| Body | Courier Prime | Everything else: forms, values, IDs, file paths, code, menus |

`h2`, `h3`, `section` and `category` text is drawn in capitals by `KickerText`, a `TextBlock` that upper-cases only
what it draws: `Text` keeps its real casing for screen readers, tests and bindings. `Ui.Text(...)` creates one
automatically for those classes; in XAML write `<controls:KickerText Classes="h2" .../>`.

Inter stays registered behind all four fonts, so a glyph they lack (other scripts, symbols) still draws.

### Where the editor departs from STYLE.md

1. **Buttons and tabs are Oswald in the text's own case, not forced capitals.** Their labels come from the
   translated strings (English and Spanish) and are read by screen readers and tests, so they are not upper-cased.
   Section labels and headings, which the editor controls, are in capitals.
2. **Errors use Hub Red, not Barn Red.** The guide asks that errors and primary actions not share a colour in one
   view. Hub Red (the tractor's darker red, already in `swatches.json`) is clearly deeper, and every error also has an
   icon or a label.
3. **Menus use the body font,** not Oswald, so the native menu bar keeps its familiar rhythm.
4. **No night-barn theme.** The guide does not define one, so the editor is light only (`RequestedThemeVariant`).
5. **The paper grain is on the window canvas only,** at about 3% strength, never under text, logos or small icons.

## Making a new view

1. Reuse a class (`card`, `side-panel`, `row`, `notice`, `status`, `chip`, `modal`, `toast`) before inventing one.
2. Take colours from the role brushes above and fonts from `DisplayFont`, `ScriptFont`, `LabelFont` and `BodyFont`.
3. One primary (Barn Red) button per view, Field Green for the other main action, plain buttons for the rest.
4. Check it in a screenshot (`FRM_SCREENSHOT_DIR`) beside [style-board.png](style-board.png), and add it to
   `ScreenshotTests` if it is a window that should not drift.
5. After an intended change of look, re-record the editor references with `FARM_EDITOR_BLESS=1` and review the PNGs
   before committing them ([CONTRIBUTING.md](../../CONTRIBUTING.md#updating-test-fixtures)).
