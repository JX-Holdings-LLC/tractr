# Image decoding and safety

The editor decodes images creators and players did not make themselves:
art they import, pictures inside projects and content packs from other
people, and SVG files. This page lists every place that happens, the
defences around Skia's native codecs, and what to check when SkiaSharp,
Avalonia or Svg.Skia are updated. The editor draws with Avalonia 12 on
SkiaSharp 3; the Rust player decodes images separately, with the `image`
crate (`crates/farm-render/src/images.rs`, with its own size limits).

## What decodes untrusted data

| Path | Input | Code |
|---|---|---|
| Art import | PNG, JPEG, WebP, GIF or BMP files the creator picks | `ArtImport.Reencode` |
| SVG import | SVG files the creator picks (XML is parsed in managed code by Svg.Custom; embedded images are decoded by Skia) | `ArtImport.RasterizeSvg`, `SvgSafety` |
| Project and pack art | `data:image/…` URLs inside imported projects and content packs, shown as thumbnails, previews and sprite sheets | `ArtBitmaps` (Avalonia `Bitmap`, which decodes with Skia) |
| Export Game icons | the project's icon asset | `FarmEngine.Export/Icons.fs` |
| Text | project text is shaped with HarfBuzz and drawn with the bundled Inter font | Avalonia |

## Defences

A current Skia has fewer known bugs, not none, so these stay whatever its
version:

- **Managed header check before every Skia decode of untrusted data.**
  `ImageHeaders.Read` reads the format and size of PNG, JPEG, GIF, WebP and
  BMP images in managed code. Anything else (ICO, WBMP, HEIF, AVIF, DNG/RAW
  and the other formats Skia sniffs) never reaches a Skia codec, and images
  over the import limits (8192 pixels per side, 16 megapixels) are refused
  before Skia parses them. This covers art import, `ArtBitmaps`
  (thumbnails, previews, the sprite sheet) and images embedded in SVGs
  (`SvgSafety`'s asset loader). Art import and `ArtBitmaps` then also check
  Skia's own reading of the header (`SKCodec`) against the same limits
  before pixels are allocated.
- **Export icons are PNG only.** `Icons.render` checks the PNG signature and
  the codec's format before decoding, although Skia would decode any format it
  knows behind a `data:image/png` label.
- **SVG import is sanitized and fetches nothing.** `ArtImport.CheckSvg`
  refuses scripts, event handlers, foreign HTML, DTD declarations, `xml:base`,
  CSS imports (also written with CSS escapes) and every link that is not a
  `#fragment` or an embedded base64 image. Embedded SVGs are checked
  recursively, at most two deep, and embedded raster images by their header.
  Under that, `SvgSafety` switches off Svg.Skia's external loading
  (`SvgDocument.ResolveExternal*`, DTD processing of nested documents),
  refuses non-`data:` image links in the parsed document, makes
  `WebRequest` refuse http, https, ftp and file URLs for the process (Svg.Model
  loads non-`data:` images with it), and decodes embedded images only after
  the header check.
- **Svg.Skia's scripting and loaders stay off.** `SvgSafety.Settings` switches off
  scripting (`EnableJavaScript`, `EnableExternalJavaScript`, no
  `JavaScriptRuntimeFactory`; no script runtime package is referenced, so
  `SKSvgSettings.DefaultJavaScriptRuntimeFactory` is null too), link
  navigation and SVG fonts. Its asset loader implements only
  `ISvgAssetLoader`, so Svg.Model loads every image through the checked
  `LoadImage` (not the newer `ISvgImageAssetLoader`) and loads no fonts from
  the document (`ISvgDocumentFontLoader`).
  `SvgImportSafetyTests.TheRendererRunsNoScriptsAndLoadsImagesOnlyThroughTheCheck`
  checks all of this.

## Versions move together

| Package | Version | Why |
|---|---|---|
| Avalonia, Avalonia.* | 12.1.4 | `Avalonia.Skia` 12.1.4 depends on SkiaSharp **3.119.4** and HarfBuzzSharp 8.3.1.3 |
| SkiaSharp, SkiaSharp.NativeAssets.Linux | 3.119.4 | must be the version `Avalonia.Skia` uses: one native Skia per process |
| Svg.Skia | 5.1.1 | Svg.Skia 4.9–5.1 build on SkiaSharp 3.119; **5.2 and later need SkiaSharp 4.148**, which no Avalonia release uses yet |
| Avalonia.Headless.XUnit | 12.1.4 | needs **xUnit v3** (`xunit.v3` in the app tests; the F# test projects stay on xUnit 2 for FsCheck.Xunit) |

`dotnet list package --include-transitive` must show one SkiaSharp and one
HarfBuzzSharp in every project. SkiaSharp 4 waits for an Avalonia release
that renders with it, and Svg.Skia 5.2 and later wait with it. Dependabot's
ignore rules (`.github/dependabot.yml`) skip them until then; its `avalonia`
group moves Avalonia, SkiaSharp and Svg.Skia together.

Two details the code depends on: the headless test app (`TestAppBuilder`)
calls `UseHarfBuzz()` after `UseSkia()`, because Avalonia 12 configures text
shaping on its own; and Export Game's icons (`Icons.renderOne`) are drawn as
an `SKImage` with explicit `SKSamplingOptions` (bilinear from the nearest
mipmap when shrinking, bilinear at its own size, Mitchell cubic when
growing). Mitchell alone would blur the 256-pixel icon at its own size and
make its PNG too large for the slot the Windows player template reserves.

## Checks by hand

The headless tests render with real Skia but cannot cover these; check them
before a release that updates Avalonia or Skia:

- on Windows: GPU rendering, window chrome, file pickers, the clipboard, IME
  and high DPI, and the Windows run of the app and Export tests;
- on Linux: X11 and Wayland, and the screen reader (Avalonia 12 brings its own
  AT-SPI bridge, `Avalonia.FreeDesktop.AtSpi`).

## Keeping it safe in later updates

- **Svg.Skia updates.** Re-check that Svg.Model still loads non-`data:`
  images through `WebRequest` (`GetImageFromWeb`): the `WebRequest` block
  depends on it, and `SvgImportSafetyTests.TheRendererFetchesNothingEvenWithoutTheCheck`
  (a loopback listener and a local file) is what notices when it stops. Look
  for new loaders and settings that could load or run something, and keep
  them off in `SvgSafety`.
- **One native Skia per process.** Avalonia, Svg.Skia and `FarmEngine.Export`
  must resolve to the same SkiaSharp; a mismatch fails at run time
  (`EntryPointNotFoundException`) rather than at build time. Check
  `dotnet list package --include-transitive` after every bump.
- **Rendering differences.** A Skia update rasterizes text and images
  slightly differently. The screenshot comparison tolerates anti-aliasing;
  anything it flags must be looked at, not re-recorded blindly.
