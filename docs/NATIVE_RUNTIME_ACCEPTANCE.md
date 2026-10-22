# Native editor evidence

## Current hosted native capture

On 22 October 2026 at `00:04:49.2955520Z`, [Windows run 39694465493](https://github.com/Gerbzz/SlideStrike/actions/runs/39694465493), [job 119548354597](https://github.com/Gerbzz/SlideStrike/actions/runs/39694465493/job/119548354597), built and ran the native presentation foundation in `native/slidestrike`. Its native application subtree is identical to main `0931f68f6692e291ed9673802757cbd3cf299750`; the capture change adds only a CI helper and workflow steps.

| Evidence | Exact value |
| --- | --- |
| Capture PR head | `34bb844cddb9683233f52bdbf8e7932ecf6c5fbf` |
| Actual tested merge checkout | `49a4cab4206e32e11a36e3f125706ad7f4653253` |
| Complete checkout tree | `c789379683a602e5a9d714de7c54319f56997dbf` |
| Native application tree | `2ecac784685591743424aa576e738344853d22e8` |
| Executable SHA-256 | `782B6A0757E4CBC35B5EDCABCA4F8AF4AF332964F23B325BF0C4F6C045D93064` |
| Public fixture | `native/slidestrike/crates/engine/src/sample.rs` |
| Fixture SHA-256 | `669AA869256D48DEABB4E1C7FF19A8C3A511079DC7049F35B56642852A398A98` |
| Toolchain | `rustc 1.99.1 (58cf8f9c3 2026-10-13)`; `cargo 1.99.1 (5f94df478 2026-08-27)` |

The retained workflow gates passed: desktop build; geometric SVG/PNG/ICO checks and three verifier controls; matching Windows executable icon payloads at 16, 24, 32, 48, 64, 128 and 256 pixels; 34 media tests; three legacy conversion unit tests and four import/save integration tests; destination protection; CLI check; and MCP identity. The historical animal artwork remained unchanged.

The capture helper launched that executable with the existing `--sample` and loopback control options, using a fresh temporary profile with preferences, recovery and audio disabled. It verified one clean, pathless, eight-slide **Northern Lights** presentation, the spawned process and GUI window, title, foreground ownership and bounds. The existing native `ui.screenshot` operation captured the title slide and the fifth slide's chart from the application's rendered GPU frame. Slide selection and clean application exit were checked. Independent visual inspection confirmed readable native UI and both expected sample views.

The README uses the following unmodified 960 x 633 native client PNGs. No image cropping, overlays, redraw or rescaling was performed. Raw outer-window evidence was also captured and reviewed; it remains in the original short-lived CI artifact because its resize-frame edges include the hosted desktop. The geometric Windows title-bar icon was visible in that separate outer-window evidence. These client captures do not establish taskbar icon behavior or other display configurations.

| Original image | SHA-256 |
| --- | --- |
| [Title/editor view](../assets/screenshots/native-2026-10-22/slidestrike-title-client.png) | `AD0874042965D32A1B3AE2E7EE5CD35B739245190BD0D4E7BC15CAEDE2D7A46E` |
| [Chart/editor view](../assets/screenshots/native-2026-10-22/slidestrike-chart-client.png) | `4E029051DF02D0992048F5872EEB7BB4A00956D7B5FE9AE83B3C1B26411804C1` |

The original [machine-readable capture proof](../assets/screenshots/native-2026-10-22/proof.json) is preserved byte-for-byte with SHA-256 `B5584F4B45D6D2AA71E36098029A41C93D8A5818D5B0500E411CC0BF79B45B08`. It retains the actual checkout, subtree, executable, fixture, process/window identity, timestamp and hashes for all four original captures. The source of the sample, including its upstream DeckCraft note, is unchanged. No personal document or client asset is used. The temporary profile and console streams are not published.

This is bounded hosted Windows rendering and clean-exit evidence for the public sample. Interactive editing, accessibility, alternate DPI, wider presentation fidelity and Room PC runtime acceptance remain outside this check. It is a CI debug build, with no installer or release supplied. The original lightweight `apps/slidestrike` package is separate from the native foundation shown here.

## Historical import/save runtime check, 15 October 2026

The [draft SlideStrike foundation PR](https://github.com/Gerbzz/SlideStrike/pull/1) was built by [Windows Actions run 38673620694](https://github.com/Gerbzz/SlideStrike/actions/runs/38673620694). That run compiled the `slidestrike` desktop target, passed focused legacy import and save/reopen tests, passed the older deck overwrite guard, checked the CLI, passed an MCP identity test, and retained a one-day executable artifact. The source head was `b66faf4f3f3c3d2b4bb50b332f180a0d036b5ea2`; the tested PR merge build was `6ee65b67ea23399fe0689dc8812f1da238287b9f`. The 48,958,976-byte debug executable reported `slidestrike 0.5.0` and SHA-256 `646D4C3F41F0197530D9AD2F9DEA7068E29E366605B92BD0132BE1008BCB441B`.

On 15 October 2026 UTC, that exact executable ran on Windows with app settings directed to an isolated workspace folder. The self-authored `.sstrike` launch brief (SHA-256 `D1ADE92B734A663E2F2A01488056FE7BCF0A994856A5998E62C37790E8DE78C1`) has three fictional slides, notes, and a small original blue chart. The app's local control interface performed these checks:

1. `file.importLegacy` opened three slides, one 812-byte image, and the expected third-slide speaker notes. The imported deck had no native path, reported Save As required, and was marked dirty. That last assertion guards the editor's close prompt; it failed on the prior draft and passed after the focused [state fix](https://github.com/Gerbzz/SlideStrike/commit/b66faf4f3f3c3d2b4bb50b332f180a0d036b5ea2).
2. `file.saveAs` wrote a 6,007-byte `.deckcraft` file (SHA-256 `CDB1C91F6DD966B2A7BD7AD497D9A22EB43ED22C474DA304E01B0CD1FCDA4ED7`). The original `.sstrike` bytes remained unchanged and the deck became clean.
3. The editor quit cleanly, reopened the saved deck, reported three slides, the expected title, notes, and image, remained clean, then quit cleanly again.
4. SlideStrike's own `ui.screenshot` captured the rendered 1440 × 900 window after reopen. The [archived native screenshot from this source revision](../assets/screenshots/archive/slidestrike-native-b66faf4.png) has SHA-256 `1ABAD2E7563049E1EF9512CD46A41D56376BC0823F2538B77F5476D626AAF6D5`. Visual inspection found the ribbon, slide list, title, subtitle, and blue chart thumbnail. No personal document or client asset is shown.

This historical screenshot and executable are from source `b66faf4`, before the working-title cleanup and geometric branding. Use the current captures above for the present native interface. The historical synthetic check does not establish interactive editing, broad PPTX or layout fidelity, accessibility, other display configurations, or release readiness. Its artifact was a short-lived CI debug build, not an installer or release.
