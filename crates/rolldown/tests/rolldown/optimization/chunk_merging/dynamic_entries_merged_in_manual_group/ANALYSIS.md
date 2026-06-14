# TLA blocks facade-chunk elimination — `dynamic_entries_merged_in_manual_group`

Investigation of why Rolldown keeps three re-export proxy chunks for this fixture
while Rollup rewrites the dynamic imports to load the merged chunk directly.

## TL;DR

The entry uses **top-level await** (`await Promise.all([...])`). Rolldown's TLA
bailout is a **graph-wide `.any()` kill switch** that disables the *entire*
`merge_common_chunks` block — including facade elimination — the moment any module
in the graph touches TLA. Rollup treats facade elimination as **completely
independent of TLA**, and only blocks chunk *merging* in the one case that can
actually deadlock: a module that is in a **dependency cycle** *and* has an
**awaiting TLA dynamic importer**. This fixture has no cycle, so Rollup merges and
drops the proxies; Rolldown bails purely because TLA exists somewhere in the graph.

## The fixture

```js
// main.js — TLA at the top level flags the whole graph
const [a, b, c] = await Promise.all([
  import('./a.js'),
  import('./b.js'),
  import('./c.js'),
]);

// a.js / b.js / c.js — leaf modules, no cycles
export const A = "a-payload"; // etc.
```

A manual code-splitting group merges `a/b/c` into one `shared-abc` chunk:

```json
{ "manualCodeSplitting": { "groups": [
  { "name": "shared-abc", "test": "[abc]\\.js$", "priority": 30 }
] } }
```

## What each bundler produces

**Rolldown (current, buggy) — 5 chunks.** The merged `shared-abc.js` is created, but
three re-export proxy chunks survive and `main.js` still imports them:

```js
// a.js   (also b.js, c.js)
import { r as A } from "./shared-abc.js";
export { A };

// main.js keeps:  import("./a.js"), import("./b.js"), import("./c.js")
```

**Rollup (and the target shape for Rolldown) — 2 chunks.** Dynamic imports rewritten
to load the merged chunk directly and pick the export off its namespace:

```js
// main.js
const [a, b, c] = await Promise.all([
  import("./shared-abc.js").then(n => n.r),
  import("./shared-abc.js").then(n => n.n),
  import("./shared-abc.js").then(n => n.t),
]);
```

## The exact failure chain in Rolldown

In `crates/rolldown/src/stages/generate_stage/code_splitting.rs`:

1. **`code_splitting.rs:848-849`** — the gate is computed graph-wide:
   ```rust
   let has_tla_or_tla_dependency =
     self.link_output.metas.iter().any(|meta| meta.is_tla_or_contains_tla_dependency);
   ```
2. **`:850-851`** — `allow_merge_common_chunks = is_merge_common_chunks_enabled() && !has_tla_or_tla_dependency`.
   `merge_common_chunks` defaults to `true` (`rolldown_common/.../experimental_options.rs:24`),
   so **TLA is the sole reason it flips to `false`** here.
3. **`main.js`'s `await Promise.all(...)`** sets `is_tla_or_contains_tla_dependency`
   via `compute_tla.rs:142` (`find_tla_source` returns `Some` for the module that
   directly uses TLA). That single `true` makes the graph-wide `.any()` fire.
4. **`:952-968`** — the whole optimization block is skipped, including the pass that
   would fix this:
   ```rust
   if allow_merge_common_chunks {          // ← false
     temp_chunk_graph.calc_chunk_dependencies(...);
     self.try_insert_common_module_to_exist_chunk(...);
     self.optimize_facade_entry_chunks(...);   // ← the pass that drops a/b/c proxies
   }
   ```

`apply_manual_code_splitting` (`:874`) runs *unconditionally*, so `a/b/c`'s **content**
is still merged into `shared-abc`. But the dynamic **entry chunks** `a.js`/`b.js`/`c.js`
(created in `init_entry_point`) are only removed by `optimize_facade_entry_chunks`
(`chunk_optimizer.rs:948`), which marks the proxy `PostChunkOptimizationOperation::Removed`
(`:1047-1054`) and re-points the dynamic import via
`common_chunk_exported_facade_chunk_namespace` (`:1069-1073`). That pass never runs →
the proxies render.

There is also a **structural coupling**: `optimize_facade_entry_chunks` consumes
`temp_chunk_graph`, which is only *populated* inside the `allow_merge_common_chunks`
branches (`:922-934`). So the call cannot simply be hoisted out of the gate — the data
it depends on isn't built when TLA is present.

## How Rollup handles TLA when optimizing chunks

Rollup keeps two concerns that Rolldown has fused **cleanly separated**.

### (A) Dynamic-import rewriting / facade elimination — never consults TLA

`setDynamicImportResolutions` (`rollup/src/Chunk.ts` ~1405-1447) resolves every dynamic
import to `(facadeChunk || chunk)` and, when there is no strict facade, appends
`.then(n => n.exportName)` to grab the specific export from the merged chunk's namespace.
Whether a facade chunk even exists is decided structurally in `generateFacades`
(`canModuleBeFacade`) — TLA is not an input to either step.

### (B) Chunk *merging* — TLA blocks it only on real deadlock risk

Two narrow guards:

- `getModulesWithDependentEntriesAndHandleTLACycles`
  (`rollup/src/utils/chunkAssignment.ts` ~457-474) forces a module into its own chunk
  **only when**
  ```js
  module.cycles.size > 0 && module.includedTopLevelAwaitingDynamicImporters.size > 0
  ```
  i.e. it must be in a dependency **cycle** *and* be dynamically imported by a module
  that **awaits** it. That is the genuine TLA deadlock shape.
- `removeUnnecessaryDependentEntries` (~544-564) guards the "already-loaded"
  optimization with a separate `awaitedAlreadyLoadedAtomsByEntry` mask — the direct
  analog of Rolldown's `avoid_redundant_chunk_loads`.

In this fixture `a/b/c` are leaf `const` modules with **no cycle**, so guard (A) does
not apply and (B) does not trigger → Rollup merges them and rewrites the dynamic imports.

## The core divergence

| Concern | Rolldown | Rollup |
|---|---|---|
| Facade elimination (drop proxy, rewrite `import()`) | Behind the **same** `allow_merge_common_chunks` gate → killed by graph-wide TLA | **Decoupled** from TLA entirely |
| TLA detection scope | **Graph-wide** `metas.iter().any(...)` (`code_splitting.rs:849`) | **Per-module**, only meaningful combined with a cycle |
| Condition that blocks a merge | *any* TLA anywhere in the graph | `cycle ∧ awaiting TLA dynamic importer` |
| Avoid-redundant-loads | Killed by the same graph-wide flag | Kept, guarded narrowly by the `awaited…` mask |

Facade elimination here is **provably TLA-safe**: the proxy `a.js` statically imports
`shared-abc`, so `import('./a.js')` already loads and awaits `shared-abc` before its
namespace resolves; rewriting to `import('./shared-abc.js').then(n => n.r)` does the same
load+await before `.then` runs. The proxy is pure indirection — removing it changes
nothing about evaluation or await order. Rolldown's gate is simply too coarse: it shares
one switch between this safe transform and the genuinely TLA-sensitive *merge* transforms.

## Fix directions

Rolldown **already has the right primitive** — it just isn't used at this gate.

1. **Make the gate per-chunk, reusing the existing precedent.** `chunk_optimizer.rs:1404`
   already defines `chunk_has_tla_or_tla_dependency(chunk_idx)` and `runtime_target_is_tla_safe`
   (`:1411`), used today to let `try_merge_runtime_chunk` proceed per-chunk despite TLA
   elsewhere. Apply the same per-chunk reasoning to facade elimination instead of the
   graph-wide `.any()`. Smallest, lowest-risk change; this fixture has no cycle, so the
   per-chunk check passes and the proxies drop.
2. **Decouple facade elimination from `allow_merge_common_chunks` outright** (closest to
   Rollup). Facade elimination is safe under TLA, so run it regardless — but this requires
   `temp_chunk_graph` to be built even when merging is disabled (`code_splitting.rs:887-934`),
   since `optimize_facade_entry_chunks` depends on it.
3. **Mirror Rollup's actual deadlock condition.** Suppress merges only when a module is
   both in a cycle and has an awaiting-TLA dynamic importer (Rolldown would need a
   `cycles` / `includedTopLevelAwaitingDynamicImporters` equivalent). Most precise, largest
   change, matches Rollup 1:1.

Expected result after the fix: the snapshot collapses from 5 chunks to 2, with `main.js`
emitting `import("./shared-abc.js").then(n => n.r)` (and `.n` / `.t`) for each target.

## Key references

- Rolldown gate: `crates/rolldown/src/stages/generate_stage/code_splitting.rs:848-851`, `:952-968`
- Facade-elimination pass: `crates/rolldown/src/stages/generate_stage/chunk_optimizer.rs:948`
- Existing per-chunk TLA precedent: `chunk_optimizer.rs:1404` (`chunk_has_tla_or_tla_dependency`), `:1411` (`runtime_target_is_tla_safe`)
- TLA propagation: `crates/rolldown/src/stages/link_stage/compute_tla.rs:140-142`
- `merge_common_chunks` default `true`: `crates/rolldown_common/src/inner_bundler_options/types/experimental_options.rs:24`
- Rollup facade/dynamic-import rewrite: `rollup/src/Chunk.ts` (`setDynamicImportResolutions` ~1405-1447, `generateFacades`)
- Rollup TLA merge guards: `rollup/src/utils/chunkAssignment.ts` (`getModulesWithDependentEntriesAndHandleTLACycles` ~457-474, `removeUnnecessaryDependentEntries` ~544-564)
- Non-TLA sibling tests that *do* eliminate proxies: `dynamic_entry_merged_in_common_chunk{,2}`
