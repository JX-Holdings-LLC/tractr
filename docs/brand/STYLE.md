# Tractr style guide: Seed Catalog

Tractr is a farm game engine. Its look borrows from 1950s seed packets, farm-supply catalogs and feed-store
signs: warm paper, a few flat printing inks, sunburst rays, ribbon banners, scalloped seals and a cheerful red
tractor. It should feel hand-sold and sincere, like a catalog page pinned to a barn wall, but stay clean enough
for software.

`style-board.png` shows the whole system on one page.

---

## 1. Personality and voice

- **Warm, plain-spoken and optimistic.** Talk like a friendly shopkeeper, not a tech startup. Short sentences
  and concrete farm words: *grow, plant, sow, harvest, yield, season, barn, field, crop, critter.*
- **A small wink, never a joke-a-minute.** One light line per piece is enough ("Tractor not included. It is,
  actually.").
- **Headlines sell the feeling, body text gives the facts.** "Grow your own worlds!" up top; "Plant crops · Raise
  critters · Ship to every platform" underneath. Technical details go in body copy, not headlines.
- **Exclamation marks only in the script voice** (section 3), and at most one per piece.
- Catalog details are welcome as flavour: "EST. 2026", "SEED No. 01", "No. 1 Quality", "Free & Open Source".

## 2. Colour

Flat inks on paper. Every colour should look like it could have been printed with one plate.

| Role | Name | Hex | Use |
| --- | --- | --- | --- |
| background | Paper | `#F2E5C6` | The default ground for everything. 50–60% of most pieces. |
| surface | Light paper | `#F7EED6` | Cards, panels and insets that sit on Paper. |
| accent | Barn Red | `#B5332A` | The hero colour: logo ribbon, headlines, primary actions, frames. About 15%. |
| | Field Green | `#4F6B3A` | Fields, seals, positive or confirmed states, secondary actions. |
| | Sprout | `#7D9440` | Lighter foliage, back hills, a second green beside Field Green. |
| | Mustard | `#E3A83A` | Sun, rays, wheel rims, highlights and selection. Large shapes only. |
| | Sun | `#F0C35E` | The sun disc, and soft highlight fills. |
| | Wheat / Stalk | `#C58F2C` / `#A8772A` | Grain heads and stems. |
| | Shadow Red | `#6E1D17` | Hard offset shadow under red-and-cream display type. Nothing else. |
| ink | Ink | `#3A2A1E` | Body text, outlines of dark machinery, tyres. Never pure black. |
| muted | Saddle | `#6D5A40` | Secondary and small text on Paper (5.3:1). |
| | Kraft | `#C9B48A` | Hairline borders and dividers on Paper. |
| | Soil / Furrow | `#9B6B42` / `#3D5530` | Tilled ground; crop-row lines on green. |
| | Pond | `#7FA6A6` | Water, cool accents. Use sparingly; this palette runs warm. |

Rules:

- **No pure white, no pure black.** Paper is the lightest colour and Ink the darkest.
- **No gradients**, except the ray-and-halftone sunburst. Depth comes from overlapping flat shapes.
- **Text contrast:** Ink on Paper (11:1) for reading. Barn Red and Field Green on Paper (4.8:1) are fine for
  text. Paper on Red or Green (4.8:1) works for labels and buttons. Saddle is the smallest-text grey.
  Mustard is never a text colour on Paper (1.7:1). Ink on Mustard (6.5:1) is fine.
- Red text on Mustard (2.9:1) is reserved for the logo's "T" seal, where it is large and has a shadow.
- Use only one strong colour per area: a red block next to a green block needs Paper or Ink between them.

## 3. Typography

Four voices, each with one job. All are free Google Fonts.

| Voice | Font | Use |
| --- | --- | --- |
| Display | **Alfa Slab One** | The wordmark, headlines and big numbers. Usually caps, tracked a little (+2 to +4% of size). |
| Script | **Lobster** | Short exclamations and taglines ("Grow your own worlds!"). Never caps, never more than one line of about six words. |
| Label | **Oswald 700** | Kickers, section labels, buttons, tags. Always caps, widely tracked (+15 to +30% of size). |
| Body | **Courier Prime** | Body copy, data, values, file names and code. The typewritten catalog voice. |

Rules:

- **Hierarchy goes kicker, script line, display, body:** a small tracked Oswald label above a Lobster or Alfa Slab
  headline, with Courier underneath.
- **Hard offset shadow on display type over colour:** Shadow Red, offset down-right by about 3–4% of the font
  size, with **zero blur**. Never a soft drop shadow.
- Don't set long text in Alfa Slab or Lobster, and don't mix more than three of the four voices in one small area.
- Numbers that matter (prices, days, counts) look best in Courier for data, or Alfa Slab when they are the
  headline.

## 4. Graphic motifs

The motif kit lives in `illustrations/` and in the Vixl build kit (`vixl/kit/art.py`). Reuse it before
inventing new pieces.

- **Sunburst.** Thin Mustard rays from a point behind the hero, at about 55% opacity with a halftone texture, plus
  a solid Sun disc. It marks the most important thing on the page.
- **Ribbon banner.** The Barn Red forked ribbon carries the wordmark. It can also carry a single important word
  or headline; one ribbon per piece.
- **Seal badge.** A scalloped Field Green seal with a Paper ring inside, holding two or three words of Oswald
  caps or a single Alfa Slab letter. On posters it is the only element that tilts (8–15° clockwise), placed near
  an upper corner like a price sticker.
- **Double-rule frame.** A heavy Barn Red outer rule and a thin inner rule a short gap inside it. It makes any
  rectangle feel like a printed page.
- **Rolling hills and crop rows.** Overlapping half-ellipse hills in Sprout and Field Green, with curved Furrow
  lines that converge toward the horizon.
- **Wheat stalks.** Thin stems with almond-shaped grains, set in loose groups of two or three that lean slightly.
  They flank the hero and are never centred alone.
- **The red tractor.** A side-view open-seat tractor: Barn Red body with a Paper stripe, Ink tyres with lugs,
  concentric Mustard and red wheel rims, a tall exhaust stack and a Paper headlamp. It faces right (forward). It
  is the mascot and appears whole, never cropped.
- **Gears appear as lugged tyres and seal edges**, not as standalone cogs. The machine is friendly, not
  industrial.

## 5. Illustration rules

- **Geometric and flat:** build things from circles, rounded rectangles, arcs and simple paths. Three or four
  colours per object.
- **No outlines** on illustrated objects; edges come from colour changes. Thin hairlines (Kraft) are for layout
  only.
- **Flat side views and simple top-down views.** No perspective tricks beyond converging crop rows.
- Light is implied by flat highlight stripes (the tractor's hood stripe), not by shading.
- **Big and confident:** the hero object fills 50–75% of the width of a poster.
- **Texture is a finish, not a style.** Use a subtle paper grain on large background fields and halftone on the
  sunburst. Never put texture on text, logos or small icons.

## 6. Composition

- **Centred, symmetrical and poster-like by default.** A typical stack runs from a small kicker and the script
  headline at the top, through the hero illustration with the sunburst behind it and the ribbon wordmark, to a
  descriptor and small print in Courier at the bottom.
- **Frame it:** most marketing pieces sit inside the double-rule frame, with generous Paper margins (at least 4% of
  the short side).
- **A Paper "packet panel":** the lower part of a piece can be a flat Paper band that holds the text, like the
  label area of a seed packet. Scenery may run behind its top edge.
- Scenery (hills, rays, rows) may bleed off the edges. Text, logos, seals and the tractor stay inside the frame.
- One focal point per piece: one sunburst, one ribbon, one seal.

## 7. Interface guidelines (principles, not a layout)

The editor concept in `exploration/` is a mood study, not a spec. The real editor carries the style with these
principles ([APP-UI.md](APP-UI.md) says how each one is built):

- **Paper is the canvas.** Panels and cards are Light paper or a slightly darker sand on Paper, separated by
  Kraft hairlines or soft rounded corners (around 8–12 px). Interfaces stay calmer than posters: keep texture very
  subtle or leave it off where text is dense.
- **Colour means something:** Barn Red is the single primary action and "you are here"; Field Green is
  confirmation, success and secondary actions; Mustard is highlight and selection (always with Ink text); Pond is
  information. Don't use red for errors and primary actions in the same view; pair errors with an icon or label.
- **Type roles carry over:** Oswald caps for section labels, tabs and buttons; Courier for values, IDs, file
  paths and code; Alfa Slab for screen titles and big numbers; Lobster only for a rare celebratory moment (a
  harvest complete, an empty state), never for navigation.
- **Icons are filled silhouettes** (house, gear, seed, drop, sun) in palette colours, with rounded corners and no
  thin line icons. Use the same drawing rules as the illustrations.
- **Catalog flavour, lightly:** dials that look like tractor gauges, ruled ledger lines, seal-shaped badges for
  status and ribbon-shaped tags are welcome as accents, not as structure.
- **Accessibility first:** the contrast pairs in section 2 hold in the UI too; don't rely on colour alone; keep
  small text in Ink or Saddle.
- A dark ("night barn") theme is not defined yet. Don't improvise one.

## 8. Logo

Files are in `logo/` as PNG at 1× and 2× plus SVG. The SVG lettering is converted to outlines, so no fonts are
needed.

| File | Use on |
| --- | --- |
| `tractr-logo-primary` | Paper and other light backgrounds. The default. |
| `tractr-logo-primary-tagline` | The same, where there is room for the tagline. |
| `tractr-logo-reverse` (+ `-tagline`) | Barn Red, Field Green or Ink backgrounds. |
| `tractr-logo-ink` | One-colour printing, stamps and embossing. |
| `tractr-icon-seal` | The "T" seal: avatars, favicons, stickers, anywhere the ribbon would be under about 160 px wide. |
| `tractr-app-icon` + `web-icons/` | App and site icons. |

- **Clear space:** at least half the ribbon's height on every side.
- **Minimum size:** ribbon 160 px (30 mm) wide; below that, use the seal icon (works down to 16 px).
- Don't recolour beyond the three versions, stretch, rotate, outline, add soft shadows or effects, set the
  wordmark in another font, or place the primary logo on a busy part of an illustration (use a Paper band).
- The primary logo's hard Shadow Red offset is part of the mark; the reverse and ink versions have none.

## 9. Do and don't

**Do:** flat inks on Paper · hard offset shadows · one sunburst, ribbon and seal per piece · centred and framed
compositions · friendly catalog copy · the red tractor facing right.

**Don't:** pure white or black · soft blur shadows · gradients (other than the sunburst) · neon or cool-blue-led
palettes · thin outline icons · stock photography · more than three type voices in one block · chrome, rust or
heavy industrial grit (that was a different direction).

---

## 10. For agents working in Vixl

Everything here was made with Vixl and is editable. To make new assets that match:

1. **Start in a workspace that contains `brand.json`.** Copy it to the root of your Vixl workspace (or work inside
   this folder). New documents then get the Paper background and the brand typography:
   - text `stage` names map to the voices: `display`/`h1` → Alfa Slab One, `h2`/`h3`/`label` → Oswald caps,
     `subtitle`/`citation` → Lobster, `body`/`lead`/`caption` → Courier Prime;
   - colour roles are `@background`, `@surface`, `@ink`, `@muted`, `@accent` (Barn Red) and `@on-accent`.
2. **Apply `swatches.json` to each new document** (`vixl -p NEW.vixl apply swatches.json`) for the other named
   colours (`@field-green`, `@mustard`, `@wheat`, `@shadow-red`, ...). They aren't in `brand.json` because
   Vixl 0.28 crashes on `palette.extra`, `fonts.allowed` and fonts given only by `family` when it creates a
   document (`vixl brand validate` accepts them). The brand check will therefore report these swatch colours as
   off-palette warnings; that's expected.
3. **Start from the nearest build script** in `vixl/kit/` and copy it rather than starting from nothing; every
   layer keeps a name. (The editable `.vixl` documents are not checked in: `vixl/ops/` has the operation lists and
   the scripts rebuild them.)
4. **Reuse the building blocks** in `vixl/kit/`: `art.tractor()`, `art.wheat()`, `art.rivets()` and the helpers
   emit ordinary Vixl operations, and the `build_*.py` scripts rebuild every file in this package
   (`cd vixl/kit && python build_poster.py`, output in `vixl/kit/out/`).
5. **Settings that define the look:**
   - ribbon: `{"shape": "ribbon"}`, fill `@accent`, wordmark centred with `"within": "<ribbon>"`;
   - seal: `{"shape": "seal", "count": 22-24, "inner_radius": 0.88}`, plus a Paper ring ellipse inset about 10%;
   - sunburst: `{"shape": "sunburst", "count": 20-28, "opacity": 0.55}` with `{"type": "look", "look": "halftone"}`;
   - paper: `{"type": "look", "look": "paper"}` on the background solid only;
   - display shadow: `layer-style` `drop-shadow` with `blur: 0`, `opacity: 1`, `color: @shadow-red`, `dx = dy` ≈ 3–4%
     of font size;
   - frame: two transparent rectangles with a Barn Red stroke (about 10 px, then about 2 px about 22 px inside).
6. **Check before exporting:** `vixl -p FILE check`. Mark scenery that bleeds off the edge with `layer-intent`
   `allow_crop`, and backdrop layers with `role: "background"` (as in `tractr-poster.vixl`), so the checks focus on
   the type. Then look at a render, compare it with `style-board.png`, and export (exports refuse to overwrite
   unless asked).
