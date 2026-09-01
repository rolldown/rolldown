# Strict Execution Order

## Summary

`strictExecutionOrder` solves the module execution order violation problem in code-split bundles. When modules are grouped into chunks, their relative execution order can differ from the original module graph order, causing runtime errors (e.g., accessing a global before it's initialized). The feature wraps modules in lazy `init_xxx()` functions and calls them explicitly in dependency order, guaranteeing correctness without generating extra chunks.

Related issues: [evanw/esbuild#399](https://github.com/evanw/esbuild/issues/399), [rollup/rollup#4539](https://github.com/rollup/rollup/issues/4539)

## The Fundamental Shift

**Standard ESM:** Loading = execution. When a module is loaded (placed in a chunk and that chunk is fetched), its top-level code runs immediately. Module **placement** directly determines **execution order**. The bundler is constrained — it must arrange modules carefully within and across chunks to preserve the correct order.

**With `strictExecutionOrder`:** Loading ≠ execution. Modules are wrapped in `init_xxx()` — loading just defines the function, execution is deferred until the init function is explicitly called. The **init call graph** controls execution order, not physical placement.

This decoupling is the key insight: **with `strictExecutionOrder`, modules can be placed anywhere**. Any chunk, any position within a chunk. The only remaining constraints are ESM structural ones:

- Symbols must be importable (chunk dependency graph must allow resolution)
- Chunks must load their dependency chunks before accessing their exports

This frees the chunking algorithm from execution order constraints entirely. Chunking becomes purely about **loading performance** — what to load together, what to defer — not about correctness.

## The Problem (without `strictExecutionOrder`)

Consider:

```
init-dep.js   →  sets global.foo
run-dep.js    →  reads global.foo
entry.js      →  import './init-dep.js'; import './run-dep.js';
```

Without `strictExecutionOrder`, if `run-dep.js` and `init-dep.js` land in different chunks (or in the wrong order within the same chunk), `global.foo` is undefined when `run-dep.js` executes.

### Alternative approaches and why wrapping wins

| Approach                                      | Trade-off                                                                                             |
| --------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| Generate more common chunks to preserve order | Chunk count explodes; defeats the purpose of chunking                                                 |
| Topological sort within each chunk            | Only works intra-chunk; cross-chunk order is still broken                                             |
| Wrap modules in init functions (current)      | Slight size overhead, but correct across all chunk boundaries. Frees chunking from order constraints. |

## Current Behavior

When `strictExecutionOrder: true`:

1. **Module wrapping** — Nearly all non-CJS modules are wrapped in an `__esmMin()` initializer, regardless of whether they have side effects:

   ```js
   var init_foo = __esmMin(() => {
     // module body
   });
   ```

   The current implementation overrides `wrap_kind` to `WrapKind::Esm` broadly when strict execution order is enabled. This is intentionally aggressive — see [Current Compromise](#current-compromise) below.

2. **Explicit init calls** — Importing modules call `init_xxx()` for their dependencies in the correct order before accessing any exports.

3. **Cross-chunk plain imports skipped** — Since init calls handle side-effect ordering, the bundler no longer needs to inject plain chunk imports purely for side effects (`compute_cross_chunk_links.rs:514-517`).

4. **On-demand wrapping (experimental)** — Behind `experimental.on_demand_wrapping` (disabled by default, also disabled in dev mode). When enabled, modules that don't actually need lazy initialization can be inlined back, avoiding unnecessary wrapper overhead. This is a generate-stage optimization that downgrades `wrap_kind` for leaf modules after chunk assignment.

## Current Compromise

The current implementation wraps **all** execution-order-sensitive modules. This is a compromise — the correct behavior is to only wrap modules whose execution order is actually ambiguous. Wrapping everything was the expedient path because ambiguity analysis wasn't implemented.

Since `strictExecutionOrder` decouples placement from execution, the overhead comes from two sources:

1. **Wrapper functions** — `var init_xxx = __esmMin(() => { ... })` around every module
2. **Init call sites** — `init_xxx()` at every import point

Two orthogonal directions to reduce this overhead.

**Priority:** Direction 2 (minimizing init calls) comes first. Even with all modules wrapped, the init call count should be minimum — this is achievable with a well-understood algorithm (transitive reduction) and delivers concrete byte savings immediately. Direction 1 (selective wrapping) is conceptually harder — determining which modules can be safely unwrapped requires cross-entry consensus analysis that depends on final chunk assignment. The effort-to-impact ratio is less clear, and it can be deferred without blocking other work like [hollow chunk elimination](./app-scenario-chunking.md#hollow-chunk-elimination) which assumes all modules are wrapped.

## Direction 1: Only wrap when needed

**Goal:** Reduce the number of modules that get `__esmMin()` wrappers.

**The principle:** A module only needs wrapping when different entry points **disagree** about its relative execution order with another module in the same chunk. If all entries agree, the module can be placed in the correct position as plain code — no wrapper needed.

**Example:**

```
Entry A → a-a → x, y, z    (execution order: x, y, z)
Entry B → b-b → x, z, y    (execution order: x, z, y)
```

- `x` is always first from both entries → **stable**, no wrapping needed
- `y` before `z` (A) vs `z` before `y` (B) → **ambiguous**, wrapping needed

Only `y` and `z` need wrapping. `x` can be plain unwrapped code at the top of its chunk.

**Algorithm — consensus partial order:**

1. For each pair of modules (m1, m2) that share a chunk, collect their relative order from every entry that reaches both
2. If m1 < m2 from **all** entries → stable pair
3. If m1 < m2 from some entries but m2 < m1 from others → **ambiguous pair**
4. A module needs wrapping only if it participates in at least one ambiguous pair within its chunk

```
order(Ei)            = total execution order of modules reachable from entry Ei
consensus(m1, m2)    = consistent iff ∀ Ei, Ej that reach both: order(Ei) and order(Ej) agree on m1 vs m2
needs_wrapping(m)    = ∃ m' in same chunk: ¬consensus(m, m')
```

**Why this matters in practice:**

- Foundational modules (framework core, polyfills, shared utilities) are imported early by every entry. They always execute first with consistent relative ordering. These are often the _largest_ modules — skipping their wrappers saves the most bytes.
- Leaf modules (route-specific components) also tend to have stable ordering — they're only reached from one or a few entries.
- Ambiguous cases concentrate at the boundary where entry paths diverge — typically a small fraction of total modules.

### Pipeline constraint

Wrapping analysis runs during the **link stage**, but final chunk assignment (including manual code splitting) happens during the **generate stage**:

```
Link stage:      wrapping analysis → decides wrap_kind per module
                 ↓ wrap_kind feeds: determine_side_effects() → reference_needed_symbols() → include_statements()
Generate stage:  chunk assignment → manual code splitting → moves modules between chunks
```

Deferring wrapping entirely is not feasible — the link stage has hard dependencies on `wrap_kind` for tree-shaking and symbol resolution.

**Approach: conservative then unwrap.** Keep wrapping everything during link (as today). After chunk assignment in the generate stage, analyze which wrappers are actually needed via consensus order analysis, and downgrade `Esm → None` for stable modules. This extends the existing `on_demand_wrapping()` pass, which already downgrades `wrap_kind` in the generate stage.

### Required data

1. **Final chunk graph** — which modules share a chunk (available after `split_chunks()`)
2. **Per-entry execution order** — for each entry, the DFS traversal order of modules it reaches. The existing global `exec_order` is NOT sufficient — we need per-entry orders to detect disagreements between entries.
3. **Entry-to-module reachability** — which entries reach which modules (already available via `SplittingInfo.bits`)

### Cost

Computing per-entry orders requires a DFS per entry point — there's no shortcut to determine relative module order from an entry without traversing from that entry. For ~1000 dynamic entries, that's ~1000 DFS traversals, each O(modules reachable from that entry). Most dynamic entries reach a small subset (a lazy route + its deps), so total work is bounded.

**Pruning:**

- **Dependency-related pairs are always stable:** If m1 transitively depends on m2 (or vice versa), their order is forced — no entry can reverse it. Only unrelated module pairs can be ambiguous.
- **Filter by shared entries:** Only consider entries reaching BOTH modules (`SplittingInfo.bits` intersection). One shared entry → unambiguous by definition.
- **Early termination:** One disagreeing entry → mark as needing wrapping, move on.

---

## Direction 2: Only emit init calls when needed

**Goal:** For modules that _do_ need wrapping, minimize the number of `init_xxx()` call sites in the output.

**Current state:** Every import statement pointing to a wrapped module emits an `init_xxx()` call (`mod.rs:213-259`). Deduplication is only per-module (via `generated_init_esm_importee_ids`) — no cross-module analysis.

**Key insight:** If `a` imports `b` and `c`, and `b` already imports `c`, then `init_c()` in `a` is redundant. `init_b()` calls `init_c()` before doing anything else — that's the guarantee of `__esmMin`. The second call bails out (already initialized), but the call site still costs bytes.

**Example:**

```
a.js:  import './b.js'; import './c.js';
b.js:  import './c.js';
```

Current output:

```js
var init_a = __esmMin(() => {
  init_b();
  init_c(); // redundant — init_b() already calls init_c()
});
```

Optimal output:

```js
var init_a = __esmMin(() => {
  init_b();
});
```

**Algorithm — transitive reduction of init calls:**

For each module `m` with direct wrapped dependencies `D = {d1, d2, ..., dn}`:

1. For each `di ∈ D`, compute `reach(di)` = all modules transitively reachable through `di`'s wrapped imports
2. Minimal init set: `D' = { di ∈ D | ∀ dj ∈ D, j ≠ i: di ∉ reach(dj) }`
3. Only emit `init_di()` for `di ∈ D'`

**Examples:**

Diamond:

```
a → b, c, d       reach(b) = {d}, reach(c) = {d}
b → d              d covered by b and c → remove init_d() from a
c → d              Result: init_a calls init_b(), init_c()
```

Deep chain:

```
a → b, c, d, e     reach(b) = {c, d, e}
b → c → d → e      c, d, e all covered by b
                    Result: init_a only calls init_b()
```

**Implementation:**

- Precompute during link stage: `BitSet` per module for transitive wrapped-dependency closure
- At codegen (`mod.rs:215`): filter `D` → `D'` before emitting
- Cost: O(modules × avg_deps) in link stage; zero overhead at codegen
- Only follow edges to **wrapped** modules (unwrapped modules don't produce init calls)

**Edge cases:**

- **TLA (Top Level Await):** If `c` is TLA, `await init_c()` in `a` is redundant if `init_b()` also `await`s `init_c()` internally. This holds by construction — `b`'s TLA flag propagates from `c`.
- **Cross-chunk boundaries:** `init_b()` in chunk A still calls `init_c()` via cross-chunk import. The reduction is about the init call graph, not chunk boundaries — still valid.

---

## How Direction 1 and Direction 2 compose

The two directions are **orthogonal**:

1. Direction 2 reduces how many `init_xxx()` calls are emitted → fewer call sites per wrapper
2. Direction 1 reduces how many modules get `__esmMin()` wrappers → fewer `init_xxx` functions exist

```
Before (current compromise): N modules wrapped, each with ~K init calls
After D2:                    N modules wrapped, each with ~K' init calls (K' < K)
After D2+D1:                 M modules wrapped (M ≪ N), each with ~K' init calls
```

D2 should be implemented first — it works regardless of whether all modules are wrapped or only some, and uses a straightforward algorithm (transitive reduction via BitSet propagation). D1 can be layered on later as a further optimization, but D2 alone already delivers the minimum init call overhead that app scenario depends on.

## Impact on Chunking Strategies

### Automatic code splitting: no change needed

With automatic code splitting (`codeSplitting: true`), modules are grouped by reachability pattern (BitSet) — modules with identical entry-reachability go in the same chunk. This is structurally correct regardless of `strictExecutionOrder`. The chunk optimizer does small adjustments (facade elimination, merging common chunks into entry chunks), but the algorithm is fundamentally sound. There's nothing to rethink here.

### Manual code splitting: rethink with `strictExecutionOrder`

This is where `strictExecutionOrder` is transformative. Without it, manual code splitting groups are constrained by execution order — pulling a module into a group can violate its execution order relative to other modules in that group. The grouping algorithm must be conservative, and facade chunks are needed to maintain ordering semantics.

With `strictExecutionOrder`, **those constraints vanish**. Init calls handle execution order regardless of placement. Manual code splitting groups become purely about **what to load together for optimal loading performance**:

- Groups can aggressively consolidate modules without correctness risk
- No need to preserve execution order within or across groups
- Facade chunks become unnecessary — the init-call mechanism makes their ordering role redundant
- The grouping algorithm can focus solely on: chunk count, chunk sizes, and which modules a given route actually needs

This means `strictExecutionOrder` + manual code splitting is the combination where the size overhead of wrapping needs to be minimized (Direction 1 and 2), but the **chunking flexibility** gained is substantial. The trade-off is: pay the wrapping cost, gain full freedom in chunk organization.

### Facade chunk elimination

Current limitation: `chunk_optimizer.rs` only eliminates facades when `chunk.modules.is_empty()`. With `strictExecutionOrder`, facades that still contain an entry module whose only job is re-exporting from other chunks could also be eliminated, since init calls already enforce the right order. This is particularly relevant for manual code splitting, where groups pull modules out of dynamic entry chunks and leave behind proxy facades.

### Bundle size measurement

Need data on a real-world app (e.g., ClickUp — ~1000 dynamic imports):

- How many modules currently get wrapped vs. how many have stable ordering? (quantifies D1 impact)
- How many init calls are transitively redundant? (quantifies D2 impact)
- What's the total byte savings (raw and gzip'd)?

## Implementation Plan

### Architecture validation

The "conservative then unwrap" approach is confirmed safe and already architected for:

- **`LinkingMetadata`** has dual fields: `original_wrap_kind` (link stage decision) and `wrap_kind` (current, mutable). `update_wrap_kind(WrapKind::None)` is the API to downgrade.
- **Over-inclusion is safe and bloat-free.** Three protective layers ensure unused wrappers never emit: (1) `stmt_info_included` guards wrapper statement generation, (2) runtime helper collection only iterates included modules, (3) tree-shaking respects downgraded wrap_kind.
- **The finalizer already handles the transition.** When `wrap_kind() == WrapKind::None`: import statements are removed, symbols inlined, no wrapper generated. Pure codegen change.

### Direction 1 implementation

**Where:** Extend `on_demand_wrapping()` in `generate_stage/on_demand_wrapping.rs`, or add a new pass after it.

**Pipeline position:**

```
generate_stage/mod.rs:
  1. ensure_lazy_module_initialization_order()
  2. on_demand_wrapping()              ← existing pass (concatenation-based)
  3. [NEW] consensus_order_unwrapping() ← Direction 1
  4. [NEW] recompute_cross_chunk_links() ← required after D1
  5. merge_cjs_namespace()
  6. compute_chunk_output_exports()
  7. finalize_modules()
```

**Critical constraint:** `compute_cross_chunk_links()` runs before D1 and skips plain side-effect imports when `strictExecutionOrder` is enabled (since init calls handle ordering). If D1 then downgrades a module to `WrapKind::None`, that module becomes eager again — its execution is controlled by placement, not init calls. But the cross-chunk link setup already omitted the plain imports needed for side-effect ordering of eager modules. Without recomputation, cross-chunk side effects can be silently omitted or reordered.

**Solution:** After D1 unwraps modules, rerun the cross-chunk link computation (or at minimum, the side-effect import portion) for any chunks containing newly-unwrapped modules. This ensures plain side-effect imports are re-injected where needed. The recomputation scope is bounded — only chunks containing unwrapped modules need updating, not the entire chunk graph.

**Step-by-step:**

1. **Compute per-entry execution orders.** Reuse the DFS pattern from `js_import_order()` (`code_splitting.rs:404-433`) which already does per-entry DFS within a chunk. Extend `determine_reachable_modules_for_entry()` (`code_splitting.rs:897-925`) to record execution order alongside reachability. Store as `Vec<IndexVec<ModuleIdx, Option<u32>>>` (entry → module → order).

2. **For each chunk, identify ambiguous pairs.** For each pair of modules in the chunk:
   - Skip if one transitively depends on the other (always stable)
   - Find entries reaching both (`SplittingInfo.bits` intersection)
   - Check if all shared entries agree on relative order
   - If any disagree → both modules need wrapping

3. **Downgrade stable modules.** Call `meta.update_wrap_kind(WrapKind::None)` for modules not involved in any ambiguous pair. The finalizer and chunk export rendering already handle this correctly.

**Existing infrastructure to reuse:**

- `SplittingInfo.bits` — entry-to-module reachability (already computed)
- `js_import_order()` — per-entry DFS within a chunk (already implemented)
- `LinkingMetadata.dependencies` — pre-computed static dependency set
- `module.importers_idx` — reverse dependency set
- `update_wrap_kind()` — the downgrade API

### Direction 2 implementation

**Where:** Link stage (precompute reachability) + codegen (`module_finalizers/mod.rs:215-259`).

**Step-by-step:**

1. **Precompute transitive wrapped-dependency closure.** During link stage (after `wrap_modules()`), compute a `BitSet` per module representing all modules transitively reachable through wrapped imports.

   **Cyclic graph handling:** The wrapped-import graph can contain cycles (ESM supports circular imports). A naive reverse-topological pass assumes a DAG and will undercompute `reach()` for modules in SCCs. The algorithm must handle this:
   - **Condense SCCs first.** Compute strongly-connected components (e.g., Tarjan's). All modules within an SCC have identical reachability — they can all reach each other. Collapse each SCC into a single node in the condensed DAG.
   - **Propagate on the condensed DAG.** Process in reverse topological order of the condensed graph: `reach(scc) = ∪ {scc_j} ∪ reach(scc_j)` for each successor SCC `scc_j`.
   - **Expand back.** Each module's `reach()` = its SCC's `reach()` + all other modules in the same SCC.

   This ensures correct coverage computation even with circular dependencies. The condensed DAG is typically much smaller than the full module graph, so the overhead is minimal.

2. **Store on `LinkingMetadata`.** Add `transitive_wrapped_deps: BitSet` field.

3. **Filter at codegen.** In `transform_or_remove_import_export_stmt()` (`mod.rs:215`), before emitting `init_xxx()`, check if the importee is in `reach(dj)` for any other direct dependency `dj`. If so, skip — it's transitively covered. Replace the simple `generated_init_esm_importee_ids` HashSet check with the transitive reduction filter.

**Note:** If D1 is applied first (some modules unwrapped), D2's transitive reduction graph changes. Unwrapped intermediary modules don't produce init calls, so they break the transitive coverage chain. The `reach()` computation should only follow edges to modules that remain wrapped (check `wrap_kind() == WrapKind::Esm`). If D1 runs in generate stage and D2's reachability was precomputed in link stage, the reachability may need recomputation. Alternatively, compute D2's reachability after D1 in the generate stage.

### Test coverage gaps

The existing test suite (20 tests) covers wrapping mechanics well but lacks:

- Tests with **multiple entries reaching shared modules in different orders** (the ambiguity case for D1)
- Tests measuring **init call count** before/after transitive reduction (D2)
- Tests with **manual code splitting + strictExecutionOrder** (only `issue_5303` touches this)
- No benchmarks measuring size overhead of wrapping

New tests needed:

- Ambiguous pair detection: two entries importing shared modules in different orders
- Consensus order: modules with stable ordering across all entries → verify no wrapper
- Transitive reduction: diamond/chain dependency → verify minimal init calls
- Manual code splitting: groups pulling modules across execution order boundaries

## Unresolved Questions

- What's the right default? Currently `false`. Should it be `true` for code-split builds?
- How should `strictExecutionOrder` interact with `preserveEntrySignatures`?
- For D2: should the transitive reduction consider non-wrapped intermediary modules? (e.g., `a → unwrapped_b → c` — does this still transitively cover `c`?) If `b` is unwrapped, its code runs inline — so `c` is NOT guaranteed to be initialized before `a` accesses it. The reduction should only follow **wrapped** edges.
- Should D2's reachability be computed in link stage (before D1) or generate stage (after D1)? If link stage, it needs recomputation after D1 unwraps modules. If generate stage, it avoids the recomputation but adds generate-stage cost.

## Related

- [manual-code-splitting](../../docs/in-depth/manual-code-splitting.md)
- [automatic-code-splitting](../../docs/in-depth/automatic-code-splitting.md)
