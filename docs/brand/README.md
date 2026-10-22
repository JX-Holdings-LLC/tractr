# Tractr brand: Seed Catalog

Tractr's look is a 1950s seed packet and farm-supply catalog: warm paper, a few flat printing inks, sunburst rays,
ribbon banners, scalloped seals and a cheerful red tractor. [STYLE.md](STYLE.md) holds the rules (voice, colour, type,
motifs, composition, logo use) and `style-board.png` shows them on one page.

This look is for the **app and its marketing**: the editor, the installer, the README and the docs. It is not for the
games Tractr makes. Those keep their own art and in-game interface, and a game never gets the Tractr look unless its
creator draws it that way.

```text
STYLE.md             the style guide
APP-UI.md            how the editor builds it: theme tokens, fonts, where the exceptions are
style-board.png      palette, type and motifs on one page

logo/                logo files, PNG at 1x and 2x and SVG (lettering outlined, no fonts needed)
  tractr-logo-primary(-tagline)    red ribbon, cream lettering, hard shadow: for light backgrounds
  tractr-logo-reverse(-tagline)    cream ribbon, red lettering: for red, green or dark backgrounds
  tractr-logo-ink                  one colour
  tractr-icon-seal                 the "T" seal: anywhere the ribbon would be under 160 px wide
  tractr-app-icon                  the seal on a paper tile (1024 px PNG only: its paper grain is a raster)
  web-icons/                       favicon.ico, 16-48 px favicons, apple-touch and Android icons, manifest

illustrations/       the red tractor and a wheat stalk on transparent backgrounds, PNG and SVG
exploration/         the original poster and the editor mood study (style reference, not a spec)

vixl/
  brand.json         Vixl workspace brand: palette roles, fonts per text stage, embedded logo
  swatches.json      the extra named colours (@field-green, @mustard, ...)
  ops/               the Vixl operation lists that built each image
  kit/               Python build kit: art.py (tractor, wheat, rivets), ui.py (the editor study), vx.py (CLI
                     driver) and the build_*.py scripts
```

The pictures that sell Tractr are in [docs/media](../media): the hero, the four seed packets and the catalog plates
around the editor screenshots. They were made with [Vixl](https://github.com/jxburros/vixl), and the scripts that make
them are in `vixl/kit/`.

## Rebuilding the pictures

You need the `vixl` CLI (0.28 or newer) and network access, because Vixl fetches the four Google Fonts on first use.
Everything is built in `vixl/kit/out/`, which git ignores; copy what you keep.

```sh
cd docs/brand/vixl/kit
python build_logos.py            # logo/, the seal, the app icon, the tractor and the wheat
python build_style_board.py      # style-board.png
python build_poster.py           # exploration/tractr-poster.png
python build_marketing.py        # the README hero, the About banner and the four seed packets
python build_plates.py SHOT.png "FIG. 1" "EDIT MODE" "A caption." plate-edit-mode   # one catalog plate
```

The editor's own images (`app-icon-256.png`, `app.ico`, the header logo and the paper grain) are cut from `logo/`
with `python3 scripts/generate-app-icon.py` (needs Pillow). The About banner is
`src/FarmingRpgMaker.App/Assets/about-banner.png`, from `build_marketing.py about`.

## Making something new

Follow section 10 of [STYLE.md](STYLE.md): work in a Vixl workspace that has `brand.json`, apply `swatches.json`,
copy the nearest build script, reuse `art.tractor()`, `art.wheat()` and the other helpers, run `vixl check`, then look
at the render next to `style-board.png`. Add new pictures to `docs/media` and link them from the README or the docs
that use them.

The fonts are Alfa Slab One, Lobster, Oswald and Courier Prime, all free under the SIL Open Font License. The editor
ships them in `src/FarmingRpgMaker.App/Assets/Fonts`; their notices are in
`assets/fonts/OFL-TractrInterface.txt` and the editor's `THIRD-PARTY-dotnet.txt`. The product name, taglines and
catalog numbers are placeholder copy and may change.
