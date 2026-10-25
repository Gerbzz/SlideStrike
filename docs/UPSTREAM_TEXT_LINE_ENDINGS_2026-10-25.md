# CRLF text normalization backport — 2026-10-25

## Origin and retained source

- Selected foundation: DeckCraft `be14248577b92059eb2b8c0c9460cceffbc1f01d`.
- Narrow upstream fix: [storytold/deckcraft a7876aab547c513a46a3e54af02bace7ed913c80](https://github.com/storytold/deckcraft/commit/a7876aab547c513a46a3e54af02bace7ed913c80), merged via upstream PR #1097.
- Source path: `crates/model/src/text.rs`; retained path: `native/slidestrike/crates/model/src/text.rs`.
- Strike base: `d60a2bfb3540d03ddf19b51d491e8bff11459f68`. The retained model source matched the selected upstream pin after line-ending normalization; this patch preserves its CRLF bytes.
- The production change is only the upstream CRLF/lone-CR normalization line in `TextBody::from_text`. The selected foundation pin, lockfile, public APIs, branding, legacy project conversion and animation/distribution customizations are unchanged. This is not a full upstream version upgrade.

## License and attribution

Upstream is `MIT OR Apache-2.0`. The imported line and adapted regression remain subject to the retained [MIT](../native/slidestrike/LICENSE-MIT) and [Apache-2.0](../native/slidestrike/LICENSE-APACHE) notices, copyright 2026 ArtCraft Team and DeckCraft contributors. Both license texts at the source commit match the retained notices after line-ending normalization. No attribution or licensing files were removed or weakened.

## Regression evidence

The test exercises real `insert.textBox`, `text.set`, `text.get`, undo and redo commands, not a stand-in normalization helper. It includes CRLF, lone CR, mixed Unicode/blank/trailing lines, unchanged LF, and empty text.

Before the production line was added, exact source `ee4e1e4e1920f3bb68161bbc2074fef36e456264` compiled and failed `tests::text_set_crlf_makes_clean_paragraphs`: actual `one\\r\\ntwo`, expected `one\\ntwo` ([hosted run 40227006397](https://github.com/Gerbzz/SlideStrike/actions/runs/40227006397), job 121270396334, 2026-10-25 19:49 UTC).

The dedicated workflow checks out the immutable PR head and runs:

```
cargo test --locked -p deckcraft-engine --lib text_set_crlf_makes_clean_paragraphs
cargo test --locked -p deckcraft-model
cargo test --locked -p deckcraft-engine
```

The existing native Windows workflow remains unchanged and covers desktop build, legacy imports/save behavior and earlier animation/distribution fixes. Passing checks must be confirmed on the final candidate; the red receipt is not a claim of completion. Native visual acceptance and released-binary acceptance are separate from these source checks.
