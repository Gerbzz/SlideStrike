# Attribution

SlideStrike carries this donor attribution record with the copied DeckCraft source. Some rows
describe upstream documentation, screenshots, icons, or packaging assets that are not included in
this source adaptation. The included standalone visual files are the approved SlideStrike peacock
icon (`assets/app-icon/slidestrike.png` and derived 256 px PNG/ICO), the unchanged Source Serif 4
original, its OFL notice, and synthetic CC0 media test fixtures. The peacock is owner-provided
Strike artwork and is not licensed by the donor MIT/Apache terms.

Every asset DeckCraft includes, bundles, embeds at build time or shows in its documentation, with
its author, source and licence. The policy is in [AGENTS.md](AGENTS.md) §1. `cargo xtask assets`
fails if a file under `assets/`, `docs/images/` or `docs/brand/` is missing from this table.

The SlideStrike adaptation includes one unchanged Adobe-authored, openly licensed Source Serif 4
font face from the pinned WordCraft snapshot to preserve older Strike deck text. This is a
documented Strike exception to DeckCraft's original exclusion policy. SlideStrike makes no claim
to the reserved `Source` font name for modified derivatives; the bundled original is unmodified.
The remaining themes, colour schemes, layouts, preset-shape geometry, sample deck and UI icons
are original work of the DeckCraft contributors (drawn or generated in code).

## In this repository

| Asset | Author | Source | Licence | Used for |
|---|---|---|---|---|
| `assets/fonts/SourceSerif4-Regular.ttf` | Adobe; Source Serif project authors | [Adobe Source Serif release](https://github.com/adobe-fonts/source-serif/tree/release), copied unchanged from pinned WordCraft `191d8fd48ec28b959dc269159a3b0f2c8b885669` (`assets/fonts/SourceSerif4-Regular.ttf`, SHA256 `E5A4EE6A3D87BB9024796BE390C6771E2A0EB1883DAE25EFFAF57CA01668E24B`) | SIL OFL-1.1, Reserved Font Name `Source`; `assets/fonts/OFL-SourceSerif4.txt` | Original face for legacy text and font subsets in searchable PDFs |
| `assets/fonts/OFL-SourceSerif4.txt` | Adobe / SIL International | Pinned WordCraft asset notice and [official Source Serif license](https://github.com/adobe-fonts/source-serif/blob/release/LICENSE.md) | OFL-1.1 license text and copyright notice | License and notice accompanying the bundled original |
| `docs/brand/artcraft-logo.svg`, `docs/brand/artcraft-logo.png` | ArtCraft Team | ArtCraft brand kit (project owner) | ArtCraft trademark, `docs/brand/LICENSE-brand.txt` (not open source) | README header |
| `docs/brand/artcraft-logo-white.svg`, `docs/brand/artcraft-logo-white.png` | ArtCraft Team | ArtCraft brand kit | ArtCraft trademark, `docs/brand/LICENSE-brand.txt` | README header (dark mode) |
| `docs/brand/artcraft-mark.svg`, `docs/brand/artcraft-mark.png` | ArtCraft Team | ArtCraft brand kit | ArtCraft trademark, `docs/brand/LICENSE-brand.txt` | README footer, About |
| `docs/brand/artcraft-mark-black.svg`, `docs/brand/artcraft-mark-black.png` | ArtCraft Team | ArtCraft brand kit | ArtCraft trademark, `docs/brand/LICENSE-brand.txt` | Mark for light backgrounds |
| `docs/brand/LICENSE-brand.txt` | ArtCraft Team | craftrules `standards/license-files` | — (licence text) | Terms for the ArtCraft marks |
| `assets/app-icon/deckcraft-1024.png`, `assets/app-icon/hicolor/*/apps/ai.storyteller.deckcraft.png` | DeckCraft contributors | rendered by DeckCraft from `assets/app-icon/icon-source.deckcraft` (shapes only) | MIT OR Apache-2.0 | Placeholder app icon (presentation board) until the mascot icon is drawn |
| `packaging/macos/dmg/background.svg` | @XusBadia | DeckCraft macOS DMG window background, original work for this repository (2026-10-08), derived from the DeckCraft app icon `assets/app-icon/deckcraft-1024.png` (referenced via `<image>`, not copied); text is outlined Inter / JetBrains Mono glyph paths | MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`) | Finder window of the macOS DMG; `packaging/macos/dmg/generate.py` renders it to `background.tiff` |
| `packaging/macos/dmg/background.tiff` | @XusBadia | Rendered from `packaging/macos/dmg/background.svg` by `packaging/macos/dmg/generate.py` | MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`) | 1x + 2x HiDPI TIFF for the macOS DMG window; includes the app icon (see its row) and LittleCMS's built-in sRGB profile ("No copyright, use freely") |
| `assets/app-icon/deckcraft.icns`, `assets/app-icon/deckcraft.ico` | DeckCraft contributors | generated from the PNGs above by `packaging/icons.sh` (sips/iconutil, `cargo xtask ico`) | MIT OR Apache-2.0 | macOS and Windows app icons |
| `assets/app-icon/hicolor/scalable/apps/ai.storyteller.deckcraft.svg` | DeckCraft contributors | the shapes of `assets/app-icon/icon-source.deckcraft` written out as SVG (same coordinates, colours and corner radii) | MIT OR Apache-2.0 | Scalable app icon for Linux desktops (hicolor theme) |
| `assets/app-icon/icon-source.deckcraft` | DeckCraft contributors | made with `deckcraft-cli run` (see git history) | MIT OR Apache-2.0 | Source of the placeholder icon |
| `docs/images/editor.png`, `docs/images/shape-format.png`, `docs/images/format-pane.png`, `docs/images/slide-sorter.png`, `docs/images/dark-mode.png`, `docs/images/sample-deck.png` | DeckCraft contributors | Screenshots of DeckCraft and slides rendered by `deckcraft-cli` from the built-in sample deck (original content; Inter font, OFL-1.1) | MIT OR Apache-2.0 | README screenshots |
| `crates/media/tests/fixtures/tone*` (`tone.mp3`, `.m4a`, `.aac`, `.ogg`, `.opus`, `.flac`, `.wma`, `tone-alac.m4a`) | DeckCraft contributors | synthesised 0.5 s 440 Hz sine, encoded by `crates/media/tests/fixtures/make.sh` (ffmpeg as the encoder only) | CC0-1.0 | Audio decoding tests |
| `crates/media/tests/fixtures/clip-*` (`clip-h264.mp4`, `clip-h264.mov`, `clip-hevc.mp4`, `clip-vp9.webm`, `clip-av1.mkv`) | DeckCraft contributors | 6 frames of a colour gradient generated by `make.sh` (python3), encoded by ffmpeg (x264 / x265 / libvpx / SVT-AV1 as encoders only) | CC0-1.0 | Video decoding tests |
| `crates/media/tests/fixtures/make.sh` | DeckCraft contributors | this repository | MIT OR Apache-2.0 | Regenerates the media fixtures |

## Bundled through dependencies

| Asset | Author | Source | Licence | Used for |
|---|---|---|---|---|
| Ubuntu Light (`Ubuntu-Light.ttf`, via the `epaint_default_fonts` crate 0.36) | Dalton Maag Ltd for Canonical | https://crates.io/crates/epaint_default_fonts | Ubuntu Font Licence 1.0 | Last-resort text face when no other font is available; UI fallback |
| Hack Regular, Noto Emoji, emoji-icon-font (via `epaint_default_fonts` / egui) | Source Foundry; Google; Jakub Steiner et al. | https://crates.io/crates/epaint_default_fonts | MIT / Bitstream-Vera, OFL-1.1, MIT | egui UI fallback glyphs |

## Embedded at build time from craft-fonts (`CRAFT_FONTS_DIR`)

Release builds (and local builds with a craft-fonts checkout) embed the fonts listed in
[craft-fonts](https://github.com/storytold/craft-fonts) `fonts/latin-manifest.txt` and
`fonts/manifest.txt` (per-file authors, sources, SHA-256 and licences in its
[ATTRIBUTION.md](https://github.com/storytold/craft-fonts/blob/main/ATTRIBUTION.md)), except the
families excluded by `crates/fonts/build.rs`:

| Families | Licence | Notes |
|---|---|---|
| Inter | OFL-1.1 | Default theme font |
| Carlito, Caladea | OFL-1.1 | Metric-compatible stand-ins for documents that ask for Calibri / Cambria |
| Liberation Sans, Serif, Mono | OFL-1.1 | Metric-compatible stand-ins for Arial, Times New Roman, Courier New |
| Montserrat, Lato, Open Sans, Roboto, Merriweather, Playfair Display, Poppins, Nunito Sans | OFL-1.1 | Theme fonts |
| BIZ UDPGothic, BIZ UDMincho, Shippori Mincho, Noto Sans Arabic | OFL-1.1 | Japanese and Arabic text |

Web (wasm32) builds embed only Inter Regular/Bold and BIZ UDPGothic Regular.
