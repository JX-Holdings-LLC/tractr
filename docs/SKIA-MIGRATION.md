# SkiaSharp 3 and Avalonia 12 migration

Status: **done** (#114). The editor renders with Avalonia 12.1.4 on SkiaSharp
3.119.4; SVG import uses Svg.Skia 5.1.1 on the same SkiaSharp. The managed
mitigations below stay in place. What is left is the check by hand under
[Still to check by hand](#still-to-check-by-hand).

Before, the editor rendered with Avalonia 11, which draws with SkiaSharp
2.88. 2.88.9 is the last 2.88 release: fixes to Skia's bundled codecs
(libpng, libjpeg-turbo, libwebp, the GIF and BMP decoders) and to FreeType
and HarfBuzz no longer reach it. The editor decodes images that creators and
players did not make themselves, so this mattered.

## What decodes untrusted data

| Path | Input | Code |
|---|---|---|
| Art import | PNG, JPEG, WebP, GIF or BMP files the creator picks | `ArtImport.Reencode` |
| SVG import | SVG files the creator picks (XML is parsed in managed code by Svg.Custom; embedded images are decoded by Skia) | `ArtImport.RasterizeSvg`, `SvgSafety` |
| Project and pack art | `data:image/…` URLs inside imported projects and content packs, shown as thumbnails, previews and sprite sheets | `ArtBitmaps` (Avalonia `Bitmap`, which decodes with Skia) |
| Export Game icons | the project's icon asset | `FarmEngine.Export/Icons.fs` |
| Text | project text is shaped with HarfBuzz and drawn with the bundled Inter font | Avalonia |

Not part of this plan: the Rust player and renderer decode images with the
`image` crate (`crates/farm-render/src/images.rs`, with its own header size
limits), not with Skia.

## Mitigations in place

They were added before the upgrade and stay: a newer Skia has fewer known
bugs, not none.

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
- **SVG import is sanitized and fetches nothing** (#74). `ArtImport.CheckSvg`
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
- **Svg.Skia 5's new features stay off.** `SvgSafety.Settings` switches off
  scripting (`EnableJavaScript`, `EnableExternalJavaScript`, no
  `JavaScriptRuntimeFactory`; no script runtime package is referenced, so
  `SKSvgSettings.DefaultJavaScriptRuntimeFactory` is null too), link
  navigation and SVG fonts. Its asset loader implements only
  `ISvgAssetLoader`, so Svg.Model loads every image through the checked
  `LoadImage` (not the newer `ISvgImageAssetLoader`) and loads no fonts from
  the document (`ISvgDocumentFontLoader`).
  `SvgImportSafetyTests.TheRendererRunsNoScriptsAndLoadsImagesOnlyThroughTheCheck`
  checks all of this.

## Versions

Checked against the packages' published dependencies (October 2026):

| Package | Before | Now | Why |
|---|---|---|---|
| Avalonia, Avalonia.* | 11.3.22 | 12.1.4 | Avalonia 12.1.4's `Avalonia.Skia` depends on SkiaSharp **3.119.4** and HarfBuzzSharp 8.3.1.3 |
| SkiaSharp, SkiaSharp.NativeAssets.Linux | 2.88.9 | 3.119.4 | must be the version `Avalonia.Skia` uses: one native Skia per process |
| Svg.Skia | 3.7.0 | 5.1.1 | Svg.Skia 4.9–5.1 build on SkiaSharp 3.119; **5.2 and later need SkiaSharp 4.148**, which no Avalonia release uses yet |
| Avalonia.Headless.XUnit | 11.3.22 | 12.1.4 | needs **xUnit v3** (`xunit.v3` 3.2.2 in the app tests) |

`dotnet list package --include-transitive` shows one SkiaSharp (3.119.4) and
one HarfBuzzSharp (8.3.1.3) in every project.

SkiaSharp 4.x waits for an Avalonia release that renders with it, and
Svg.Skia 5.2 and later wait with it. Dependabot's ignore rules
(`.github/dependabot.yml`) skip SkiaSharp 4 and Svg.Skia 5.2 and later until
then; the `avalonia` group moves Avalonia, SkiaSharp and Svg.Skia together.

## What changed

- **Packages** (`Directory.Packages.props`): the versions above. The F#
  test projects stay on xUnit 2 (`xunit` 2.9.3, for FsCheck.Xunit); the app
  tests use `xunit.v3`.
- **App tests on xUnit v3**: the test project is an executable
  (`OutputType` Exe), `NativeFactAttribute` passes the source location to
  `FactAttribute` (xUnit3003), and the update tests pass
  `TestContext.Current.CancellationToken` to the async calls (xUnit1051).
  `[AvaloniaFact]`, `[AvaloniaTheory]`, `CaptureRenderedFrame`,
  `ForceRenderTimerTick` and `KeyPressQwerty` are unchanged; the coverage
  collector (`coverlet.collector`, `--collect:"XPlat Code Coverage"`) keeps
  working.
- **Text shaping is configured on its own in Avalonia 12**: the headless test
  app (`TestAppBuilder`) calls `UseHarfBuzz()` after `UseSkia()`. The app
  itself uses `UsePlatformDetect()`, which includes HarfBuzz.
- **`TextBox.Watermark` is obsolete** (renamed `PlaceholderText`); every
  view and test uses `PlaceholderText`.
- **`Bitmap.Save(path, quality)` is obsolete**; the tests that save captures
  pass `PngBitmapEncoderOptions.Default`.
- **Export Game icons** (`Icons.renderOne`): `SKPaint.FilterQuality` is
  obsolete, so the icon is drawn as an `SKImage` with explicit
  `SKSamplingOptions`: bilinear from the nearest mipmap when it shrinks,
  bilinear at its own size, Mitchell cubic when it grows. That is what
  SkiaSharp 2.88's high filter quality chose: the default icon comes out
  pixel for pixel the same at 16, 32 and 256 pixels, and slightly different
  at 48 (between two mipmap levels: about 2 of 255 per channel on average,
  more along edges). Mitchell alone would blur the 256-pixel
  icon at its own size, which made its PNG too large for the slot the
  Windows player template reserves.
- Nothing else in the editor needed a change: `WriteableBitmap.Lock()`
  (Rgba8888, premultiplied), `Bitmap.DecodeToWidth`/`DecodeToHeight`, the
  storage provider pickers, `TopLevel.GetTopLevel`, compiled bindings, the
  Fluent theme and `Avalonia.Fonts.Inter` build and behave as before. The
  editor uses none of the APIs Avalonia 12 removed (`IDataObject`,
  `SystemDecorations`, `GotFocusEventArgs`, `Gestures.*`, the binding
  plugins, `GetVisualRoot`).
- **Editor screenshots** (`fixtures/editor`): the comparison passes
  unchanged, so the references were not re-recorded.
- **License notices** (`tools/editor-licenses/THIRD-PARTY-dotnet.txt`)
  regenerated: new packages are `Avalonia.HarfBuzz`,
  `Avalonia.FreeDesktop.AtSpi`, `Svg.Animation` and `Svg.SceneGraph`.

## Still to check by hand

The headless tests render with real Skia but cannot cover these; check them
before the next release:

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
