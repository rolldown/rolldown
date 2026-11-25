# Cache — Implementation

> The cache-integrity contract and open questions live in [design.md](./design.md).

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

Counting types literally named `*Cache`, there are 13. Grouped by purpose:

### 1. Incremental-build cache

| Type             | Location                                           | Stores                                                                   |
| ---------------- | -------------------------------------------------- | ------------------------------------------------------------------------ |
| `ScanStageCache` | `crates/rolldown/src/types/scan_stage_cache.rs:23` | The module-graph snapshot + module index maps. See the rest of this doc. |

### 2. Cross-build invalidation state

Not result-caches; they persist alongside `ScanStageCache` so the next
incremental build knows what to invalidate / can answer plugin queries.

| Data                     | Location                                    | Notes                                                                                                 |
| ------------------------ | ------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `transform_dependencies` | `crates/rolldown_plugin/src/plugin_driver/` | `addWatchFile()` deps; module → files it depends on. Documented in `bundler-data-lifecycle.md`.       |
| `module_infos`           | `crates/rolldown_plugin/src/plugin_driver/` | Plugin-populated module metadata for `this.getModuleInfo`. Documented in `bundler-data-lifecycle.md`. |

### 3. Within-build memoization

| Type                                | Location                                                                      | Stores                                                                                                             |
| ----------------------------------- | ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `PackageJsonCache`                  | `crates/rolldown_plugin_vite_resolve/src/package_json_cache.rs:9`             | `side_effects_cache: FxDashMap<PathBuf, Arc<PackageJson>>`, `optional_peer_dep_cache: FxDashMap<PathBuf, Arc<…>>`. |
| `ResolverCaches`                    | `crates/rolldown_plugin_vite_resolve/src/resolver.rs:77`                      | `package_json: PackageJsonCache`, `importer_exists: FxDashSet<String>`.                                            |
| `TsconfigCache`                     | `crates/rolldown_binding/src/transform_cache.rs:12`                           | `resolver: Arc<Resolver>`, `cache: FxDashMap<PathBuf, Arc<TsConfig>>`. NAPI-exposed (`#[napi]`).                   |
| `RawTransformOptions` `cache` field | `crates/rolldown_common/src/inner_bundler_options/types/transform_options.rs` | tsconfig → compiled Oxc transform options.                                                                         |
| oxc_resolver internal cache         | external crate, held by the bundler-level `SharedResolver`                    | filesystem/path metadata.                                                                                          |

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

| Type                    | Location                                                                                | Stores                                                                                                                                                                                                            |
| ----------------------- | --------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `PluginContextData`     | `packages/rolldown/src/plugin/plugin-context-data.ts:69`                                | `moduleOptionMap`, `resolutionOptionsMap`, `olderResolutionOptionsMap`, `resolveOptionsMap`, `loadModulePromiseMap`, `renderedChunkMeta`, `outputCodeCache`, `normalizedInputOptions`, `normalizedOutputOptions`. |
| `InvalidateJsSideCache` | `crates/rolldown_common/src/inner_bundler_options/types/invalidate_js_side_cache.rs:11` | `Arc<InvalidateJsSideCacheFn>` — a Rust-held callback into JS.                                                                                                                                                    |
| `FilterExprCache`       | `crates/rolldown_binding/src/options/plugin/binding_plugin_options.rs:218`              | Pre-compiled plugin-hook filter expressions (NAPI binding, per-plugin).                                                                                                                                           |

`InvalidateJsSideCache` is wired in `crates/rolldown_binding/src/utils/normalize_binding_options.rs`; on the JS side
(`packages/rolldown/src/utils/bindingify-input-options.ts`) it is bound to
`PluginContextData.clear`. Calling it clears `renderedChunkMeta`,
`loadModulePromiseMap`, `outputCodeCache`, `resolutionOptionsMap` and `olderResolutionOptionsMap`, not the other
`PluginContextData` maps.
`Bundle::bundle_write` and `Bundle::bundle_generate` call it on every exit, and
`Bundler::with_cached_bundle` calls it again before creating the next bundle, because a
failed scan never reaches the output stage.

`PluginContextData.moduleOptionMap` keeps the `meta` of each module for the whole bundler, and a
hook result without `meta` keeps the old one. So Rust resets it through `reset_js_module_options`
before it loads modules again (rolldown/rolldown#11223). JS keeps no `moduleSideEffects`: only
Rust keeps it (see below).

- `Bundle::scan_modules_impl` passes `None` before `buildStart` of a full scan. All options are
  cleared.
- `ScanStage::reset_changed_modules` passes the changed modules of an incremental rebuild before
  `buildStart`, so `getModuleInfo` in `buildStart` shows their start options. Without it,
  `buildStart` sees the options of the last fetch, such as the ones of the HMR stage of the same
  change.
- `ModuleLoader::fetch_modules` passes the changed modules of a partial scan. Each one goes back
  to the options it started with (`resolvedModuleOptionMap`), because the scan fetches it again
  from the resolution that created it, and its importers do not resolve it again. In an
  incremental rebuild, this second reset also drops what `buildStart` set. The HMR stage and lazy
  compilation reset only here.
- JS keeps a shallow copy of the start `meta`, as Rollup copies `meta`. A hook that changes a
  nested object of `meta` in place also changes the start options, so a reset keeps that change.

As in Rollup, only the resolution that creates a module sets its start options. Later
resolutions of the same id, `this.resolve`, and `this.load` options for a module that exists do
not change the module.

- Each JS resolution (a `resolveId` or `resolveDynamicImport` object result, or a `this.load`
  call) saves its own `meta` in `resolutionOptionsMap` under a receipt. Its `moduleSideEffects`
  goes to Rust with the resolution itself.
  The receipt goes to Rust on `HookResolveIdOutput` or the `this.load` fetch, and Rust keeps it
  as `ResolvedId::module_options_receipt`.
- When `ModuleLoader::try_spawn_new_task` creates a module (not a refetch), the module task
  calls `create_js_module_options` with that receipt before the `load` hook runs, for normal and
  external modules. JS starts the module from those options, with a copy of `meta`, and keeps
  them in `resolvedModuleOptionMap` for the reset above. In a full scan, a module without a
  receipt skips the call, because the full-scan reset already left it with no options.
- `this.resolve` returns the `meta` under its own receipt and removes it, and the
  `moduleSideEffects` of its own Rust result: the hook value, else the value from
  `reported_module_side_effects`, which is the same default value as a `getModuleInfo` read. It
  creates no module options.
- The options of resolutions that created no module (Rust `this.resolve` probes, and imports of a
  module that already exists) stay until the build ends (`PluginContextData.clear`) or a full scan
  starts. A partial scan also drops them: each `resetModuleOptions` call with ids (one per HMR
  update or lazy compile, two per incremental rebuild, none when no module changed) drops
  `olderResolutionOptionsMap` and moves
  `resolutionOptionsMap` there. A module takes its options in the scan that resolved it, so only
  unused options are dropped, and an HMR-only session keeps at most those of the previous HMR
  update. The receipt counter does not restart, so a receipt never finds the options of another
  build.

On the Rust side, the HMR stage and an incremental rebuild fetch a changed module again from its
`originative_resolved_id`. That `ResolvedId` keeps the `moduleSideEffects` of the resolution
that created the module (`ModuleTask::run_inner`).

It also keeps the `package.json` that the resolution found, and the module format that its
`type` field gave. The file can change after the resolution, so `ModuleLoader::fetch_modules`
reads it again from the same path before it fetches a changed module, also for the modules in
`pending_rescans` (`refresh_package_json` in `module_loader/resolve_utils.rs`). The HMR stage,
lazy compilation and an incremental rebuild all fetch through it. It checks that the file exists
with `metadata` of the file system, which also sees a file in a Yarn PnP zip archive. It infers
the format again from the whole id or from the id without `?query` and `#fragment`, whichever
gave the old format. When both gave it, it uses the whole id, so an id with a query from
Rolldown's resolver that started as `Unknown` stays `Unknown`.

A `package.json` that cannot be read or parsed keeps the data of the last read, and the module is
fetched as usual. The refresh never fails the scan, for two reasons:

- The refresh runs before the scan knows which modules the graph still needs. The same change can
  remove the last import of the module, and then a fresh build does not read the file at all.
- A successful scan must fetch each changed module again, because the HMR stage stamps and ships
  each of them.

A fresh build can give another result here. It fails to resolve the module when the file cannot be
parsed, and it uses the next `package.json` up when the file cannot be read.

The refresh reads the file through the `package.json` cache of the resolver, so it sees a change
only after that cache was cleared. The dev engine clears it, among other cases, when a watched
`package.json` changes (`internal-docs/dev-engine/implementation.md` §9c). Core does not watch a
`package.json`, so a plugin must watch it, for example with `addWatchFile`. A direct caller of a
partial scan clears the cache with `Bundler::clear_resolver_cache`. After the cache was cleared,
other resolutions read the file again too. A new import of a module under a `package.json` that
cannot be parsed then fails as in a fresh build.

A `package.json` that no longer exists gives no `package.json` data. Three cases are not handled:

- A `package.json` that a user adds nearer to the module is not found, because the path is the one
  that the resolution found.
- A `package.json` that a user adds after it was missing is not read again.
- When only the `package.json` changes, the module is not fetched again, so it keeps the old data
  until the module itself changes.

All three need the files that the resolution depends on, also the ones that it looked for and did
not find. Rolldown does not track them yet.

`moduleSideEffects` lives only in Rust. A hook value reaches Rust with the hook result: the
`resolveId`, `resolveDynamicImport`, `load` and `transform` wrappers send Rust only the
`moduleSideEffects` of the result, and Rust applies each value that is not null in hook order.
Rust plugins return theirs the same way.

A write to `getModuleInfo(id).moduleSideEffects` goes to `ModuleSideEffectsOverrides`
(`crates/rolldown_common/src/types/module_side_effects_overrides.rs`), a concurrent map, so a
write works while module tasks still run. The JS setter of the plugin context
(`BindingPluginContext::set_module_side_effects`) and of the chunking context
(`BindingChunkingContext::set_module_side_effects`) is one binding call. A Rust plugin writes with
`PluginContext::set_module_side_effects`.

- `BundleFactory` creates the map and passes it to each `PluginDriver`
  (`module_side_effects_overrides`), so it lasts across incremental builds, as the module infos
  do. A full build and the full scan in `Bundle::scan_modules_impl` clear it.
- A partial scan removes the values of the changed modules, in `ScanStage::scan` and in
  `ModuleLoader::fetch_modules`, together with the JS reset above. The HMR stage and lazy
  compilation reset only in `fetch_modules`.
- A write of `null` or `undefined` stores "not decided". The module is then treated as if no hook
  had given a value: the option, else `package.json`, else the statement analysis. An external
  takes the option, else `ExternalModule::side_effects_without_option`. Earlier hook and written
  values do not come back.
- A later `load` or `transform` value that is not null, from a JS or a Rust plugin, removes the
  written value (`PluginDriver::set_hook_side_effects`, called from `load_source` and
  `PluginDriver::transform`). So the last value wins, as in Rollup.
- Without `experimental.importAttributes`, the key is the raw id, so one write reaches every
  external with that raw id.
- `apply_module_side_effects_overrides` (`module_loader/module_side_effects_overrides.rs`)
  applies the written values to the normal and external modules for tree shaking. It runs after
  the scan, and again after `buildEnd` (`Bundle::end_build_phase`), on the scan output and on the
  incremental snapshot (`ScanStageCache::apply_module_side_effects_overrides`). So a write in
  `buildEnd` reaches tree shaking. The output hooks run after linking, so
  `ModuleSideEffectsOverrides::write` ignores a write after `buildEnd`. If it kept the write, the
  next partial scan would apply it to a module that did not change.

`getModuleInfo(id).moduleSideEffects` shows the declared value, as in Rollup, not the result of
the statement analysis that tree shaking uses for `true`:

- Rust keeps two values in `ModuleInfo::side_effects` (`ModuleInfoSideEffects`). `decided` is
  the last hook value (`HookSideEffects::declared`), or `None` when no hook decided.
  `without_hook` (`declared_side_effects_without_hook` in `ecma_module_view_factory.rs`) is
  `treeshake.moduleSideEffects`, else, for an external module, `false` for a Node.js built-in
  and `true` otherwise, else `package.json#sideEffects` (when Rolldown has `package.json` data
  for the id), else `true`. `treeshake: false` also shows `true`, though tree shaking is off
  there. The shown
  value is `decided`, else `without_hook` (`ModuleInfoSideEffects::shown`).
- `ModuleTask::run_inner` sets both in the placeholder info, so `load` already sees the value.
  `decided` then follows each hook value in order, from JS and Rust plugins alike: after `load`
  and after each `transform` hook (`PluginDriver::set_hook_side_effects`), and after parsing. An
  external sets both in `ExternalModuleTask::run_inner`. An info rebuilt from the module table
  keeps the earlier values.
- A written value is not stored in the info. Each read applies it
  (`ModuleSideEffectsOverrides::apply`): a value replaces `decided`, and "not decided" clears it,
  so the info shows `without_hook` and the getter does not return `null` after a write.
  `NativePluginContextImpl::get_module_info`, `ChunkingContextImpl::get_module_info` and each
  `moduleParsed` call (`PluginDriver::module_parsed`) give Rust plugins the info with the write
  applied. `DevEngine::get_module_info` applies it too.
- `ScanStage::scan` sets the start value again for the changed modules of a partial scan, together
  with the JS reset above.
- The JS getter (`transformModuleInfo` in `packages/rolldown/src/utils/transform-module-info.ts`,
  for the plugin context, the `moduleParsed` argument and the chunking context) is one binding call
  (`get_module_side_effects`) on each read, so an info object that a plugin keeps stays current.

`treeshake.moduleSideEffects` can be a JS function. It runs for a module only when Rolldown needs
its value, once per module task:

- Without a function option, `without_hook` is computed at module creation
  (`SideEffectsWithoutHook::Known`), because that is cheap and native.
- With a function option, `without_hook` is `SideEffectsWithoutHook::OptionFn`: the
  `package.json` part (`fallback`) is known, and the function has not run. The value is needed in
  two cases:
  - No hook decided the module. Tree shaking needs the option after the last `transform` hook
    (`normalize_side_effects`), as before, and a getter read before any hook decided needs it
    too.
  - A plugin writes `null` or `undefined`. The getter shows the value without hooks, and tree
    shaking uses the option, else `package.json`, else the statement analysis.
- `ModuleSideEffectsOverrides` keeps the result per module (`option_fn_result`), so the other
  readers and tree shaking reuse it. A partial scan removes it for the changed modules, together
  with their written values, because their module tasks run again. A full scan clears it.
- One overlap can call the function twice for a module: a JS read that needs the value while a
  Rust caller still awaits its own call. The first result is kept
  (`ModuleSideEffectsOverrides::set_option_fn_result`), so all readers see one value.
- Rust readers in async code call the function through `read_option_side_effects` or
  `ModuleSideEffectsOverrides::option_fn_result_or_call`: the module task, the external module
  task, `apply_module_side_effects_overrides`, and `PluginContext::get_module_side_effects`
  (async).
- The JS getter and setter run on the JS thread, so Rust cannot await the function there. They
  pass the function itself (`PluginContextData.moduleSideEffectsOption`), and the binding calls it
  at once when the value needs it (`ModuleSideEffectsOverrides::shown_or_call`). The setter does
  this after a `null` write, so tree shaking does not call the function again.
- `ChunkingContextImpl::get_module_side_effects` for Rust callers is sync and cannot call the
  function. It returns `None` in the one case that needs a call that has not run: a `null` write
  from Rust in the chunking context or in an output hook, for a module whose value a hook
  decided. After the scan, every other value is known.

Known costs (release builds, 10000 modules, measured against rolldown/rolldown#11789):

- A module created from a JS `resolveId` object result makes one more Rust → JS → Rust call before
  `load` (`TaskContext::create_js_module_options`): about 2 µs per module.
- A `moduleSideEffects` read in the chunking context (a `codeSplitting.groups` `name` function) is
  one binding call: about 0.6 µs per read. The plugin context reads showed no measurable change.

Future work for these costs (not implemented):

- Skip the creation call when the resolution has no `meta`, and do not await it.
- Cache the values per chunking pass in the chunking context.

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
[bundler-data-lifecycle.md](../bundler-data-lifecycle/implementation.md); that doc also covers cache integrity
on a failed build.

### The struct

`crates/rolldown/src/types/scan_stage_cache.rs:23`:

```rust
pub struct ScanStageCache {
  snapshot: Option<NormalizedScanStageOutput>,
  pub barrel_state: BarrelState,
  pub module_id_to_idx: FxHashMap<ModuleId, VisitState>,
  pub importers: IndexVec<ModuleIdx, Vec<ImporterRecord>>,
  pub modules_with_changed_importers: FxHashSet<ModuleIdx>,
  pub pending_rescans: Vec<ResolvedId>,
  pub user_defined_entry: FxHashSet<ModuleId>,
  pub user_defined_entry_common_dir: Option<Arc<Path>>,
  pub module_idx_by_abs_path: FxHashMap<ArcStr, ModuleIdx>,
  pub module_idx_by_stable_id: FxHashMap<StableModuleId, ModuleIdx>,
}
```

| Field                            | Purpose                                                                                                                                                                                                                                                                                                                                                              |
| -------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `snapshot`                       | The full module graph. `None` is a legal transient state; it is `private` and accessed only through the methods below.                                                                                                                                                                                                                                               |
| `barrel_state`                   | Barrel re-export resolution state (`BarrelState`).                                                                                                                                                                                                                                                                                                                   |
| `module_id_to_idx`               | The `ModuleId` → `ModuleIdx` registry/allocator (see "Module identity model").                                                                                                                                                                                                                                                                                       |
| `importers`                      | Reverse dependency graph: per module, who imports it.                                                                                                                                                                                                                                                                                                                |
| `modules_with_changed_importers` | Modules whose `importers` records were mutated by a partial scan; `merge` drains it to re-derive their materialized importer sets (see below).                                                                                                                                                                                                                       |
| `pending_rescans`                | Work queue: files of an aborted partial scan, whose mutations were reverted (`ModuleLoader::revert_partial_scan`); the next partial scan retries them so their errors keep surfacing. A lazy compile's scan retries only its own proxy (`ModuleLoader::set_retry_pending_rescans`; see [lazy-compilation/design.md](../lazy-compilation/design.md#6-repeat-visits)). |
| `user_defined_entry`             | The set of configured root entry `ModuleId`s.                                                                                                                                                                                                                                                                                                                        |
| `user_defined_entry_common_dir`  | Common directory of the entries of the last full scan, configured and emitted. The path externals a partial scan adds render relative to it.                                                                                                                                                                                                                         |
| `module_idx_by_abs_path`         | Absolute-path → `ModuleIdx`, used by the watcher. Paths are slash-normalized.                                                                                                                                                                                                                                                                                        |
| `module_idx_by_stable_id`        | `StableModuleId` → `ModuleIdx`, used by HMR.                                                                                                                                                                                                                                                                                                                         |

`module_idx_by_abs_path` and `module_idx_by_stable_id` are **derived** —
`ScanStageCache::build_module_index_maps` clears and rebuilds both
from the snapshot whenever `set_snapshot` runs.

Snapshot accessors (`scan_stage_cache.rs`):

- `set_snapshot` (`:44`) — installs a snapshot and rebuilds the index maps.
- `get_snapshot` (`:76`) — `&NormalizedScanStageOutput`; **panics if `snapshot` is `None`**.
- `get_snapshot_mut` (`:51`) — `&mut`; **panics if `None`**.
- `take_snapshot` (`:56`) — moves the snapshot out, leaving `None`.
- `apply_module_side_effects_overrides` (`:72`) — takes the snapshot, applies the plugin writes to `moduleSideEffects` to it, restores it on every outcome, then propagates any error.
- `merge` — splices a scan output into the snapshot (see below).
- `create_output` (`:313`) — produces a `NormalizedScanStageOutput` for the build to consume.

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
7. The first resolution of an id sets the kind of its module (external or not
   external), like its `ModuleIdx`. A later resolution of the same id to the
   other kind does not change the module. Instead, `check_external_conflict`
   (in the `Seen(idx)` arm of `try_spawn_new_task`) records
   `INVALID_EXTERNAL_ID` or `EXTERNAL_MODULES_CANNOT_BE_TRANSFORMED_TO_MODULES`,
   and the scan fails (#11417).

   In a partial scan, the kind of a module comes from one of two places:

   - A module that the scan re-scans or adds gets its kind from this scan.
     A changed external module is not re-scanned: `changed_resolved_ids` in
     `scan_stage.rs` marks it as invalidated, so the first resolution of its
     id in this scan sets its kind again. Thus a changed file that was
     external can become a normal module.
   - An unchanged module keeps its kind from the snapshot.

   Known gap: a partial scan also reports this error when an edit only
   switches the kind of an unchanged module. An example is an edit that
   changes the specifier of the last importer of the old kind. A fresh build
   accepts this edit. The error stays until the next full scan, because the
   dev engine runs partial scans again after a failed partial scan.

   If the kind comes from the snapshot, the error has a second help line: a
   full rebuild fixes the error if all imports already resolve the same way.
   Two fixes for the gap are possible: check the final `importers[idx]` at the
   end of the scan, or make the next scan a full scan.

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
| `Seen(idx)` arm          | `try_spawn_new_task`, `:230`–`:244` | No write. Checks the module kind (invariant 7) and returns idx. Does not re-scan the module.  |
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

`ScanStageCache::merge` in `scan_stage_cache.rs`. Signature: `merge(&mut self, scan_stage_output: ScanStageOutput, plugin_driver: &PluginDriver) -> BuildResult<()>`.

### Callers

- `Bundle::normalize_scan_stage_output_and_update_cache` — the non-full-scan
  branch.
- `HmrStage::compute_hmr_update_for_file_changes` and
  `HmrStage::render_patch` (its lazy scan) — the HMR update and lazy-compile paths.

The full-scan build path does not call `merge`; it uses `set_snapshot` instead
(`bundle.rs:250`). All current callers pass a partial-scan output, whose
`module_table` is `HybridIndexVec::Map`; that is why `merge`'s `IndexVec` match
arm is `unreachable!()`.

### Algorithm

1. **First-build escape hatch** — if `snapshot` is `None`,
   convert the whole output via `try_into` and return.
2. **Extract `modules`** — the `module_table` is matched: the
   `IndexVec` arm is `unreachable!()`; the `Map` arm is collected into a `Vec`
   and **sorted by idx**. The sort places existing modules (idx < cache length)
   before new ones (idx ≥ cache length), and orders new modules ascending so
   that `push` lands each at its allocated slot.
3. **Per-module loop**:
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
4. **Re-derive importer sets of affected cached modules** —
   drain `modules_with_changed_importers` (filled by
   `ModuleLoader::mark_module_importers_changed` next to every mutation of the
   `importers` edge list) and rebuild each listed module's materialized
   `importers`/`importers_idx`/`dynamic_importers` sets from the edge list via
   `EcmaView::rebuild_importer_sets`. The scan does this only for modules it
   produced; a cached module whose importer added/removed/re-kinded an import
   of it is refreshed here (issue #7416).
5. **Merge entry points** — collect every module present in the partial scan,
   remove those modules' references from every cached dynamic entry, then merge
   the new rows. The cleanup is global because deleting or retargeting an import
   produces no row for the old target. Drop stale entries left without call
   sites; the merge can re-add an unspanned import with empty `related_stmt_infos`.
6. **Patch barrel modules** — drain
   `barrel_state.resolved_barrel_modules` and write the resolved import records
   back into the cached modules.
7. **Recompute user-defined entries** — start from the scan
   output's set, add back persistent configured roots
   (`self.user_defined_entry`) that still resolve to a live module. This
   rebuilds the set each build rather than extending it monotonically.
8. **Refresh plugin-facing `ModuleInfo`** — for the modules re-derived in
   step 4, re-run `to_module_info` and `plugin_driver.set_module_info` so
   `this.getModuleInfo(id).importers` matches the merged graph (the scan
   refreshes it only for modules it produced).

`merge` has two panic surfaces: the `module_id_to_idx[new_module.id()]` index
expression (panics on a missing key — reachable only if invariant 6 is
violated) and the `unreachable!()` arm. `Module::idx()` returns the same value
as the `module_id_to_idx` lookup and is infallible.

---

## Readers and writers

### Writers of `ScanStageCache`

| Writer                                                   | Location                                                                                                                                          | What it writes                                                                                                                                                                                                                                         |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `ScanStage::scan(scan_mode, &mut self.cache)`            | called at `bundle.rs:104`                                                                                                                         | Non-snapshot fields via the loader.                                                                                                                                                                                                                    |
| `ModuleLoader` (`cache: &'a mut ScanStageCache`)         | `module_loader.rs:117` and methods                                                                                                                | `module_id_to_idx`, `barrel_state` (e.g. removes `barrel_infos` on invalidate), `importers`, `modules_with_changed_importers`, `user_defined_entry` (full incremental scan), `user_defined_entry_common_dir` (every full scan).                        |
| `ScanStageCache::merge`                                  | called in `Bundle::normalize_scan_stage_output_and_update_cache` and `HmrStage`                                                                   | `snapshot`, `module_idx_by_abs_path`, `module_idx_by_stable_id`, `barrel_state.resolved_barrel_modules` (drained), `modules_with_changed_importers` (drained), `tla_*` fields in the snapshot; refreshes plugin `module_infos` for re-derived modules. |
| `ModuleLoader::revert_partial_scan`                      | `module_loader.rs` scan error exit                                                                                                                | Restores `module_id_to_idx` / `importers` / `barrel_state` / importer marks to their pre-scan state and fills `pending_rescans`.                                                                                                                       |
| `ScanStageCache::set_snapshot`                           | `scan_stage_cache.rs:44`; called at `bundle.rs:250` and inside `apply_module_side_effects_overrides`                                              | `snapshot` + rebuilds `module_idx_by_abs_path` / `module_idx_by_stable_id`.                                                                                                                                                                            |
| `ScanStageCache::apply_module_side_effects_overrides`    | `scan_stage_cache.rs:72`; called in `Bundle::end_build_phase`, `Bundle::normalize_scan_stage_output_and_update_cache` and twice in `hmr_stage.rs` | Takes and restores `snapshot`; applies the plugin writes to `moduleSideEffects` to the per-module side effects inside it.                                                                                                                              |
| `ScanStageCache::create_output`                          | `scan_stage_cache.rs:313`; called at `bundle.rs:258`                                                                                              | Mutates `snapshot.symbol_ref_db` (clones it without scoping, swaps); returns a `NormalizedScanStageOutput`.                                                                                                                                            |
| `merge_immutable_fields_for_cache`                       | `bundle.rs:315`, called at `bundle.rs:279`                                                                                                        | `get_snapshot_mut()`; reinstates symbol-table scoping after the link stage.                                                                                                                                                                            |
| `with_cached_bundle` / `with_cached_bundle_experimental` | `impl_bundler_incremental_build.rs:9` / `:27`                                                                                                     | Moves the whole `ScanStageCache` between `Bundler` and `Bundle`.                                                                                                                                                                                       |

### Readers of `ScanStageCache`

| Reader                                | Location                                         | What it reads                                                                                                                                                                     |
| ------------------------------------- | ------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `HmrStage`                            | `hmr_stage.rs:48`, `:52`                         | `get_snapshot().module_table`, `get_snapshot().index_ecma_ast`; also uses the module index maps. HMR is also a writer (it calls `merge` / `apply_module_side_effects_overrides`). |
| `ModuleLoader`                        | `module_loader.rs:410`, `:983`                   | `get_snapshot()` (e.g. `module_table.modules.get(..)`). Also reads `module_id_to_idx` (`:229`, `:869`), `barrel_state`, `user_defined_entry`, `user_defined_entry_common_dir`.    |
| `apply_module_side_effects_overrides` | `module_loader/module_side_effects_overrides.rs` | Reads the plugin writes in `ModuleSideEffectsOverrides`; mutates the per-module side effects of the snapshot it gets.                                                             |
| `merge`                               | `scan_stage_cache.rs:80`                         | Reads `module_id_to_idx`, `user_defined_entry`, and `importers` (re-derives importer sets).                                                                                       |

---

## Related

- [design.md](./design.md) — cache-integrity contract and open questions
- [bundler-data-lifecycle](../bundler-data-lifecycle/implementation.md) — bundler-level vs
  bundle-level data, `BundleMode`, cache integrity on a failed build.
- [module-id](../module-id/implementation.md) — `ModuleId` design.
- [rust-bundler](../rust-bundler/implementation.md) — `Bundler` struct and build lifecycle.
- [watch-mode](../watch-mode/implementation.md) — watch mode, which drives partial scans.
