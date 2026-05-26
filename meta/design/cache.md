# Cache

## Summary

Rolldown has several distinct cache mechanisms. The architecturally central
one is **`ScanStageCache`** — the bundler-level snapshot of the parsed module
graph that makes incremental builds and HMR possible. The others are
within-build memoization, plugin scratch state, and a JS-side store.

This doc inventories every cache, then details `ScanStageCache`: its data, the
module-identity model it depends on (`ModuleId` / `ModuleIdx` /
`module_id_to_idx`), how `ScanStageCache::merge` splices a partial scan into the
snapshot, and the complete list of readers and writers.

All file/line references are against the working tree at the time of writing
and will drift; treat them as starting points.

## Cache inventory

Counting types literally named `*Cache`, there are 14. Grouped by purpose:

### 1. Incremental-build cache

| Type             | Location                                           | Stores                                                                   |
| ---------------- | -------------------------------------------------- | ------------------------------------------------------------------------ |
| `ScanStageCache` | `crates/rolldown/src/types/scan_stage_cache.rs:20` | The module-graph snapshot + module index maps. See the rest of this doc. |

### 2. Cross-build invalidation state

Not result-caches; they persist alongside `ScanStageCache` so the next
incremental build knows what to invalidate / can answer plugin queries.

| Data                     | Location                                    | Notes                                                                                                 |
| ------------------------ | ------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `transform_dependencies` | `crates/rolldown_plugin/src/plugin_driver/` | `addWatchFile()` deps; module → files it depends on. Documented in `bundler-data-lifecycle.md`.       |
| `module_infos`           | `crates/rolldown_plugin/src/plugin_driver/` | Plugin-populated module metadata for `this.getModuleInfo`. Documented in `bundler-data-lifecycle.md`. |

### 3. Within-build memoization

| Type                                | Location                                                                         | Stores                                                                                                              |
| ----------------------------------- | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `SideEffectCache` (enum)            | `crates/rolldown/src/stages/link_stage/tree_shaking/determine_side_effects.rs:9` | `None` / `Visited` / `Cache(DeterminedSideEffects)`; a transient local memo during the link-stage side-effect walk. |
| `PackageJsonCache`                  | `crates/rolldown_plugin_vite_resolve/src/package_json_cache.rs:9`                | `side_effects_cache: FxDashMap<PathBuf, Arc<PackageJson>>`, `optional_peer_dep_cache: FxDashMap<PathBuf, Arc<…>>`.  |
| `ResolverCaches`                    | `crates/rolldown_plugin_vite_resolve/src/resolver.rs:77`                         | `package_json: PackageJsonCache`, `importer_exists: FxDashSet<String>`.                                             |
| `TsconfigCache`                     | `crates/rolldown_binding/src/transform_cache.rs:12`                              | `resolver: Arc<Resolver>`, `cache: FxDashMap<PathBuf, Arc<TsConfig>>`. NAPI-exposed (`#[napi]`).                    |
| `RawTransformOptions` `cache` field | `crates/rolldown_common/src/inner_bundler_options/types/transform_options.rs`    | tsconfig → compiled Oxc transform options.                                                                          |
| oxc_resolver internal cache         | external crate, held by the bundler-level `SharedResolver`                       | filesystem/path metadata.                                                                                           |

### 4. Plugin scratch state (in `PluginContext.meta()`)

Named `*Cache` but functionally per-build shared maps that pass data between
plugin hook invocations. All in `crates/rolldown_plugin_utils/src/`.

| Type                       | Location                        | Stores                                         |
| -------------------------- | ------------------------------- | ---------------------------------------------- |
| `AssetCache`               | `file_to_url.rs:24`             | `FxDashMap<String, String>`                    |
| `PublicAssetUrlCache`      | `public_file_to_built_url.rs:5` | `FxDashMap<String, String>`                    |
| `CSSEntriesCache`          | `constants.rs:46`               | `FxDashMap<ArcStr, ArcStr>`                    |
| `CSSModuleCache`           | `constants.rs:51`               | `FxDashMap<String, FxHashMap<String, String>>` |
| `CSSChunkCache`            | `constants.rs:82`               | `FxDashMap<ArcStr, String>`                    |
| `RemovedPureCSSFilesCache` | `constants.rs:90`               | `FxDashMap<ArcStr, Arc<OutputChunk>>`          |
| `CSSUrlCache`              | `constants.rs:95`               | `FxDashMap<String, String>`                    |

Related non-`Cache`-named structures in the same file: `ViteMetadata`,
`HTMLProxyResult`, `HTMLProxyMap`, `CSSStyles`, `PureCSSChunks`.

### 5. JS-side cache

| Type                    | Location                                                                                | Stores                                                                                                                                    |
| ----------------------- | --------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `PluginContextData`     | `packages/rolldown/src/plugin/plugin-context-data.ts:18`                                | `moduleOptionMap`, `resolveOptionsMap`, `loadModulePromiseMap`, `renderedChunkMeta`, `normalizedInputOptions`, `normalizedOutputOptions`. |
| `InvalidateJsSideCache` | `crates/rolldown_common/src/inner_bundler_options/types/invalidate_js_side_cache.rs:11` | `Arc<InvalidateJsSideCacheFn>` — a Rust-held callback into JS.                                                                            |
| `FilterExprCache`       | `crates/rolldown_binding/src/options/plugin/binding_plugin_options.rs:218`              | Pre-compiled plugin-hook filter expressions (NAPI binding, per-plugin).                                                                   |

`InvalidateJsSideCache` is wired in `crates/rolldown_binding/src/utils/normalize_binding_options.rs`; on the JS side
(`packages/rolldown/src/utils/bindingify-input-options.ts`) it is bound to
`PluginContextData.clear`. Calling it clears the JS-side `PluginContextData`.

### 6. Watch-mode filesystem cache

The `notify` crate's `RecommendedCache` is held inside the debouncer in
`crates/rolldown_fs_watcher/src/` and tracks filesystem metadata for event
debouncing.

---

## `ScanStageCache` — the incremental-build cache

### Where it lives

`ScanStageCache` is **bundler-level** data (it survives across builds). During
a build it is temporarily moved into the per-build `Bundle`, then moved back.
The move in/out is done by `with_cached_bundle` /
`with_cached_bundle_experimental` in
`crates/rolldown/src/bundler/impl_bundler_incremental_build.rs:9` / `:27`.

The two-tier model (bundler-level vs bundle-level) is documented in
[bundler-data-lifecycle.md](./bundler-data-lifecycle.md); that doc also covers cache integrity
on a failed build.

### The struct

`crates/rolldown/src/types/scan_stage_cache.rs:20`:

```rust
pub struct ScanStageCache {
  snapshot: Option<NormalizedScanStageOutput>,
  pub barrel_state: BarrelState,
  pub module_id_to_idx: FxHashMap<ModuleId, VisitState>,
  pub importers: IndexVec<ModuleIdx, Vec<ImporterRecord>>,
  pub user_defined_entry: FxHashSet<ModuleId>,
  pub module_idx_by_abs_path: FxHashMap<ArcStr, ModuleIdx>,
  pub module_idx_by_stable_id: FxHashMap<StableModuleId, ModuleIdx>,
}
```

| Field                     | Purpose                                                                                                                |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `snapshot`                | The full module graph. `None` is a legal transient state; it is `private` and accessed only through the methods below. |
| `barrel_state`            | Barrel re-export resolution state (`BarrelState`).                                                                     |
| `module_id_to_idx`        | The `ModuleId` → `ModuleIdx` registry/allocator (see "Module identity model").                                         |
| `importers`               | Reverse dependency graph: per module, who imports it.                                                                  |
| `user_defined_entry`      | The set of configured root entry `ModuleId`s.                                                                          |
| `module_idx_by_abs_path`  | Absolute-path → `ModuleIdx`, used by the watcher. Paths are slash-normalized.                                          |
| `module_idx_by_stable_id` | `StableModuleId` → `ModuleIdx`, used by HMR.                                                                           |

`module_idx_by_abs_path` and `module_idx_by_stable_id` are **derived** —
`build_module_index_maps` (`scan_stage_cache.rs:213`) clears and rebuilds both
from the snapshot whenever `set_snapshot` runs.

Snapshot accessors (`scan_stage_cache.rs`):

- `set_snapshot` (`:34`) — installs a snapshot and rebuilds the index maps.
- `get_snapshot` (`:66`) — `&NormalizedScanStageOutput`; **panics if `snapshot` is `None`**.
- `get_snapshot_mut` (`:41`) — `&mut`; **panics if `None`**.
- `take_snapshot` (`:46`) — moves the snapshot out, leaving `None`.
- `update_defer_sync_data` (`:50`) — takes the snapshot, runs `defer_sync_scan_data`, restores it on every outcome, then propagates any error.
- `merge` (`:70`) — splices a scan output into the snapshot (see below).
- `create_output` (`:229`) — produces a `NormalizedScanStageOutput` for the build to consume.

### `BundleMode`

`crates/rolldown_common/src/types/bundle_mode.rs` — decides whether the cache
is created, kept, or reused:

| Mode                   | Cache in | Cache out | Use case                                                           |
| ---------------------- | -------- | --------- | ------------------------------------------------------------------ |
| `FullBuild`            | None     | discarded | one-shot build, non-incremental watch                              |
| `IncrementalFullBuild` | fresh    | saved     | first incremental build, or dev-mode recovery after a failed build |
| `IncrementalBuild`     | existing | updated   | subsequent incremental builds                                      |

`is_full_build()` is true for `FullBuild` and `IncrementalFullBuild`;
`is_incremental()` is true for `IncrementalFullBuild` and `IncrementalBuild`.

### The snapshot: `NormalizedScanStageOutput`

`crates/rolldown/src/stages/scan_stage.rs:41`. Fields include `module_table`,
`index_ecma_ast` (parsed AST per module), `stmt_infos`, `entry_points`,
`symbol_ref_db`, `runtime`, `dynamic_import_exports_usage_map`,
`user_defined_entry_modules`, `tla_module_count`, `tla_keyword_span_map`.

`make_copy` (`scan_stage.rs:65`) clones the snapshot but clones
`symbol_ref_db` via `clone_without_scoping` (a performance optimization —
scoping is reinstated after the build).

### `ScanStageOutput` vs `NormalizedScanStageOutput`

`ScanStageOutput` (`scan_stage.rs:131`) is what the scan produces. Its
`module_table`, `index_ecma_ast`, and `stmt_infos` are `HybridIndexVec`, while
the snapshot's are dense `IndexVec`-based. The conversion happens in `merge`
(partial scan) or `try_into` (full scan).

---

## Module identity model

`ScanStageCache::merge` cannot be understood without this model.

### `ModuleId` vs `ModuleIdx`

A module has two identities:

- **`ModuleId`** — the resolved file path (+ query). Stable; the module's name.
- **`ModuleIdx`** — a small integer (newtype over `u32`). A slot number / array
  index. Permanent for the bundler session.

`Module::id()` (`crates/rolldown_common/src/module/mod.rs:33`) returns
`&ModuleId`; `Module::idx()` (`:18`) returns the `ModuleIdx` stored in the
module struct's `idx` field.

### `module_id_to_idx` — the registry / allocator

`module_id_to_idx: FxHashMap<ModuleId, VisitState>` is the single source of
truth mapping a module's name to its slot. It is monotonic: a new module is
always assigned `idx = module_id_to_idx.len()`. Slots are handed out
`0, 1, 2, …` with no gaps, and are never reused.

### `VisitState`

`crates/rolldown/src/module_loader/module_loader.rs:96`:

```rust
pub enum VisitState { Seen(ModuleIdx), Invalidate(ModuleIdx) }
```

Both variants carry the idx. The variant is a freshness flag:

- `Seen(i)` — module is up to date; the loader skips it (no re-scan).
- `Invalidate(i)` — module is stale; the loader re-scans it, reusing `i`.

### `IndexVec` / `Map` / `HybridIndexVec`

- `IndexVec<ModuleIdx, T>` — a `Vec` indexed by `ModuleIdx`. **Dense**: slot `i`
  exists for every `i` in `0..len`.
- `FxHashMap<ModuleIdx, T>` — **sparse**: holds only the keys inserted.
- `HybridIndexVec<ModuleIdx, T>` (`crates/rolldown_common/src/types/hybrid_index_vec.rs`)
  — an enum that is either `IndexVec(..)` or `Map(..)`. `Default` is the
  `IndexVec` variant.

A **full scan** produces all modules → dense `IndexVec`. A **partial scan**
produces only the changed + newly discovered modules → sparse `Map`.

### Invariants

1. A module's `ModuleIdx` is assigned exactly once (at first resolution) and
   never changes or gets reused.
2. Idxs are allocated densely from 0; the allocated set is exactly
   `0..module_id_to_idx.len()`.
3. The cache snapshot is dense and total: `module_table` and every parallel
   side-table (`index_ecma_ast`, `stmt_infos`, `symbol_ref_db` local DBs) have
   a slot for every allocated idx.
4. A partial-scan output is sparse: it contains exactly {changed} ∪ {new}
   modules. Unchanged modules are absent.
5. For a module in a partial-scan output, "new" ⟺ its idx ≥ the cache's
   current module count at merge time.
6. The module loader assigns a single `ModuleIdx` per module and uses that
   same value as the scan-output `Map` key, the `Module.idx` field, and the
   `module_id_to_idx` value (see `try_spawn_new_task`). These three are
   therefore equal for any given module.

---

## `module_id_to_idx` — update lifecycle

`module_id_to_idx` lives in `ScanStageCache`. `ModuleLoader` holds a mutable
borrow of the same cache — `cache: &'a mut ScanStageCache`
(`module_loader.rs:117`) — so the loader's writes mutate the bundler's actual
cache directly. There is no copy.

`module_id_to_idx` is updated **eagerly during the scan stage**, by the loader.
`merge` runs after the scan and **only reads** `module_id_to_idx` — it never
inserts into it.

### Write sites (all in `module_loader.rs`, during the scan)

| Site                     | Location                            | Effect                                                                                        |
| ------------------------ | ----------------------------------- | --------------------------------------------------------------------------------------------- |
| Runtime module           | `fetch_modules`, `:304`–`:308`      | `Entry::Vacant` → insert `Seen(idx)` (once).                                                  |
| Invalidate changed files | `fetch_modules`, `:348`–`:350`      | For each watcher-reported file: `Entry::Occupied` → `insert(Invalidate(idx))`. idx unchanged. |
| `Seen(idx)` arm          | `try_spawn_new_task`, `:230`–`:244` | No write; returns idx, module not re-scanned.                                                 |
| `Invalidate(idx)` arm    | `try_spawn_new_task`, `:246`–`:251` | `insert(Seen(idx))` — module is being re-scanned.                                             |
| `None`, partial scan     | `try_spawn_new_task`, `:252`–`:259` | New module: `insert(id, Seen(len))`, `len = module_id_to_idx.len()`.                          |
| `None`, full scan        | `try_spawn_new_task`, `:260`–`:264` | New module: `insert(id, Seen(alloc()))`.                                                      |

### Per-entry state machine

```
   (absent) --first resolution--> Seen(idx) --file changed--> Invalidate(idx)
                                     ^                              |
                                     |  loader re-scans the module  |
                                     +------------------------------+
```

The idx is fixed at birth; later transitions only flip the `Seen`/`Invalidate`
flag.

### Within-build ordering

In a partial scan, `fetch_modules` processes each watcher-reported file by
first flipping it to `Invalidate`, then calling `try_spawn_new_task`, which
hits the `Invalidate` arm, flips it back to `Seen`, and re-scans. The
intermediate `Invalidate` state is what forces a re-scan — a `Seen` entry would
make `try_spawn_new_task` return immediately without re-scanning. It also
dedups: once flipped back to `Seen`, importers that later resolve the same
module just return the idx.

Consequence: every module present in a scan output was registered in
`module_id_to_idx` by the loader before `merge` runs.

---

## `ScanStageCache::merge` — the write path

`scan_stage_cache.rs:70`. Signature: `merge(&mut self, scan_stage_output: ScanStageOutput) -> BuildResult<()>`.

### Callers

- `bundle.rs:256` — in `normalize_scan_stage_output_and_update_cache`, the
  non-full-scan branch.
- `hmr_stage.rs:286`, `:379`, `:621` — HMR update paths.

The full-scan build path does not call `merge`; it uses `set_snapshot` instead
(`bundle.rs:250`). All current callers pass a partial-scan output, whose
`module_table` is `HybridIndexVec::Map`; that is why `merge`'s `IndexVec` match
arm is `unreachable!()`.

### Algorithm

1. **First-build escape hatch** (`:77`–`:82`) — if `snapshot` is `None`,
   convert the whole output via `try_into` and return.
2. **Extract `modules`** (`:83`–`:92`) — the `module_table` is matched: the
   `IndexVec` arm is `unreachable!()`; the `Map` arm is collected into a `Vec`
   and **sorted by idx**. The sort places existing modules (idx < cache length)
   before new ones (idx ≥ cache length), and orders new modules ascending so
   that `push` lands each at its allocated slot.
3. **Per-module loop** (`:94`–`:158`):
   - `new_idx` is the `Map` key (indexes the scan output); `idx` is
     `module_id_to_idx[new_module.id()].idx()` (indexes the cache). By
     invariant 6 they are equal.
   - Update `module_idx_by_abs_path` (normal modules only, slash-normalized)
     and `module_idx_by_stable_id`.
   - **New module** (`new_idx ≥ cache.module_table.modules.len()`): push the
     module / AST / stmt infos / local symbol DB onto the parallel collections;
     adjust `tla_module_count` and `tla_keyword_span_map`.
   - **Existing module**: overwrite the same collections at `idx`; adjust TLA
     count by the old↔new delta; replace or remove the TLA span.
   - All payload is moved (`mem::take` / `take` / `mem::replace` / `mem::swap`)
     out of the scan output — never cloned.
4. **Merge entry points** (`:161`–`:181`) — for a matching existing entry
   point, drop `related_stmt_infos` for re-scanned modules and extend with the
   new ones; otherwise push the new entry point.
5. **Patch barrel modules** (`:184`–`:192`) — drain
   `barrel_state.resolved_barrel_modules` and write the resolved import records
   back into the cached modules.
6. **Recompute user-defined entries** (`:194`–`:208`) — start from the scan
   output's set, add back persistent configured roots
   (`self.user_defined_entry`) that still resolve to a live module. This
   rebuilds the set each build rather than extending it monotonically.

`merge` has two panic surfaces: the `module_id_to_idx[new_module.id()]` index
expression (panics on a missing key — reachable only if invariant 6 is
violated) and the `unreachable!()` arm. `Module::idx()` returns the same value
as the `module_id_to_idx` lookup and is infallible.

---

## Readers and writers

### Writers of `ScanStageCache`

| Writer                                                   | Location                                                                                | What it writes                                                                                                                                     |
| -------------------------------------------------------- | --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ScanStage::scan(scan_mode, &mut self.cache)`            | called at `bundle.rs:104`                                                               | Non-snapshot fields via the loader.                                                                                                                |
| `ModuleLoader` (`cache: &'a mut ScanStageCache`)         | `module_loader.rs:117` and methods                                                      | `module_id_to_idx`, `barrel_state` (e.g. removes `barrel_infos` on invalidate), `importers`, `user_defined_entry` (full incremental scan).         |
| `ScanStageCache::merge`                                  | `scan_stage_cache.rs:70`; called at `bundle.rs:256`, `hmr_stage.rs:286/379/621`         | `snapshot`, `module_idx_by_abs_path`, `module_idx_by_stable_id`, `barrel_state.resolved_barrel_modules` (drained), `tla_*` fields in the snapshot. |
| `ScanStageCache::set_snapshot`                           | `scan_stage_cache.rs:34`; called at `bundle.rs:250` and inside `update_defer_sync_data` | `snapshot` + rebuilds `module_idx_by_abs_path` / `module_idx_by_stable_id`.                                                                        |
| `ScanStageCache::update_defer_sync_data`                 | `scan_stage_cache.rs:50`; called at `bundle.rs:257`, `hmr_stage.rs:289/382/623`         | Takes and restores `snapshot`; `defer_sync_scan_data` mutates per-module `side_effects` inside it.                                                 |
| `ScanStageCache::create_output`                          | `scan_stage_cache.rs:229`; called at `bundle.rs:258`                                    | Mutates `snapshot.symbol_ref_db` (clones it without scoping, swaps); returns a `NormalizedScanStageOutput`.                                        |
| `merge_immutable_fields_for_cache`                       | `bundle.rs:315`, called at `bundle.rs:279`                                              | `get_snapshot_mut()`; reinstates symbol-table scoping after the link stage.                                                                        |
| `with_cached_bundle` / `with_cached_bundle_experimental` | `impl_bundler_incremental_build.rs:9` / `:27`                                           | Moves the whole `ScanStageCache` between `Bundler` and `Bundle`.                                                                                   |

### Readers of `ScanStageCache`

| Reader                 | Location                              | What it reads                                                                                                                                                        |
| ---------------------- | ------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `HmrStage`             | `hmr_stage.rs:48`, `:52`              | `get_snapshot().module_table`, `get_snapshot().index_ecma_ast`; also uses the module index maps. HMR is also a writer (it calls `merge` / `update_defer_sync_data`). |
| `ModuleLoader`         | `module_loader.rs:410`, `:983`        | `get_snapshot()` (e.g. `module_table.modules.get(..)`). Also reads `module_id_to_idx` (`:229`, `:869`), `barrel_state`, `user_defined_entry`.                        |
| `defer_sync_scan_data` | `module_loader/deferred_scan_data.rs` | Reads `module_id_to_idx` (passed as `&FxHashMap<ModuleId, VisitState>`); mutates the snapshot's per-module side effects.                                             |
| `merge`                | `scan_stage_cache.rs:70`              | Reads `module_id_to_idx` and `user_defined_entry`.                                                                                                                   |

---

## Cache integrity on a failed build

A build mutates `ScanStageCache` through several non-atomic "tear → repair"
steps; an early `?` return between a tear and its repair can leave the cache
broken for the next build. The invariant, the three torn windows
(ownership / scoping / defer-sync), and the unconditional-repair rule are
documented in [bundler-data-lifecycle.md](./bundler-data-lifecycle.md) ("Cache integrity on a
failed build"). The three fix sites — `with_cached_bundle`, `bundle_up`'s
ordering of `merge_immutable_fields_for_cache`, and `update_defer_sync_data` —
reference that section.

## Unresolved Questions

- `merge`'s `module_id_to_idx[new_module.id()]` index panics on a missing key
  and is reachable only on internal inconsistency; `Module::idx()` yields the
  same value without a fallible lookup. Whether to switch is a tracked
  follow-up (audit that no caller feeds `merge` a `Module` whose `.idx` was not
  loader-allocated).
- `merge` is a large multi-field mutation with no mid-loop `?`, but a panic
  mid-`merge` (the two surfaces above) would leave the snapshot present but
  internally inconsistent. Restoring presence does not guarantee consistency.

## Related

- [bundler-data-lifecycle](./bundler-data-lifecycle.md) — bundler-level vs
  bundle-level data, `BundleMode`, cache integrity on a failed build.
- [module-id](./module-id.md) — `ModuleId` design.
- [rust-bundler](./rust-bundler.md) — `Bundler` struct and build lifecycle.
- [watch-mode](./watch-mode.md) — watch mode, which drives partial scans.
