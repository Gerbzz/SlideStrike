# SlideStrike

SlideStrike is a Windows-first Rust presentation editor prototype. It edits an open `.sstrike` deck with slides, text and image objects, notes, selection and dragging, undo/redo, and presentation mode. The optional native editor exports PNG and PDF through the selected DesignCraft page crates.

This repository layout is a source package from the frozen 2026-10-11 Office checkpoint. The standalone layout passed 21 locked all-feature tests and built a Windows native executable. Independent review of the frozen outputs checked synthetic, searchable PDF text. No release or installer is supplied.

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
