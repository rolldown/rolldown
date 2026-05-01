# Already-Loaded Atom Propagation — Default-On Regressions

After flipping `experimental.alreadyLoadedAtomPropagation` default to `true`,
8 test cases regress (3 rolldown integration fixtures + 3 more
fixtures uncovered by deeper investigation + 2 test262 cases).

## Common root cause

The pass is conceptually Rollup-like but currently too early and too
unconditional. It strips dynamic-entry bits from modules **before** rolldown
has enforced the safety constraints that the post-grouping `chunk_optimizer.rs`
+ `rehome_runtime_module` pipeline relies on. Specifically, the strip step is
missing:

1. **TLA / awaited dynamic-import distinction.** Rollup runs the analysis
   twice (`already_loaded` and `awaited_already_loaded`) and only strips bits
   that are loaded *non-awaited*. Rolldown does not track await context in
   `import_record`, so we currently treat every dynamic import uniformly.
2. **`preserveEntrySignatures` guard.** The chunk_optimizer's
   `try_insert_into_existing_chunk` refuses to merge into a strict /
   exports-only entry chunk when doing so would expose new exports. Our
   pre-grouping strip routes modules directly into entry chunks via the bits
   bucketing pass, bypassing that gate.
3. **Cycle / dependency / side-effect validation.** Rollup's atom-level
   analysis tracks `correlatedAtoms` and side-effect placement; our
   per-module strip ignores these. Manual chunks, runtime helpers, and
   shared static-dep chains can all close static-import cycles after a
   strip.
4. **DFS evaluation-order preservation.** Stripping a dynamic-entry bit
   from a module that has side effects can hoist those side effects into a
   different chunk and change the observable evaluation order.

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

#### `test262: language/module-code/verify-dfs.js`

**Symptom:** `Expected SameValue("B", "A") to be true` — modules evaluate
out of DFS order.

**Shape:**
```js
// verify-dfs.js
import './verify-dfs-a_FIXTURE.js';
import './verify-dfs-b_FIXTURE.js';

// a fixture
check(import('./verify-dfs-b_FIXTURE.js'));
evaluated('A');

// b fixture
evaluated('B');
```

**Why:** B is treated as already-loaded for the dynamic import from A. The
strip changes chunk placement / import edges enough that B's `evaluated('B')`
side effect runs before A's, violating the spec'd DFS order.

### B. `preserveEntrySignatures` guard

#### `crates/rolldown/tests/rolldown/issues/9049/_config.json` — `extended-preserve-entry-signatures-strict` variant

**Symptom:** `Expected 4 chunks but got 3`.

**Why:** Shared services have their dynamic-route bits stripped (both routes'
only importer is main, which statically reaches the services), leaving
`bits = {main}`. Bucketing places them directly in main's entry chunk,
bypassing the strict-signature check that lives inside
`try_insert_into_existing_chunk`.

The non-strict variant of this fixture passes correctly with the optimization.

#### `crates/rolldown/tests/rolldown/issues/4895`

**Symptom:** `strict.js` gains an unintended export. Output shifts from:

```js
import { t as shared } from "./lib2.js";
export { unused };
```

to:

```js
const shared = "shared";
export { shared as t, unused };
```

**Why:** `lib2.js` originally had bits for both the strict entry and its
dynamic-import entry. The strip removes the dynamic bit (lib2 is reachable
statically from the strict entry). Single-bit `lib2` then lands in strict's
entry chunk via bucketing, exposing `shared as t` — a violation of
`preserveEntrySignatures: 'strict'`.

#### `crates/rolldown/tests/rolldown/misc/preserve_entry_signature/exports-only`

**Symptom:** `main2.js` gains an unintended export. Same class as 4895 but
for `preserveEntrySignatures: 'exports-only'` (which behaves like strict
when the entry already has exports).

```js
// before
export { unused };

// after
export { value as t, unused };
```

### C. Cycle / dependency / side-effect validation

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

#### `crates/rolldown/tests/rolldown/tree_shaking/issue_4682`

**Symptom:** Chunk graph gains an inverted edge — `static.js` ends up
importing `main.js` while `main.js` still imports `static.js`.

```js
// static.js
import "./main.js";
```

**Why:** Strip moves part of the static-dep side-effect chain into main's
chunk while `static.js` still depends on something main holds. Per-module
stripping doesn't model Rollup's atom / correlated-side-effect grouping, so
side-effect placement order is no longer preserved.

#### `crates/rolldown/tests/rolldown/issues/8920_2`

**Symptom:** `entry-2.js should host the runtime module` — runtime ends up
in a separate `chunk.js` instead of co-located with the dominator.

**Why:** Adjacent to category C (side-effect / placement). The strip moves
`node2` out of the `{entry-2, node1-dyn}` common chunk into entry-2's entry
chunk. That changes the runtime-consumer chunk set seen by
`rehome_runtime_module`'s dominator search; the new set has no dominator,
so a fresh runtime chunk is emitted.

The runtime module itself is already excluded from the strip — necessary
but not sufficient, because *consumers* of runtime helpers (modules with
non-empty `depended_runtime_helper`) still get repositioned.

## Suggested fix order

1. **Track awaited dynamic imports** in `import_record` and run the analysis
   twice, only stripping bits that are loaded non-awaited. Closes test262
   TLA + DFS cases.
2. **Plumb `preserveEntrySignatures` into `apply_strip`.** Skip a strip if
   the resulting bitset would land the module directly in a strict /
   exports-only entry chunk that doesn't already export the relevant
   bindings. Closes 9049-strict, 4895, exports-only.
3. **Cycle-aware strip.** Either run a cycle-detection-and-revert pass on
   the projected post-strip chunk graph, or restrict stripping to modules
   whose static-dep closure does not pass through a chunk statically
   downstream of the move target. Closes 9225, 4682, 8920_2.
4. **Runtime-consumer awareness.** Once (3) lands, re-evaluate whether
   modules with `depended_runtime_helper` need additional handling beyond
   the cycle guard, or whether the dominator search just needs to be
   re-run on the post-strip topology.

Until at least (1)–(3) land, the flag should default off (or these eight
cases must be triaged individually — they are not test-too-strict
situations; in particular 9225's cycle and the test262 TLA hang are real
correctness regressions, not snapshot drift).
