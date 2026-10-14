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

SlideStrike provides an editable deck for a presentation in progress. Arrange slides with text and images, keep speaker notes, revise with undo and redo, and export PNG or PDF through the optional native editor.

<p align="center"><a href="#highlights">Highlights</a> · <a href="#build-and-test">Build and test</a> · <a href="assets/branding">Original artwork</a></p>

## Highlights

- Arrange text and image objects on slides.
- Save an editable `.sstrike` deck with notes.
- Present or export pages as PNG or PDF.

## Current status

The main branch is a lightweight editor prototype. The standalone package passed 21 locked tests, and its native editor built and rendered an [archived screenshot of the legacy interface](docs/SCREENSHOT_PROVENANCE.md), but no installer or release is supplied. A [newer native foundation remains in draft PR #1](https://github.com/Gerbzz/SlideStrike/pull/1); its [Windows source checks passed on revision `00d189c`](https://github.com/Gerbzz/SlideStrike/actions/runs/38575829825), but it has no matching runtime screenshot or visual acceptance yet. Broad presentation compatibility, native interaction, accessibility, and layout fidelity remain unverified.

## Deck and exported pages

A `.sstrike` deck keeps slide objects and speaker notes editable. PNG and PDF exports contain the presented pages; notes stay in `.sstrike` and are omitted from those exports. The bounded export check used three small synthetic slides, so wider layout fidelity remains open.

## Build and test

Install a Rust toolchain that supports edition 2024, then from the repository root run:

```powershell
cargo test --locked --manifest-path apps/slidestrike/Cargo.toml --all-features
cargo build --locked --manifest-path apps/slidestrike/Cargo.toml --bin slidestrike-native --features native-ui
```

The app has its own `Cargo.lock` and `[workspace]`. Its path dependencies resolve to `vendor/designcraft-page`, whose eight selected Rust crates, needed fonts, hyphenation data, ICC profile, upstream licenses, and notices are included. Synthetic image and deck fixtures live in `fixtures/slidestrike` at their tested relative paths. Cargo fetches the remaining dependencies from crates.io.

## Limits and source terms

The synthetic export checks covered three small slides with visible text and image placement. Speaker notes stay in the editable deck and are omitted from exported pages. This does not establish broad PowerPoint compatibility or layout fidelity. Native pointer, keyboard, screen reader, high contrast, scaling, presentation controls, font fallback, and other-platform acceptance are pending.

The app's first-party license has not been selected; no first-party license grant is made here. The bundled upstream DesignCraft subset retains its original MIT/Apache license texts and third-party font/asset notices. See [SOURCE_SCOPE.md](SOURCE_SCOPE.md) for provenance and the tested boundary.

## PWDS brand artwork

The company mark in this masthead is the existing [official PWDS logo](https://github.com/Gerbzz/PWDS-Website/blob/main/public/prowebdevsolutions-logo.png), with the [official orange display asset](https://prowebdevsolutions.com/images/prowebdevsolutions-logo-orange.svg) for dark backgrounds. The app logo above it remains the original Strike suite artwork.
