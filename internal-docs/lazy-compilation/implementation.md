# Lazy Compilation — Implementation

> Goals, scope, and key design decisions live in [design.md](./design.md).

## Data Lifecycle

### Overview

Lazy compilation involves data at two scopes:

1. **Session Scope** - Shared by all browser tabs, lives for the entire dev server lifetime
2. **Client Scope** - Per browser tab, identified by `clientId`

### Session Scope

Data shared across all connected browser tabs:

| Data              | Description                                                             |
| ----------------- | ----------------------------------------------------------------------- |
| Module Graph      | All resolved and compiled modules                                       |
| `lazy_entries`    | Set of proxy module IDs discovered during resolution                    |
| `fetched_entries` | Set of proxy modules that have been fetched via a `/@vite/lazy` request |
| Build Output      | Bundled JS files on disk/memory                                         |
| Watched Files     | Files monitored for changes                                             |

**Key behavior**: Once a lazy module is fetched by any client, all subsequent clients receive the fetched template (which imports the real module directly). The build output is refreshed after lazy compilation, so future page loads get the fetched template without needing a `/lazy` request.

### Client Scope

Data specific to each browser tab:

| Data                  | Description                                                                                                       |
| --------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `clientId`            | Unique identifier for the browser tab                                                                             |
| `shipped` (ship map)  | Module stable id → rebuild stamp of the factory copy this client holds; written only on the client's delivery ack |
| `top_level_evaluated` | Boot-evaluated map: modules the entry chunk runs at top level, with stamps; frozen at hello, never written again  |

Session lifecycle:

- A client session is exactly `clientId → ClientSession { shipped, top_level_evaluated, next_seq }` in `SharedClients` on the `DevEngine` (`crates/rolldown_dev/src/types/client_session.rs`)
- Created by `DevEngine::register_client` when Vite receives the `vite:client-connected` hello (`bundledDev.ts`); removed by `DevEngine::remove_client` when the client's websocket disconnects, which also drops that client's undelivered pending payloads
- The ship map is written only by `DevEngine::notify_payload_delivered(filename)`. Every patch and lazy chunk ends with a `__rolldown_runtime__.payloadDelivered(filename)` line appended by Vite (`payloadDeliveredAck` in `bundledDev.ts`). The client sends it back as `vite:bundled-dev:payload-delivered`, and the engine merges the matching `PendingPayload` (`crates/rolldown_dev/src/types/pending_payload.rs`, inserted by `compile_lazy_entry`) into `shipped[C]`, keeping the higher stamp per module
- The special client id `"rolldown-tests"` (`create_client_for_testing` in `dev_engine.rs`) is a plain session with an empty ship map; no delivery is ever marked, so every step ships the full affected set

The HMR model behind these records — why the server tracks possession, not execution — is in [hmr/design.md](../hmr/design.md).

### Fetched vs Held

These are distinct concepts at different scopes:

- **Fetched** (session-level): The browser sent a `/lazy` request for this proxy module. The server has compiled the actual module and its dependencies. All clients now receive the fetched template.

- **Held** (client-level): The client's ship map or boot-evaluated map says this client has the module's current copy. Used to size the lazy chunk for a specific client. Whether a module has **executed** is known only in the browser (`isExecuted` = module cache lookup), and the HMR boundary decision runs there (Vite `BundledDevHMRClient`).

A module can be fetched but not held by a particular client (e.g., Client A fetched it, Client B hasn't navigated to that route yet).

Per-client outcomes when a fetched lazy module is later edited (see "Editing a fetched lazy module"):

- Every connected client gets a push with `changedIds`; the patch carries only the modules that client lacks or holds at an older stamp (`render_hmr_patch` in `hmr_stage.rs`). The browser walk then decides the outcome: a hot update, a client no-op (no changed id executed, so the patch is never fetched), or a reload request
- `HmrUpdate::Noop` is produced when the changed set is empty and the patch would carry nothing (`render_hmr_patch`), e.g. an unfetched lazy file that maps to no graph module

### Build Output Refresh

After successful lazy compilation:

1. `DevEngine` notifies the coordinator via `ModuleChanged` (carrying the **raw proxy id**, `?rolldown-lazy=1` included)
2. Coordinator first calls `update_watch_paths()` — watch files discovered during the lazy compile would otherwise be dropped when the rebuild task starts; this step is what makes later edits to the lazy module trigger rebuilds at all
3. Coordinator queues a `Rebuild` task with the proxy id as the changed file and marks output as stale
4. The rebuild swaps the stub for the fetched template in the build output; future page loads get it directly (no `/lazy` request needed)

The raw proxy id is deliberately **not** normalized: during the partial rebuild it resolves back to itself (the resolver preserves the query), string-matches the proxy module's key in the incremental cache, and forces the proxy's `load` hook to re-run — which now returns the fetched template. Normalizing to the real module id would invalidate the wrong module and leave the cached stub proxy in place.

A successful background rebuild is **silent** to connected clients: output is swapped in place and no websocket message is sent (the running page keeps the code it got from `/lazy`). A reload fires only if a `FullReload` was already pending or the server is recovering from a previously-broadcast build error. `Rebuild` tasks never generate HMR updates and merge only with other `Rebuild`s, so the `?rolldown-lazy=1` pseudo-path can never leak into HMR-update computation — though plugins do observe it once through the `watch_change` hook.

## Known Limitations

### Shared-Module Deduplication

When multiple lazy entries share common dependencies, two cooperating gates prevent duplicate execution:

```
Entry
├── import('./lazy-a')  ← lazy boundary
│   └── shared.js (sync dep)
└── import('./lazy-b')  ← lazy boundary
    └── shared.js (sync dep)
```

1. **Server-side selection**: when collecting the sync deps for a lazy chunk, `collect_sync_dependencies_for_client` (`hmr_stage.rs`) skips modules whose current copy the requesting client holds — in its ship map (factory shipped) or its boot-evaluated map (run by the entry chunk)
2. **Runtime module-cache gate**: every module in a chunk is a `__rolldown_runtime__.registerFactory(stableId, factory)` call, and `initModule` (`runtime-extra-dev-common.js`) runs the factory only when the id is not yet in the module cache. Registering a factory twice overwrites the map entry; the module body runs once

Two `/lazy` requests in quick succession, before the first chunk's delivery ack arrives, both see an unmarked ship map and both carry `shared.js` — duplicate bytes, safe: the second registration overwrites the first and the module cache gate runs the body once.

An HMR patch re-runs a module for a different reason: the client's apply step removes the module from the module cache first (`removeModuleCache`), so `initModule` will run the factory again. The factory shape is the same in both payload kinds (see [hmr/design.md](../hmr/design.md), principle 5).

### Link-Stage-Synthesized Exports (JSON, text, base64, dataurl)

Modules whose exports are synthesized at link time are **broken inside lazy chunks** (and HMR patches): JSON/text/base64/dataurl modules are scanned as a bare expression statement with `ExportsKind::None`, and the `export default` is materialized only by the link stage's `generate_lazy_export` — which the lazy/HMR render path never runs (it renders pristine scan-time AST clones). The lazy chunk registers them with no exports holder — `registerModule(id)`, which the runtime fills in as `{ exports: {} }` — so importers see **empty exports on first lazy load**; after the background rebuild + a page refresh the full build applies the transform and the same import works. No playground fixture covers this yet.

### CSS

CSS bundling was removed from rolldown (#4271), and the lazy boundary is created without loading the target — so `import('./style.css')` builds fine and the hard error (`Bundling CSS is no longer supported`) is **deferred to the first `/lazy` request**: HTTP 500, catchable rejection at the consumer's `await import()`.

### Assets

Rolldown core has no built-in asset handling: an extension outside the default `module_types` map is read as UTF-8 and parsed as JS, so a binary file statically imported in a lazy subtree fails the lazy compile at request time. Asset imports inside a lazy subtree work only when a plugin converts them to JS in its `load` hook (as the dev server's ported `vite:asset` plugin does — see "Emitted assets").

### Sourcemaps

The engine side carries a separate map: `HmrLazyChunkOutput { code, filename, sourcemap, sourcemap_filename, carried }` (`crates/rolldown_common/src/hmr/lazy_chunk_output.rs`), and the napi layer forwards `sourcemap` / `sourcemap_filename` in `BindingLazyChunkOutput` (`binding_dev_engine.rs`). With `sourcemap: 'file'`/`true` the code gains a `//# sourceMappingURL=lazy_compile_{n}.js.map` comment, and the consumer must serve the map under `sourcemap_filename` for that comment to resolve. Vite's `triggerLazyBundling` (`bundledDev.ts`) returns only `code` and `filename` to its middleware, so only `sourcemap: 'inline'` reaches the browser for lazy chunks. HMR patches carry their map through `HmrPatch { sourcemap, sourcemap_filename }` and Vite serves both patch and map from its in-memory file store. This path currently has no test coverage.

## Implementation Details

### Module ID Format

**IMPORTANT**: All runtime module lookups use **stable IDs** (`stable_id`), relative paths from the cwd (e.g., `src/module.js`), computed via `ModuleId::new(id).stabilize(cwd)` with the cwd captured in the `build_start` hook.

This ensures consistency between:

- The `requestLazy("src/module.js", …)` call and the fetched template's `loadExports($STABLE_MODULE_ID)`
- Compiled module wrappers: `registerFactory("src/module.js", function (__rolldown_module_id__) { … })` (inside the wrapper body, `registerModule` / `createModuleHotContext` receive the id via the `__rolldown_module_id__` parameter)
- `import.meta.hot.accept("src/dep.js", ...)` specifiers
- `changedIds` in the HMR push and the `registerGraph` rows the client walk reads

Absolute paths survive in exactly two places: the `/@vite/lazy?id=` query value (the proxy id) and the fetched template's `import($MODULE_ID)` (used for resolution).

`render_proxy_template` (`lazy_compilation_plugin.rs`) substitutes **four placeholders**, each as a serde_json-quoted JS string literal (so the templates contain bare `$PLACEHOLDER` tokens and Windows backslash paths are escaped correctly, #9102). The stub template is an empty `export {};` and uses none of them; only the fetched template references placeholders:

| Placeholder               | Value                              | Used by                                    |
| ------------------------- | ---------------------------------- | ------------------------------------------ |
| `$PROXY_MODULE_ID`        | absolute path + `?rolldown-lazy=1` | no template                                |
| `$STABLE_PROXY_MODULE_ID` | stable id + `?rolldown-lazy=1`     | no template                                |
| `$MODULE_ID`              | absolute path (query stripped)     | fetched — `import($MODULE_ID)`             |
| `$STABLE_MODULE_ID`       | stable id                          | fetched — `loadExports($STABLE_MODULE_ID)` |

`render_proxy_template` replaces `$MODULE_ID` **last**, because the other three placeholder names contain `MODULE_ID` as a substring.

### Fetched State Tracking

The `LazyCompilationPlugin` maintains two sets in `LazyCompilationContext` (shared with the `DevEngine` via `plugin.context()`):

- `lazy_entries` - All proxy module IDs created during resolution
- `fetched_entries` - Proxy module IDs that have been fetched (requested at runtime via `/lazy`)

When `resolve_id` is called for a dynamic import:

1. If the importer is a **fetched proxy** (`?rolldown-lazy=1` + in `fetched_entries`) → return `None` (skip proxy creation, resolve to actual module)
2. Otherwise → resolve the specifier via `ctx.resolve` (`skip_self: true`, forwarding `args.custom`) and append `?rolldown-lazy=1`. The append is **idempotent** (#9439): `ctx.resolve` can re-enter other plugins' resolve hooks (e.g. an alias plugin), so if the resolved id already ends with the marker it is reused — a doubled suffix would desync the proxy id from the runtime invalidation key in the stub template (regression vitejs/vite#22454, pinned by the aliased-import spec)

Re-resolution of a known proxy id (any import kind — e.g. a dev server resolving the stub id as an entry to serve a lazy compilation request) is claimed by the lazy plugin itself: if the specifier ends with `?rolldown-lazy=1` and is present in `lazy_entries`, it resolves to itself. Unknown proxy ids fall through and stay unresolvable (cf. the #9969 gate below). User-land `resolve_id` hooks never see proxy ids at all — the `PluginDriver` skips them for `?rolldown-lazy=1` specifiers — because a virtual-module plugin would not recognize its own id with the query appended and nothing else can resolve a virtual id (regression vitejs/vite#23124, pinned by `packages/rolldown/tests/dev/dev-lazy-compile.test.ts`).

When `load` is called for a proxy module:

1. Only ids present in `lazy_entries` are served at all — any other `?rolldown-lazy=1` id falls through to `Ok(None)`
2. If in `fetched_entries` → return fetched template; otherwise → return stub template
3. User-land build hooks (`resolve_id`, `load`, `transform`, `transform_ast`, `module_parsed`) are skipped for proxy ids (`?rolldown-lazy=1`) so plugins only see real modules; the lazy plugin itself still runs to serve the stub/fetched template.

**Security gate — unknown module rejection (#9969)**: the id passed to `compileEntry` / `compile_lazy_entry` is treated purely as a lookup key into the build cache, never resolved as a filesystem path. An id not present from a prior build is rejected in `HmrStage::compile_lazy_entry` with `Lazy entry module not found in cache` — so a malicious `/@vite/lazy` request cannot bundle an arbitrary file (analogous to Vite's `server.fs.strict`; pinned by `packages/rolldown/tests/dev/dev-lazy-compile.test.ts`). Note the ordering: `DevEngine::compile_lazy_entry` calls `mark_as_fetched` unconditionally **before** this validation, so an unknown id still lands in `fetched_entries` (harmless, but worth knowing when debugging).

### Lazy Chunk Rendering

`Bundler::compile_lazy_entry(module_id, client_id, shipped, evaluated, stamp_table, next_hmr_patch_id)` (`impl_bundler_hmr.rs`) → `HmrStage::compile_lazy_entry(module_id, client_id, shipped, evaluated, stamp_table)` (the `client_id` param is unused at this layer — per-client tailoring comes solely from the two maps):

1. Look the proxy up in the module cache (the #9969 gate), then run `ScanMode::Partial([proxy's resolved id])`
2. `collect_sync_dependencies_for_client` walks the proxy's static deps plus the proxy's own dynamic import, **stopping** at any module whose current copy the client holds: its stable id is in `shipped` or `evaluated` with a stamp that `stamp_table.is_stale` reports as current; external modules are dropped and the rest sorted by id
3. Each module is rendered by `HmrAstFinalizer` into a factory registration (`impl_traverse_for_hmr_ast_finalizer.rs`):

   ```js
   __rolldown_runtime__.registerFactory('src/foo.js', function (__rolldown_module_id__) {
     try {
       // registerModule/createModuleHotContext use __rolldown_module_id__;
       // ESM exports are published as:
       // var __rolldown_exports__ = __rolldown_runtime__.__exportAll({ ... })
     } finally {
     }
   });
   ```

   (CJS modules get `var __rolldown_module__ = { exports: {} }` / `var __rolldown_exports__ = __rolldown_module__.exports` locals at the top of the body.) The same shape serves lazy chunks and HMR patches; re-execution is decided by the module cache, not by the wrapper.

4. Dynamic imports inside the rendered modules are rewritten:
   - importee id contains `?rolldown-lazy=1` (a nested lazy proxy) → ``__rolldown_runtime__.requestLazy("<stableRealId>", () => import(`/@vite/lazy?id=<absProxyId, percent-encoded>&clientId=${__rolldown_runtime__.clientId}`))`` — the same shape the full build emits, built by `create_request_lazy_call` in `crates/rolldown/src/hmr/utils.rs` (pinned by the nested-dynamic-import spec). The id is encoded at compile time so the emitted code never references `encodeURIComponent`, which user code in the importer's scope could shadow
   - ordinary `import()` → `(__rolldown_runtime__.initModule("<stableId>"), Promise.resolve().then(() => __rolldown_runtime__.loadExports("<stableId>")))` (`try_rewrite_dynamic_import` in `hmr_ast_finalizer.rs`); `initModule` returns at once for a module already in the module cache
5. The chunk starts with one `registerGraph(delta)` statement (`render_register_graph_source`, `module_graph_delta.rs`) and then carries registrations only — no execute-entry tail. `requestLazy` runs the module once the chunk has evaluated, so a throw from the module body reaches the importer's `await import(...)` instead of becoming a floating rejection inside the proxy's async wrapper
6. The result is post-processed under a synthetic name `lazy_compile_{n}.js` (n from the engine's `next_hmr_patch_id` counter, the same counter that names `hmr_patch_{n}.js`, so the two payload kinds never collide as pending-payload keys — see the field doc in `dev_engine.rs`) and returned as `HmrLazyChunkOutput`: `code`, `filename`, the optional sourcemap, and `carried` — the `(stable id, render-time stamp)` list that `DevEngine::compile_lazy_entry` stores as the chunk's `PendingPayload`

### Emitted Assets (#9815)

Lazy compiles (like HMR patches) never run the generate stage, so assets emitted during the compile have no `onOutput` path. Instead, on success `DevEngine::compile_lazy_entry` drains `file_emitter.add_additional_files` into a `BundleOutput` and fires the `onAdditionalAssets` dev callback **before** returning the code — so the consumer can register/serve the assets (test-dev-server puts them in `memoryFiles`) before the browser requests them (fixes vitejs/vite#22596, pinned by the emitted-asset spec).

Design constraint for consumers: asset URLs must be resolved **eagerly at `load`** (`emitFile` + `getFileName`, as the dev server's Vite-style asset plugin does) — a `renderChunk`-time placeholder scheme would leak, because the lazy render path never runs `renderChunk`.

### Build Output Refresh

After successful lazy compilation, the dev engine's success branch does two things, in order:

```rust
// In DevEngine::compile_lazy_entry
if result.is_ok() {
  // 1. deliver assets emitted during the compile (before the code returns)
  if let Some(on_additional_assets) = ... { ... }
  // 2. queue the background rebuild
  self.notify_module_changed(proxy_module_id);
}
```

The coordinator handles `ModuleChanged`:

1. Call `update_watch_paths()` first (see "Data Lifecycle → Build Output Refresh" for why)
2. Queue a `TaskInput::Rebuild` with the raw proxy id as the changed file
3. Set `has_stale_bundle_output = true`
4. Schedule build if stale (runs immediately only when the coordinator is Idle/Failed; otherwise waits in the queue)

On **failure**, neither step runs: a failed lazy compile queues no rebuild and the stub template stays in the build output (but the proxy remains marked fetched). If the background rebuild itself fails, the consumer caches the error, broadcasts an error overlay to every client, and cancels any pending full reload so the page never reloads onto a broken bundle (#9903); the coordinator enters `Failed { Rebuild }` with stale output, recovered by the next file change or page access.

### Error Handling

The error contract (no longer "POC — Err or panic is fine"):

- **Unknown module id** → `Err("Lazy entry module not found in cache. module_id=...")` in `HmrStage::compile_lazy_entry`; the napi binding surfaces it as a rejected promise prefixed `Failed to compile lazy entry: ...`; the dev-server middleware answers HTTP 500 (missing `id`/`clientId` params fall through to `next()`; success sets `Content-Type: application/javascript`)
- **Init errors are catchable (#9981)**: the lazy chunk only registers factories; `requestLazy` runs `initModule` itself once the chunk has evaluated, inside the promise it hands back to the consumer. An error thrown while the lazy module initializes therefore rejects that promise, hence the consumer's `await import(...)` — try/catch works, and without a handler exactly one `unhandledrejection` fires. The rejection is memoized like a native `import()` of a throwing module; a retry could not work, since a factory registers its module before running its body. Pinned on both the **cold** path (first `/lazy` compile) and the **warm** path (fetched proxy after rebuild + reload) by the lazy-init-error specs (#9975 added the original failing spec; #9981 rewrote and split it)
- **Runtime `loadExports` miss** does not throw — it warns and returns `{}`
- The one remaining panic: calling `compile_lazy_entry` before any bundle has been built

### ClientId

- Generated by Vite's client script (`nanoid()` in `bundledDevClient.ts`) before the runtime is constructed, sent to the server in the `vite:client-connected` hello, passed to the `DevRuntime` constructor, and interpolated into every `/@vite/lazy` request via `__rolldown_runtime__.clientId`
- Its only role in lazy compilation is **per-client chunk sizing**: `DevEngine::compile_lazy_entry` copies that client's ship map and boot-evaluated map so the returned chunk omits modules the client already holds. Nothing is "routed" — the compiled code returns synchronously in the HTTP response
- An unknown `clientId` silently degrades to empty maps (`unwrap_or_default()` in `DevEngine::compile_lazy_entry`), so the full dependency closure is returned

### Editing a Fetched Lazy Module

After `/lazy`, the real module and its sync deps are ordinary watched graph modules (thanks to the `update_watch_paths()` step), and an edit flows through the standard watch → per-client HMR path:

- The server walk (`collect_client_update_superset`, `hmr_stage.rs`) climbs static and dynamic importers, but proxy importers (`?rolldown-lazy=1`) are left out of the dynamic-importer index (`rebuild_importer_sets`, `crates/rolldown_common/src/ecmascript/ecma_view.rs`), so the walk stops at the lazy module. Every connected client gets a push with `changedIds`; the patch carries what that client lacks or holds stale
- The boundary decision runs in the browser (Vite `BundledDevHMRClient`). If the lazy module self-accepts (`import.meta.hot.accept()`), a client that executed it applies a hot update. Otherwise the client walk crosses the proxy edge, finds no executed importer, and sends `vite:bundled-dev:reload-needed`; the server answers that client with a full reload once the rebuild output lands (see [hmr/design.md](../hmr/design.md), "Failure policy" and "Lazy dynamic-import HMR"). Pinned by the shared-module spec's watch/auto-reload test
- A client where no changed id executed computes a client no-op and never fetches the patch (see "Fetched vs Held")

## End-to-End Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│ 1. INITIAL BUILD                                                        │
├─────────────────────────────────────────────────────────────────────────┤
│  - Entry + sync dependencies compiled normally                          │
│  - Dynamic imports (import()) → replaced with proxy modules             │
│  - Proxy module ID: /abs/path/module.js?rolldown-lazy=1                 │
│  - Proxy contains STUB template (empty body, never executed)            │
│  - import() of a proxy → __rolldown_runtime__.requestLazy(realId, ...)  │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 2. BROWSER LOADS INITIAL BUNDLE                                         │
├─────────────────────────────────────────────────────────────────────────┤
│  - Vite's client script makes clientId (nanoid), sends the hello;       │
│    the server creates the session (DevEngine::register_client)          │
│  - The proxy's chunk is never fetched; nothing registers under the      │
│    proxy id (a factory-less cache entry would confuse the HMR walk)     │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 3. USER CODE HITS: import('./lazy-module')                              │
├─────────────────────────────────────────────────────────────────────────┤
│  - requestLazy("src/lazy-module.js", fetchChunk) memoizes one promise   │
│    per real module id (dedup across importers)                          │
│  - Factory already registered on this client → initModule, no request   │
│  - Otherwise fetchChunk():                                              │
│      /@vite/lazy?id=<encoded proxy id>&clientId=xxx                    │
│  - Browser waits on the memoized promise                                │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 4. DEV SERVER RECEIVES /lazy REQUEST                                    │
├─────────────────────────────────────────────────────────────────────────┤
│  - Calls DevEngine.compileEntry(proxyModuleId, clientId)                │
│  - Engine copies the client's ship map + boot-evaluated map             │
│  - Marks proxy as FETCHED in LazyCompilationContext                     │
│  - Rejects ids not in the module cache (security gate, #9969)           │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 5. PARTIAL SCAN + RENDER                                                │
├─────────────────────────────────────────────────────────────────────────┤
│  - ScanMode::Partial([proxyModuleId])                                   │
│  - Plugin's load hook sees proxy is fetched → returns FETCHED template  │
│  - Fetched template: import("/abs/path/lazy-module.js")                 │
│  - resolve_id sees importer is a fetched proxy → returns None           │
│  - Actual module + sync deps compiled — minus modules the client        │
│    already holds (ship map / boot-evaluated map)                        │
│  - registerGraph rows first, then registerFactory(stableId, fn) per     │
│    module; registrations only, no execute-entry tail                    │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 6. RETURN COMPILED JS TO BROWSER                                        │
├─────────────────────────────────────────────────────────────────────────┤
│  - Assets emitted during the compile already delivered via              │
│    onAdditionalAssets (#9815)                                           │
│  - Response is a single JS string (code only — no sourcemap channel)    │
│  - Browser loads it as an ES module; initializers register each module  │
│  - requestLazy then runs initModule("src/lazy-module.js") itself        │
│  - Original import() promise resolves with the module's exports         │
│    (or rejects catchably, #9981)                                        │
│  - Vite appends payloadDelivered(filename); the client's ack updates    │
│    the ship map (DevEngine::notify_payload_delivered)                   │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 7. BUILD OUTPUT REFRESH (Background)                                    │
├─────────────────────────────────────────────────────────────────────────┤
│  - DevEngine sends CoordinatorMsg::ModuleChanged { proxyModuleId }      │
│  - Coordinator: update_watch_paths() → queue Rebuild → mark stale       │
│  - Rebuild updates build output with fetched template                   │
│  - Silent to connected clients; future page loads skip /lazy            │
└─────────────────────────────────────────────────────────────────────────┘
```

## Lessons Learned

### Issue 1: Module ID Consistency is Critical

**Problem**: The proxy module, compiled module, and HMR runtime must use the same ID format for module lookups to work.

**Solution**: Use **stable IDs** (`stable_id`, relative paths from cwd) consistently in the runtime:

- `registerFactory(stableId, factory)` — inside the wrapper, `registerModule(__rolldown_module_id__, { exports })` and `createModuleHotContext(__rolldown_module_id__)` receive the id via the wrapper parameter (the main-bundle path still emits stable-id string literals)
- `loadExports(stableId)` / `initModule(stableId)`
- `import.meta.hot.accept(stableId, callback)`
- `registerGraph` rows and `changedIds` in the HMR push

The lazy compilation plugin computes the stable ID in its `load` hook using the `cwd` obtained from the `build_start` hook.

### Issue 2: Proxy Content Must Change After Fetch

**Problem**: The initial lazy load worked correctly, but on page refresh:

- Build output still contained the stub template
- Stub tried to fetch `/lazy` again
- But the actual module was never included in the returned code

**Root cause**: The proxy module content never changed after being fetched. The plugin always returned the same stub template.

**Solution**: Implement two-state proxy modules:

1. Add `fetched_entries` set to `LazyCompilationContext`
2. Mark proxy as fetched before compilation: `lazy_ctx.mark_as_fetched(&proxy_module_id)`
3. In `load` hook, check state and return appropriate template:
   ```rust
   let template = if self.fetched_entries.contains(args.id) {
     include_str!("./proxy-module-template-fetched.js")
   } else {
     include_str!("./proxy-module-template.js")
   };
   ```

### Issue 3: Fetched Proxy Must Not Create Self-Referencing Proxy

**Problem**: After marking proxy as fetched, the fetched template's `import($MODULE_ID)` was being intercepted by `resolve_id` hook, which created ANOTHER proxy for the same module - causing infinite recursion.

**Solution**: In `resolve_id`, skip proxy creation when the importer is a fetched proxy:

```rust
if let Some(importer) = args.importer {
  if importer.contains("?rolldown-lazy=1") && self.fetched_entries.contains(importer) {
    return Ok(None);  // Let normal resolution happen
  }
}
```

This allows the fetched template's dynamic import to resolve to the actual module.

### Issue 4: Build Output Must Update After Lazy Compilation

**Problem**: After the first lazy load, the build output on disk still had the stub template. Page refresh would show the stub again, requiring another `/lazy` request.

**Solution**: Notify the coordinator to trigger a rebuild after successful lazy compilation:

```rust
// In DevEngine::compile_lazy_entry
if result.is_ok() {
  // (assets delivered via on_additional_assets first — see "Emitted Assets")
  self.notify_module_changed(proxy_module_id);
}
```

The notification deliberately carries the raw proxy id (`?rolldown-lazy=1` included) — it is the correct incremental-cache invalidation key, since the module whose content changed is the _proxy_ (stub → fetched template), not the real module. See "Build Output Refresh".

### Issue 5: Non-Identifier Export Names Need Computed Property Syntax

**Problem**: The HMR finalizer was generating invalid JavaScript for export names that are not identifiers (`'rolldown:exports'` was the proxy contract before `requestLazy`; the rule applies to any such name):

```js
// INVALID - colon in identifier
var __rolldown_exports__ = __rolldown_runtime__.__exportAll({ rolldown:exports: () => lazyExports });
```

**Solution**: Use `is_validate_identifier_name()` to detect non-identifier export names and use computed property syntax:

```rust
let computed = !is_validate_identifier_name(exported.as_str());
self.ast_factory.make_lazy_export_property(exported, expr, computed)
```

This generates valid JavaScript:

```js
// VALID - computed property
var __rolldown_exports__ = __rolldown_runtime__.__exportAll({
  ['rolldown:exports']: () => lazyExports,
});
```

### Issue 6: Multiple Code Paths Need Updating

**Problem**: There were TWO implementations of `rewrite_hot_accept_call_deps`:

1. `HmrAstFinalizer` (for HMR patches)
2. `ScopeHoistingFinalizer` (for regular builds with dev mode)

Only updating one left the other using `stable_id`.

**Solution**: Always search for all implementations when changing behavior. Use `grep` to find all occurrences.

### Issue 7: Proxy vs Actual Module IDs

The lazy compilation plugin creates two distinct module IDs:

- **Proxy module**: `/abs/path/module.js?rolldown-lazy=1` (a server-side graph node holding the stub/fetched template; its chunk is never fetched and nothing registers under its stable id `src/module.js?rolldown-lazy=1`)
- **Actual module**: `/abs/path/module.js` (compiled on-demand, contains real code; registers under `src/module.js`)

The flow is:

1. Initial build creates proxy at `module.js?rolldown-lazy=1` with stub template
2. User triggers lazy load → `/@vite/lazy?id=...?rolldown-lazy=1`
3. DevEngine marks proxy as fetched
4. Partial scan from proxy → plugin returns fetched template
5. Fetched template imports actual module → triggers compilation
6. The lazy chunk registers the actual module's factory under `src/module.js`; `requestLazy("src/module.js", …)` then runs it and resolves the importer's promise
7. After the background rebuild, both proxy (fetched) and actual module are in the output

### Issue 8: Proxy-ID Creation Must Be Idempotent (#9439)

**Problem**: With an alias plugin present, `ctx.resolve` re-entered the lazy plugin's `resolve_id`, appending `?rolldown-lazy=1` twice. The doubled suffix desynced the proxy id from the id the real module registers under, so the importer never saw the real exports (`mod.foo` came back undefined — regression vitejs/vite#22454).

**Solution**: Before appending the marker, check whether the resolved id already ends with `?rolldown-lazy=1` and reuse it. Pinned by the aliased-import spec.

### Issue 9: Fetched Template Must Read Exports From the Registry (#9132)

**Problem**: The fetched template originally returned the dynamic import's namespace object. When a shared lazy module landed in a common chunk, chunk-level renaming minified the export names and the namespace lookup yielded `undefined`.

**Solution**: read the exports from the runtime registry by stable id, which preserves original export names. Today `requestLazy` resolves with `initModule("<stableRealId>")` directly; the fetched template's body is never executed and its `import($MODULE_ID)` only roots the partial scan. Pinned by the shared-module spec.

### Issue 10: Init Errors Must Reject the Consumer's Promise (#9981)

**Problem**: An error thrown while a lazily-compiled module initialized escaped as an unhandled rejection instead of surfacing at the consumer's `await import(...)`.

**Solution**: the lazy chunk carries no execute-entry tail. `requestLazy` runs `initModule` itself after `fetchChunk` resolves, inside the promise it returns, so a throw from the module body rejects the consumer's import promise instead of floating out of an async wrapper. Pinned by the two lazy-init-error specs (cold and warm paths).

### Issue 11: `export * as ns from` Is Not `export * from`

**Problem**: `export * as ns from './dep'` and `export * from './dep'` are the same oxc AST node (`ExportAllDeclaration`), distinguished only by whether `exported` is set. The HMR finalizer ignored that field and rendered both as a star re-export — `__reExport(__rolldown_exports__, import_dep)` — so the re-exporting module's namespace object never carried `ns`, and every consumer read `undefined`. Only the module wrappers were affected (lazy chunks and HMR patches); the scope-hoisted build resolves the same source correctly.

**Solution**: When `exported` is present, bind the importee's `loadExports` result under that single name in the namespace object (`{ ns: () => import_dep }`, computed when the name is not a valid identifier) and emit no `__reExport`. Pinned by `crates/rolldown/tests/rolldown/topics/hmr/export_star_as/`.

### Issue 12: Re-exports From Externals Must Keep a Real Import (#10478)

**Problem**: The dev runtime registry only holds modules this build wrapped, so an external is never in it — `loadExports('<external id>')` warns `Module <id> not found` and returns `{}`. The plain-import arm of the HMR finalizer knew this and emitted a real `import * as X from 'ext'` hoisted outside the wrapper; the three re-export arms (`export * from`, `export * as ns from`, `export { x } from`) did not, and asked the registry instead. Every re-exported name read as `undefined`, silently. When the same module also imported that external, both arms named the binding through `ensure_static_import_info` but deduplicated against different sets, so both declarations were emitted under one name — and since the real import sits at chunk top level while the `var` sits inside the factory, the `var` legally shadowed it and the module's own uses of the external broke too.

**Solution**: Route all four arms through one `create_importee_binding_stmt`, which picks `loadExports` for normal modules and a real import statement for externals. The per-kind deduplication sets then stay disjoint by construction, so the shadowing `var` cannot be emitted. Pinned by `crates/rolldown/tests/rolldown/topics/hmr/reexport_external/` (HMR patch, executed) and a `dev-lazy-compile.test.ts` case (lazy chunk).

## Implementation Notes

### Directive Prologue Handling

The injected helper function is inserted **after** any directive prologues (e.g., `"use strict"`) to preserve their semantics. The plugin counts leading string literal expression statements and inserts the helper after them. The helper is only injected when at least one dynamic import in the module was actually wrapped.

## Test Coverage

E2E playground: `packages/test-dev-server/tests/playground/lazy-compilation/` (one dev server config with `experimental.devMode.lazy: true` + an alias plugin):

| Spec                        | Pins                                                                              |
| --------------------------- | --------------------------------------------------------------------------------- |
| `basic`                     | lazy module arrives in exactly one JS request (`/@vite/lazy`), no stub chunk      |
| `aliased-import`            | idempotent proxy-id creation under alias re-entrancy (vite#22454)                 |
| `emitted-asset`             | assets emitted during lazy compile are servable on first load (vite#22596)        |
| `lazy-init-error`           | init errors catchable with try/catch — cold and warm paths (#9975/#9981)          |
| `lazy-init-error-unhandled` | exactly one `unhandledrejection` without a handler — cold and warm paths          |
| `nested-dynamic-import`     | nested lazy `import()` inside a lazy chunk resolves on first click                |
| `shared-module`             | export-name preservation in shared chunks (#9132) + watch/auto-reload after fetch |

Several specs use `retry: 0` because the bugs only reproduce on the first interaction with a fresh server. Unit test: `packages/rolldown/tests/dev/dev-lazy-compile.test.ts` pins the unknown-id rejection (#9969), the `requestLazy` rewrite, and that the emitted URL does not reference `encodeURIComponent` (a user binding of that name in the importer must not break the lazy route).

## Files Changed (Reference)

For future debugging, these files handle lazy compilation:

### Core Plugin

1. **`crates/rolldown_plugin_lazy_compilation/src/lazy_compilation_plugin.rs`** - Plugin with `resolve_id` and `load` hooks; `LazyCompilationContext` with fetched-state tracking; `render_proxy_template`
2. **`crates/rolldown_plugin_lazy_compilation/src/proxy-module-template.js`** - Stub template (not fetched)
3. **`crates/rolldown_plugin_lazy_compilation/src/proxy-module-template-fetched.js`** - Fetched template
4. **`crates/rolldown/src/hmr/utils.rs`** - `create_request_lazy_call`, the one emitted shape for a lazy boundary
5. **`crates/rolldown_plugin_hmr/src/runtime/runtime-extra-dev-common.js`** - `requestLazy`, the runtime entry point
6. **`crates/rolldown/src/utils/apply_inner_plugins.rs`** - registers the plugin when `experimental.dev_mode.lazy == true`

### Dev Engine

6. **`crates/rolldown_dev/src/dev_engine.rs`** - `compile_lazy_entry()` (ship-map + boot-evaluated snapshot, mark-as-fetched, pending-payload insert, asset delivery, `notify_module_changed()`), client sessions (`register_client`, `remove_client`, `notify_payload_delivered`; types in `types/client_session.rs` and `types/pending_payload.rs`)
7. **`crates/rolldown_dev/src/types/coordinator_msg.rs`** - `ModuleChanged` message variant
8. **`crates/rolldown_dev/src/bundle_coordinator.rs`** - Handles `ModuleChanged` (`update_watch_paths` + rebuild), state machine
9. **`crates/rolldown_binding/src/binding_dev_engine.rs`** - napi surface (`compile_entry`, `register_client`, `notify_payload_delivered`, `remove_client`)

### HMR/Build

10. **`crates/rolldown/src/hmr/hmr_stage.rs`** - `compile_lazy_entry()`: cache gate, partial scan, per-client dep collection, chunk rendering
11. **`crates/rolldown/src/hmr/hmr_ast_finalizer.rs`** + **`impl_traverse_for_hmr_ast_finalizer.rs`** - `registerFactory` wrappers, dynamic-import rewrites (incl. the `/@vite/lazy` rewrite), computed-property exports
12. **`crates/rolldown/src/hmr/utils.rs`** - register-module / hot-context statement builders (`__rolldown_module_id__` param)
13. **`crates/rolldown/src/bundler/impl_bundler_hmr.rs`** - `Bundler::compile_lazy_entry` entry point
14. **`crates/rolldown_plugin_hmr/src/runtime/runtime-extra-dev-common.js`** - browser runtime: `registerFactory`, `initModule` (module-cache gate), `registerModule`, `loadExports`, `requestLazy`

### Reference Dev Server (Vite full bundle mode, vendored at `vite/` (repo root))

15. **`packages/vite/src/node/server/middlewares/triggerLazyBundling.ts`** - the `/@vite/lazy` middleware (500 on error, `application/javascript` on success)
16. **`packages/vite/src/node/server/bundledDev.ts`** - `triggerLazyBundling` (`devEngine.compileEntry`), `onAdditionalAssets` storage, rebuild/reload handling
17. **`packages/vite/src/node/plugins/asset.ts`** - the bundled-dev branch resolves asset imports eagerly at `load`

## References

- [design.md](./design.md) — goals, scope, and key design decisions
- Current implementation: `crates/rolldown_plugin_lazy_compilation/`
- Dev engine: `crates/rolldown_dev/` (see also `internal-docs/dev-engine/`)
- Example: `examples/lazy-compilation/`
