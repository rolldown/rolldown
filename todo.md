# Already-Loaded Atom Propagation — Default-On Regressions

## Status

After flipping `experimental.alreadyLoadedAtomPropagation` default to `true`:

- **Step 2 (preserveEntrySignatures gate) — DONE.** Implemented in
  `apply_strip`: when stripping a bit would leave the module with a single
  remaining bit pointing at a `Strict` entry, the strip is suppressed.
  `ExportsOnly` is already resolved to `Strict` (when the entry has exports)
  by `init_entry_point`, so the gate covers both. The module then flows
  through the common-chunk path where the existing
  `can_merge_without_changing_entry_signature` check fires.
- **Cleared by step 2:**
  - ✅ `crates/rolldown/tests/rolldown/issues/9049` (strict variant)
  - ✅ `crates/rolldown/tests/rolldown/issues/4895` (snapshot was previously
    showing buggy behavior — strict entry exposed `shared as t`; new
    snapshot correctly preserves the strict signature)
  - ✅ `crates/rolldown/tests/rolldown/misc/preserve_entry_signature/exports-only`
    (same — pre-existing buggy snapshot fixed)
  - ✅ `test262: language/module-code/verify-dfs.js` (incidental win — the
    DFS fixtures have exports that resolve to `ExportsOnly→Strict`; the gate
    keeps them out of the entry chunk and preserves DFS evaluation order)
- **Remaining failures:** 2 fixtures + 1 test262 (down from 3 + 2).

## Common root cause for what's left

The pass is conceptually Rollup-like but still missing the safety constraints
that the post-grouping `chunk_optimizer.rs` + `rehome_runtime_module`
pipeline relies on:

1. **TLA / awaited dynamic-import distinction.** Rollup runs the analysis
   twice (`already_loaded` and `awaited_already_loaded`) and only strips bits
   that are loaded *non-awaited*. Rolldown does not track await context in
   `import_record`, so we currently treat every dynamic import uniformly.
2. **Cycle / dependency / side-effect validation.** Rollup's atom-level
   analysis tracks `correlatedAtoms` and side-effect placement; our
   per-module strip ignores these. Manual chunks, runtime-helper consumers,
   and shared static-dep chains can all close static-import cycles after a
   strip.

## Failing cases by root cause

### A. TLA / awaited dynamic-import distinction

#### `test262: language/module-code/top-level-await/module-graphs-does-not-hang.js`

**Symptom:** `$DONE was not called` — the module never finishes resolving.

**Shape:**
```js
import "./module-graphs-parent-tla_FIXTURE.js";
await import("./module-graphs-grandparent-tla_FIXTURE.js");
// parent → tla_FIXTURE.js (TLA)
// grandparent → parent
```

**Why:** parent transitively depends on a TLA module; grandparent is
imported via `await import(...)`. The pass strips bits without distinguishing
awaited from non-awaited dynamic imports, producing an async evaluation
shape that never settles.

### B. Cycle / dependency / side-effect validation

#### `crates/rolldown/tests/rolldown/issues/9225`

**Symptom:**
```
AssertionError: Output chunks must not have circular static imports
cycle: api.js → main.js → api.js
```

**Why:** Manual code splitting extracts `api` into its own chunk. Static
chain is `main → api → env → dep → side`. With the optimization, all
dynamic bits get stripped from `env`/`dep`/`side` (the only dynamic-entry
importers go through main, which statically reaches them). They land in
main's chunk. `main → api` (static) and `api → main` (because main now holds
`env` that api imports) close a cycle. Genuinely unsound — ESM evaluation
hits TDZ on the back-edge.

#### `crates/rolldown/tests/rolldown/issues/8920_2`

**Symptom:** `entry-2.js should host the runtime module` — runtime ends up
in a separate `chunk.js` instead of co-located with the dominator.

**Why:** The strip moves `node2` out of the `{entry-2, node1-dyn}` common
chunk into entry-2's entry chunk. That changes the runtime-consumer chunk
set seen by `rehome_runtime_module`'s dominator search; the new set has no
dominator, so a fresh runtime chunk is emitted.

The runtime module itself is already excluded from the strip — necessary
but not sufficient, because *consumers* of runtime helpers (modules with
non-empty `depended_runtime_helper`) still get repositioned.

## Suggested fix order

1. **Track awaited dynamic imports** in `import_record` and run the analysis
   twice, only stripping bits that are loaded non-awaited. Closes test262
   TLA case (verify-dfs already cleared by step 2).
2. ~~**Plumb `preserveEntrySignatures` into `apply_strip`.**~~ ✅ Done.
3. **Cycle-aware strip.** Either run a cycle-detection-and-revert pass on
   the projected post-strip chunk graph, or restrict stripping to modules
   whose static-dep closure does not pass through a chunk statically
   downstream of the move target. Closes 9225, 8920_2.
4. **Runtime-consumer awareness.** Once (3) lands, re-evaluate whether
   modules with `depended_runtime_helper` need additional handling beyond
   the cycle guard, or whether the dominator search just needs to be
   re-run on the post-strip topology.

Until (1) and (3) land, the flag still ships 1 fixture cycle (9225) + 1
runtime-placement regression (8920_2) + 1 test262 TLA hang.
