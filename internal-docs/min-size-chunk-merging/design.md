# Min-size chunk merging (`experimental.minChunkSize`)

Status: **implemented** (experimental, default-off). Pass lives in
`crates/rolldown/src/stages/generate_stage/min_chunk_size.rs`
(`GenerateStage::merge_small_common_leaf_chunks`).

## Why

Rolldown (Rollup model) extracts _every_ shared module into a shared chunk with no
size floor. Empirically (4 code-split shapes probed), a shared chunk always has
≥2 importers — so a "merge into one importer" pass has no clean targets, and the
chunking algorithm already folds single-reachability modules into their consumer.

webpack/rspack `splitChunks.minSize` (default ~20 KB) instead **does not extract**
a sub-threshold shared module: it stays _duplicated_ in each consumer. For a tiny
shared leaf used by two entries that means 2 chunks instead of 3 — fewer requests,
less cross-chunk `import` boilerplate, better gzip. This is the win we want.

So min-size merging fundamentally requires **duplicating a module into multiple
chunks**, which Rollup/rolldown deliberately never does. That is the whole cost.

## What makes it hard (validated against the code)

Rolldown assumes _one module ⇒ one chunk_ throughout the back half of generate:

| Stage             | File                                                                        | 1:1 assumption                                                    |
| ----------------- | --------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| cross-chunk links | `stages/generate_stage/compute_cross_chunk_links.rs` (≈186/462/583/619)     | import-vs-local via `module_to_chunk[m] == chunk`                 |
| deconfliction     | `utils/chunk/deconflict_chunk_symbols.rs`                                   | per-chunk, exec-order-dependent renamer (`chunk.canonical_names`) |
| finalize          | `module_finalizers/mod.rs:266`, `stages/generate_stage/finalize_modules.rs` | each module AST finalized **once** against its single chunk       |
| render            | `stages/generate_stage/render_chunk_to_assets.rs:~259`                      | codegens `ast_table[m]` once per `chunk.modules` entry            |

Good news from the render path: it codegens the _same_ finalized AST for every
`chunk.modules` entry, so a module listed in two chunks renders in both **without
AST cloning** — _iff_ that single finalized AST is valid in every chunk.

## Approach: leaf-only duplication with globally-pinned names

Restrict to **side-effect-free leaf modules** (no import records, no side effects):

- No outgoing refs ⇒ the finalized AST contains nothing chunk-specific except its
  own declared-symbol names.
- Side-effect-free ⇒ executing it in N chunks is observationally identical (only
  declarations), so duplication is safe.

Make the one finalized AST valid everywhere by giving each duplicated-leaf symbol a
**single name used by every chunk**:

1. Pre-name all duplicated-leaf declared symbols once (a `Renamer`) → unique among
   the duplicated set; store `pinned_names: FxHashMap<SymbolRef, CompactStr>`.
2. In each chunk that receives a leaf, _reserve_ those names and pin
   `canonical_names[symbol] = pinned` before deconflicting the chunk's own symbols
   (so any colliding author symbol is renamed instead). Needs a small
   `Renamer::pin_name` helper.

Then references resolve to the pinned name in every chunk, and the leaf's def uses
the same pinned name. (minify re-shortens per chunk afterwards; each chunk is
self-contained.)

### Local-vs-cross rule

Add `duplicated_leaf_modules: FxHashSet<ModuleIdx>` to `ChunkGraph`. The
import-vs-local checks become `module_to_chunk[m] == chunk || duplicated_leaf_modules.contains(m)`
— a duplicated leaf is local to any chunk that references it, because we copy it
into every importer.

### Importer-set completeness (the subtle part)

The leaf must be duplicated into **every** chunk that references its symbols, incl.
cross-chunk re-export chains (module A in C1 re-exports S's symbol; B in C2 imports
from A ⇒ C2 references S's canonical symbol although its direct edge is to A). Two
ways to get the authoritative set:

- **Run the pass AFTER `compute_cross_chunk_links`** and read each chunk's
  `imports_from_other_chunks` (authoritative; includes re-export chains). Cost:
  must scrub those link structures + `exports_to_other_chunks` when removing S, and
  downstream passes (`on_demand_wrapping`, `compute_chunk_output_exports`) consume
  them.
- **Run BEFORE** and detect importers from direct import edges, but then restrict
  eligibility to leaves whose symbols are **not re-exported across chunks**
  (conservative; provably complete via direct edges). Simpler mutation, narrower
  applicability. **Recommended for the first landed version.**

## Touchpoints

1. Option: `experimental.minChunkSize?: number` — **done** (TS `input-options.ts`,
   `bindingify-input-options.ts`; Rust `experimental_options.rs` +
   `binding_experimental_options.rs`; helper `min_chunk_size()`).
2. New pass after chunk formation: detect eligible chunks (Common, not runtime, all
   side-effect-free leaves via `import_records.is_empty()` +
   `side_effects().has_side_effects() == false`, size < min, has importers, not
   re-exported across chunks), compute pinned names, mutate (`chunk.modules` += leaf
   into each importer; tombstone S via `post_chunk_optimization_operations::Removed`;
   record `duplicated_leaf_modules`), re-run `sort_chunk_modules`.
3. `Renamer::pin_name` + `deconflict_chunk_symbols` pins/reserves duplicated-leaf
   names (reads `pinned_names`).
4. Local-vs-cross set-membership in `compute_cross_chunk_links.rs` +
   `module_finalizers/mod.rs:266`.

## Staged plan (all landed)

- **S1 (done):** option plumbing; behavior-neutral.
- **S2 (done):** detector + graph mutation + pinned-name computation behind the option.
- **S3 (done):** duplicated-leaf "local everywhere" rule in
  `compute_cross_chunk_links.rs` (symbol-assign skip + cross-import skip) and
  `module_finalizers/mod.rs`; `Renamer::pin_name` + deconflict pinning.
- **S4 (done):** ON fixtures in `packages/rolldown/tests/fixtures/min-chunk-size/`
  (`basic`, `multi-export-leaf` with minify, `re-export-excluded`) asserting
  chunk-count drop + correct runtime.

### Note: re-export completeness (learned during S4)

The conservative re-export exclusion must scan **all** modules, not just
`is_included` ones — a pure re-export pass-through (`export { k } from './util'`)
is typically tree-shaken (`is_included == false`) yet a consumer can still
reference the leaf through it. The `re-export-excluded` fixture pins this: it
failed with `ReferenceError: k is not defined` before the all-modules scan.

### Note: ESM-only eligibility (learned post-S4 review)

The "local everywhere" rule was only wired into the **ESM** finalizer and
`compute_cross_chunk_links` paths. The CJS/IIFE/UMD reference-resolution paths
(`module_finalizers::ScopeHoistingFinalizer` CJS branch and `types/generator.rs`)
still classify a duplicated leaf living in a non-primary chunk as cross-chunk and
index a require binding that `compute_cross_chunk_links` deliberately never
populated for the leaf — a hard `panic!("no entry found for key")` at
`module_finalizers/mod.rs`. Reproduced with a 2-entry shared side-effect-free leaf
and `format: "cjs"`.

`merge_small_common_leaf_chunks` therefore **early-returns unless the output
format is ESM** (`OutputFormat::is_esm()`); the `min_chunk_size_cjs_not_duplicated`
fixture pins that the leaf stays a standalone shared chunk under CJS. Lifting this
gate requires teaching those non-ESM reference paths the same
`duplicated_leaf_modules` carve-out.

`require()`d and otherwise wrapped leaves are already excluded by the existing
`import_records.is_empty()` leaf check — wrapping introduces a runtime import
record, so a wrapped leaf is never a "leaf" for this pass (verified by the
`min_chunk_size_require_not_duplicated` fixture: the required leaf stays a wrapped
shared chunk).

### Note: leaf-name vs importer global shadowing (guarded)

A duplicated-leaf symbol is pinned to its own name and force-reserved in every
importer chunk _before_ that chunk's unresolved (free) globals are reserved, and a
free reference is never renamed. So a leaf-declared name that also appears as an
unresolved global in an importer chunk would have its global reference silently
captured by the copied declaration — wrong runtime value, no error (e.g. a leaf
exporting `process` shadows an importer's `typeof process`). Built-in globals are
safe (the renamer reserves `GLOBAL_OBJECTS`), but Node globals, bundler-injected /
`provide`d globals, etc. are not.

`merge_small_common_leaf_chunks` therefore **skips duplicating a leaf when any of
its declared names collides with an unresolved global of one of its importer
chunks** (`leaf_name_shadows_importer_global`); the leaf stays a standalone shared
chunk — always correct, at worst a missed optimization. Pinned by the
`min_chunk_size_esm_shadow_global_not_duplicated` fixture.

## Risks / edge cases

CJS/IIFE/UMD output is gated off entirely (see the ESM-only note above); within ESM,
wrapped leaves and dynamic-import targets/entries are already excluded (`EntryPoint`,
not `Common`). Remaining edge cases to validate before default-on: HMR `hot` refs,
external-namespace symbols, a leaf shared by 3+ chunks, `preserveModules`/manual
chunks (excluded). The full `crates/rolldown/tests` + `packages/rolldown-tests`
suites must stay green with the option OFF, and the ON fixtures must execute
correctly (no duplicate-symbol / ReferenceError) before considering default-on.

## Rejected alternatives

- Clean single-importer merge (no duplication): no targets in rolldown's model.
- Merging small shared chunks together: over-fetch (different reachability sets).
- Finalize-per-(module,chunk) with AST cloning: oxc arena ASTs make re-finalization
  heavy; the pinned-name trick keeps it single-pass.
