<p align="center">
  <img src="assets/branding/SlideStrike.png" alt="Original SlideStrike logo" width="160" height="160">
</p>

<h1 align="center">SlideStrike</h1>

<p align="center">
  <a href="https://prowebdevsolutions.com"><picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/branding/PWDS-logo-orange.svg">
    <img src="assets/branding/PWDS-logo.png" alt="PWDS company logo" width="120">
  </picture></a>
</p>

<p align="center"><strong>Build the story one slide at a time.</strong></p>

SlideStrike is a presentation editor project with a slide model, speaker notes, images, a native desktop UI, and export paths. This repository preserves the original editable `.sstrike` prototype and includes a fuller native presentation foundation based on a pinned DeckCraft source revision.

![SlideStrike native editor at tested source b66faf4, reopened on a self-authored three-slide launch brief](assets/screenshots/slidestrike-native-current.png)

This capture is from native source `b66faf4`, after importing a synthetic `.sstrike` deck, saving it as `.deckcraft`, and reopening it. The image predates the subsequent source change that removed publisher wording from the working title row; PWDS remains in About and the start screen. See [native runtime evidence](docs/NATIVE_RUNTIME_ACCEPTANCE.md). The [older lightweight editor screenshot](assets/screenshots/slidestrike-editor.png) has [separate provenance](docs/SCREENSHOT_PROVENANCE.md).

<p align="center"><a href="#highlights">Highlights</a> · <a href="#build-and-test">Build and test</a> · <a href="assets/branding">Original artwork</a></p>

## Highlights

- Arrange text and image objects on slides.
- Save an editable `.sstrike` deck with notes.
- Present or export pages as PNG or PDF.
- Explore a fuller native presentation model, PPTX paths, rendering, media, and an egui desktop editor in `native/slidestrike`.

## Current status

The original standalone package passed 21 locked tests at an earlier checkpoint. Its lightweight editor built offline and rendered an [archived screenshot of the legacy interface](docs/SCREENSHOT_PROVENANCE.md). The newer native foundation [built and passed focused tests on Windows CI at draft source `b66faf4`](https://github.com/Gerbzz/SlideStrike/actions/runs/38673620694). A CI-built executable then passed an isolated Windows check of legacy import, required Save As state, native save/reopen, source preservation, notes, image, and authentic rendered window capture. See [the exact runtime evidence and limits](docs/NATIVE_RUNTIME_ACCEPTANCE.md). No installer or release is supplied. Native interaction, accessibility, and wider presentation fidelity remain unverified.

| Path | Role |
| --- | --- |
| `apps/slidestrike` | Original editor and `.sstrike` archive reader/writer. |
| `native/slidestrike` | Full adapted model, PPTX, render, engine, media, and desktop UI source graph; desktop build, focused tests, and a synthetic import/save/reopen runtime check passed on Windows. |
| `fixtures/slidestrike` | Small synthetic inputs for the original tests and the new import regression test. |

The native graph retains its internal `deckcraft-*` crate names and `.deckcraft` file format so its model, format, engine, and UI remain coherent. Its desktop executable is named `slidestrike`. The older DesignCraft page export subset supports only the original prototype.

## Deck and exported pages

A `.sstrike` deck keeps slide objects and speaker notes editable. PNG and PDF exports contain the presented pages; notes stay in `.sstrike` and are omitted from those exports. The bounded export check used three small synthetic slides, so wider layout fidelity remains open.

The native `Import Older Strike Deck` command converts an existing `.sstrike` archive into an **unsaved** presentation. It checks old archive contents, maps slide objects and notes, and reports any required font substitution. Save As must use a new `.deckcraft` or `.pptx` path; native writes to `.sstrike` are refused to protect the original file. Import resets old undo history and regenerates object IDs. The native draft now keeps an imported deck marked unsaved until Save As so its close guard can prompt. Wider visual and fidelity validation is pending.

## Build and test

Use Rust 1.90 or newer and a supported native compiler toolchain. Cargo may need to fetch dependencies. From the repository root:

```powershell
cargo test --locked --manifest-path apps/slidestrike/Cargo.toml --all-features
cargo build --locked --manifest-path apps/slidestrike/Cargo.toml --bin slidestrike-native --features native-ui

# Native foundation (run its broader engine tests and desktop build)
cargo test --locked --manifest-path native/slidestrike/Cargo.toml -p deckcraft-engine
cargo build --locked --manifest-path native/slidestrike/Cargo.toml -p deckcraft --bin slidestrike
```

The app has its own `Cargo.lock` and `[workspace]`. Its path dependencies resolve to `vendor/designcraft-page`, whose eight selected Rust crates, needed fonts, hyphenation data, ICC profile, upstream licenses, and notices are included. Synthetic image and deck fixtures live in `fixtures/slidestrike` at their tested relative paths. Cargo fetches the remaining dependencies from crates.io.

## Limits and source terms

The synthetic export checks covered three small slides with visible text and image placement. The native runtime check covered one three-slide legacy import, native save/reopen, and a rendered window capture. Native pointer, keyboard, screen reader, high contrast, scaling, presentation controls, font fallback, PPTX losses, broader media, recovery, and other-platform acceptance are pending. No PowerPoint parity claim is made.

The app's first-party license has not been selected; no first-party license grant is made here. The bundled upstream DesignCraft subset retains its original MIT/Apache license texts and third-party notices. The native graph is adapted from [`storytold/deckcraft` commit `be14248577b92059eb2b8c0c9460cceffbc1f01d`](https://github.com/storytold/deckcraft/commit/be14248577b92059eb2b8c0c9460cceffbc1f01d), with its MIT/Apache licenses, notice, and attribution. The included Source Serif 4 original retains its OFL notice. See [SOURCE_SCOPE.md](SOURCE_SCOPE.md) and [native source adaptation](native/slidestrike/ADAPTATION.md). Review dependency notices, including Symphonia's MPL-2.0 terms, before distributing binaries.

## PWDS brand artwork

The company mark in this masthead is the existing [official PWDS logo](https://github.com/Gerbzz/PWDS-Website/blob/main/public/prowebdevsolutions-logo.png), with the [official orange display asset](https://prowebdevsolutions.com/images/prowebdevsolutions-logo-orange.svg) for dark backgrounds. The app logo above it remains the original Strike suite artwork.

The SlideStrike peacock and PWDS artwork are separate brand assets; no public license for those marks is granted here.
