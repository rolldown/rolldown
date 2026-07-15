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

| Data               | Description                                                                                          |
| ------------------ | ---------------------------------------------------------------------------------------------------- |
| `clientId`         | Unique identifier for the browser tab                                                                |
| `executed_modules` | Modules the browser has actually executed (used for HMR boundary computation and lazy-patch pruning) |

Session lifecycle:

- A client session is exactly `clientId → ClientSession { executed_modules }` in `SharedClients` on the `DevEngine`
- Created **implicitly** on the first `hmr:module-registered` message for that `clientId` (dev server → `registerModules` → napi `register_modules`); removed via `removeClient` when the client's websocket disconnects
- `executed_modules` is a **grow-only** set of **stable ids** — and it includes proxy ids like `src/foo.js?rolldown-lazy=1`, since the lazy chunk re-registers the proxy under its stable id
- On the runtime side, `registerModule` feeds a debounced batcher that coalesces ids into one `hmr:module-registered` message; the messenger queues messages until the websocket opens
- The special client id `"rolldown-tests"` is treated as having executed everything (Rust-level tests bypass per-client gating; only the browser E2E playgrounds exercise the `executed_modules` path)

### Fetched vs Executed

These are distinct concepts at different scopes:

- **Fetched** (session-level): The browser sent a `/lazy` request for this proxy module. The server has compiled the actual module and its dependencies. All clients now receive the fetched template.

- **Executed** (client-level): The browser has actually run the module's code. Used to prune the lazy patch for a specific client and to gate HMR propagation.

A module can be fetched but not executed by a particular client (e.g., Client A fetched it, Client B hasn't navigated to that route yet).

Per-client outcomes when a fetched lazy module is later edited (see "Editing a fetched lazy module"):

- Client that executed it → a real `Patch`, or `FullReload` if no HMR boundary accepts the change
- Client that never executed it → an effectively-empty `Patch` whose code is just `__rolldown_runtime__.applyUpdates([]);` (**not** `Noop` — `Noop` is produced only when the changed file maps to no graph module at all, e.g. an unfetched lazy file)

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

When multiple lazy entries share common dependencies, two cooperating layers prevent duplicate execution:

```
Entry
├── import('./lazy-a')  ← lazy boundary
│   └── shared.js (sync dep)
└── import('./lazy-b')  ← lazy boundary
    └── shared.js (sync dep)
```

1. **Server-side pruning**: when collecting the sync deps for a lazy patch, the server skips modules whose stable id is in the requesting client's `executed_modules` (populated via `hmr:module-registered`)
2. **Runtime dedup flag**: lazy chunks are rendered with `dedup_module_initializer: true`, which appends a truthy third argument to every module wrapper — `__rolldown_runtime__.createEsmInitializer(stableId, factory, 1)` / `createCjsInitializer(...)` — and the runtime skips the factory when the id is already registered

The server-side race window still exists (two `/lazy` requests in rapid succession, before the first patch's `hmr:module-registered` arrives, produce overlapping chunks — TODO in `hmr_stage.rs`), but the runtime dedup flag makes it harmless: `shared.js` appears in both chunks yet executes once.

**HMR patches deliberately omit the dedup flag** (`dedup_module_initializer: false`): a patch's whole point is to re-execute the module body and publish new exports, so deduping would silently drop updates. Code comments mark the flag as a workaround pending a runtime dispose/re-execute API.

### Link-Stage-Synthesized Exports (JSON, text, base64, dataurl)

Modules whose exports are synthesized at link time are **broken inside lazy chunks** (and HMR patches): JSON/text/base64/dataurl modules are scanned as a bare expression statement with `ExportsKind::None`, and the `export default` is materialized only by `NormalizeLazyExportsPass` — which the lazy/HMR render path never runs (it renders pristine scan-time AST clones). The lazy chunk registers them as `registerModule(id, {})`, so importers see **empty exports on first lazy load**; after the background rebuild + a page refresh the full build applies the transform and the same import works. No playground fixture covers this yet.

### CSS

CSS bundling was removed from rolldown (#4271), and the lazy boundary is created without loading the target — so `import('./style.css')` builds fine and the hard error (`Bundling CSS is no longer supported`) is **deferred to the first `/lazy` request**: HTTP 500, catchable rejection at the consumer's `await import()`.

### Assets

Rolldown core has no built-in asset handling: an extension outside the default `module_types` map is read as UTF-8 and parsed as JS, so a binary file statically imported in a lazy subtree fails the lazy compile at request time. Asset imports inside a lazy subtree work only when a plugin converts them to JS in its `load` hook (as the dev server's ported `vite:asset` plugin does — see "Emitted assets").

### Sourcemaps

Only `sourcemap: 'inline'` works for lazy chunks. The `/lazy` payload is a plain `String` through the whole chain (`HmrStage` → `DevEngine` → napi → middleware), with no field for a separate map file: with `'file'`/`true`, the code gains a `//# sourceMappingURL=lazy_compile_{n}.js.map` comment but the map asset is discarded server-side (the comment dangles); `'hidden'` drops the map silently. HMR patches, by contrast, carry their map through `HmrPatch { sourcemap, sourcemap_filename }` and the dev server serves both patch and map from the in-memory file store — so an identical `sourcemap: 'file'` config works for HMR edits and silently breaks for lazy chunks. This path currently has no test coverage.

## Implementation Details

### Module ID Format

**IMPORTANT**: All runtime module lookups use **stable IDs** (`stable_id`), relative paths from the cwd (e.g., `src/module.js`), computed via `ModuleId::new(id).stabilize(cwd)` with the cwd captured in the `build_start` hook.

This ensures consistency between:

- The stub's `delete __rolldown_runtime__.modules[stableProxyId]` / `loadExports(stableProxyId)` calls
- Compiled module wrappers: `createEsmInitializer("src/module.js", ...)` (inside the wrapper body, `registerModule` / `createModuleHotContext` receive the id via the `__rolldown_module_id__` parameter)
- `import.meta.hot.accept("src/dep.js", ...)` specifiers
- `applyUpdates([["src/boundary.js", "src/acceptedVia.js"]])` boundaries

Absolute paths survive in exactly two places: the `/@vite/lazy?id=` query value (the proxy id) and the fetched template's `import($MODULE_ID)` (used for resolution).

The templates are rendered with **four placeholders**, each substituted as a serde_json-quoted JS string literal (so the templates contain bare `$PLACEHOLDER` tokens and Windows backslash paths are escaped correctly, #9102):

| Placeholder               | Value                              | Used by                                    |
| ------------------------- | ---------------------------------- | ------------------------------------------ |
| `$PROXY_MODULE_ID`        | absolute path + `?rolldown-lazy=1` | stub — the `/@vite/lazy?id=` request       |
| `$STABLE_PROXY_MODULE_ID` | stable id + `?rolldown-lazy=1`     | stub — module-map delete + `loadExports`   |
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

When `load` is called for a proxy module:

1. Only ids present in `lazy_entries` are served at all — any other `?rolldown-lazy=1` id falls through to `Ok(None)`
2. If in `fetched_entries` → return fetched template; otherwise → return stub template

**Security gate — unknown module rejection (#9969)**: the id passed to `compileEntry` / `compile_lazy_entry` is treated purely as a lookup key into the build cache, never resolved as a filesystem path. An id not present from a prior build is rejected in `HmrStage::compile_lazy_entry` with `Lazy entry module not found in cache` — so a malicious `/@vite/lazy` request cannot bundle an arbitrary file (analogous to Vite's `server.fs.strict`; pinned by `packages/rolldown/tests/dev/dev-lazy-compile.test.ts`). Note the ordering: `DevEngine::compile_lazy_entry` calls `mark_as_fetched` unconditionally **before** this validation, so an unknown id still lands in `fetched_entries` (harmless, but worth knowing when debugging).

### Lazy Chunk Rendering

`Bundler::compile_lazy_entry(module_id, client_id, executed_modules, next_patch_id)` → `HmrStage::compile_lazy_entry` (the `client_id` param is unused at this layer — per-client tailoring comes solely from `executed_modules`):

1. Look the proxy up in the module cache (the #9969 gate), then run `ScanMode::Partial([proxy's resolved id])`
2. `collect_sync_dependencies_for_client` walks the proxy's static deps plus the proxy's own dynamic import, **stopping** at any module whose stable id is in the client's `executed_modules`; external modules are dropped and the rest sorted by id
3. Each module is rendered by `HmrAstFinalizer` into an initializer wrapper:

   ```js
   var init_foo = __rolldown_runtime__.createEsmInitializer(
     'src/foo.js',
     function (__rolldown_module_id__) {
       try {
         // registerModule/createModuleHotContext use __rolldown_module_id__;
         // ESM exports are published as:
         // var __rolldown_exports__ = __rolldown_runtime__.__exportAll({ ... })
       } finally {
       }
     },
     1,
   ); // trailing `1` = dedup flag, lazy chunks only
   ```

   (CJS modules use `createCjsInitializer` with `__rolldown_exports__` / `__rolldown_module__` params.)

4. Dynamic imports inside the rendered modules are rewritten:
   - importee id contains `?rolldown-lazy=1` (a nested lazy proxy) → ``import(`/@vite/lazy?id=${encodeURIComponent(absProxyId)}&clientId=${__rolldown_runtime__.clientId}`).then(() => __rolldown_runtime__.loadExports("<stableProxyId>"))`` — partial bundles have no separately bundled proxy chunk, so the proxy's top-level `'rolldown:exports'` export would be lost inside the init wrapper; reading it back via `loadExports` preserves the surface `__unwrap_lazy_compilation_entry` expects (pinned by the nested-dynamic-import spec)
   - ordinary `import()` → `Promise.resolve().then(() => __rolldown_runtime__.loadExports("<stableId>"))`, prefixed with the importee's `init_x()` call when it is in the same patch
5. The chunk ends with the proxy entry's `init_xxx()` call (this re-registers the proxy id with the real initializer — what the stub's step 3 awaits)
6. The result is post-processed under a synthetic name `lazy_compile_{n}.js` (n from the dev engine's `next_invalidate_patch_id` counter, shared with `hmr.invalidate` patches — **not** the coordinator's `hmr_patch_{n}.js` counter) and returned as a plain JS string

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
- **Init errors are catchable (#9981)**: an error thrown while the lazy module initializes rejects the re-registered proxy's `'rolldown:exports'` promise, hence the stub's `lazyExports`, hence the consumer's `await import(...)` — try/catch works, and without a handler exactly one `unhandledrejection` fires. Pinned on both the **cold** path (first `/lazy` compile) and the **warm** path (fetched proxy after rebuild + reload) by the lazy-init-error specs (#9975 added the original failing spec; #9981 rewrote and split it)
- **Runtime `loadExports` miss** does not throw — it warns and returns `{}`
- The one remaining panic: calling `compile_lazy_entry` before any bundle has been built

### ClientId

- Generated by the HMR runtime at init via `crypto.randomUUID()` (before any lazy import can run), appended to the websocket URL (`?clientId=...` — the dev server closes clientId-less sockets with code 1008) and interpolated into every `/@vite/lazy` request via `__rolldown_runtime__.clientId`
- Its only role in lazy compilation is **per-client patch pruning**: the engine looks up that client's `executed_modules` so the returned chunk omits modules the client already ran. Nothing is "routed" — the compiled code returns synchronously in the HTTP response
- An unknown `clientId` silently degrades to an empty executed set (the full dependency closure is returned)

### Editing a Fetched Lazy Module

After `/lazy`, the real module and its sync deps are ordinary watched graph modules (thanks to the `update_watch_paths()` step), and an edit flows through the standard watch → per-client HMR path:

- The fetched proxy participates as a plain **non-accepting** importer (`hmr_info.deps` comes only from `import.meta.hot.accept`, never from dynamic imports). Editing a non-self-accepting lazy module therefore propagates proxy → dynamic importer → `NoBoundary` → `FullReload` for every client that executed it; the dev task auto-upgrades to `HmrRebuild`, and the dev server defers the reload until the rebuild output lands (canceling it if the rebuild errors, #9903). Pinned by the shared-module spec's watch/auto-reload test
- If the lazy module self-accepts (`import.meta.hot.accept()`), executed clients get a normal per-client `Patch` instead
- Clients that never executed the module get the effectively-empty `applyUpdates([])` patch (see "Fetched vs Executed")

## End-to-End Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│ 1. INITIAL BUILD                                                        │
├─────────────────────────────────────────────────────────────────────────┤
│  - Entry + sync dependencies compiled normally                          │
│  - Dynamic imports (import()) → replaced with proxy modules             │
│  - Proxy module ID: /abs/path/module.js?rolldown-lazy=1                 │
│  - Proxy contains STUB template (fetches via /@vite/lazy endpoint)      │
│  - Proxy exports 'rolldown:exports' promise                             │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 2. BROWSER LOADS INITIAL BUNDLE                                         │
├─────────────────────────────────────────────────────────────────────────┤
│  - Runtime initializes; clientId = crypto.randomUUID()                  │
│  - Proxy registers under its STABLE id:                                 │
│      registerModule("src/module.js?rolldown-lazy=1", { exports })       │
│  - Stub template is ready to fetch on demand                            │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 3. USER CODE HITS: import('./lazy-module')                              │
├─────────────────────────────────────────────────────────────────────────┤
│  - Proxy module executes (stub template)                                │
│  - Deletes its own runtime registration (so the chunk can re-register)  │
│  - Fetches: /@vite/lazy?id=/abs/path/lazy-module.js?rolldown-lazy=1&clientId=xxx
│  - Browser waits on the promise                                         │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 4. DEV SERVER RECEIVES /lazy REQUEST                                    │
├─────────────────────────────────────────────────────────────────────────┤
│  - Calls DevEngine.compileEntry(proxyModuleId, clientId)                │
│  - Engine looks up the client's executed_modules                        │
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
│  - Actual module + sync deps compiled — minus the client's              │
│    already-executed modules                                             │
│  - Modules rendered as createEsm/CjsInitializer(stableId, fn, 1)        │
│    (dedup flag); chunk ends with the proxy entry's init call            │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 6. RETURN COMPILED JS TO BROWSER                                        │
├─────────────────────────────────────────────────────────────────────────┤
│  - Assets emitted during the compile already delivered via              │
│    onAdditionalAssets (#9815)                                           │
│  - Response is a single JS string (code only — no sourcemap channel)    │
│  - Browser loads it as an ES module; initializers register each module  │
│  - Entry init call re-registers the proxy id with the real initializer  │
│  - Stub resolves: loadExports(stableProxyId)['rolldown:exports']        │
│  - Original import() promise resolves (or rejects catchably, #9981)     │
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

- `createEsmInitializer(stableId, factory[, dedup])` / `createCjsInitializer(...)` — inside the wrapper, `registerModule(__rolldown_module_id__, { exports })` and `createModuleHotContext(__rolldown_module_id__)` receive the id via the wrapper parameter (the main-bundle path still emits stable-id string literals)
- `loadExports(stableId)`
- `import.meta.hot.accept(stableId, callback)`
- `applyUpdates([[boundaryStableId, acceptedViaStableId]])`

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

**Problem**: The HMR finalizer was generating invalid JavaScript:

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

- **Proxy module**: `/abs/path/module.js?rolldown-lazy=1` (loaded initially, contains stub/fetched code; registers at runtime under its stable id `src/module.js?rolldown-lazy=1`)
- **Actual module**: `/abs/path/module.js` (compiled on-demand, contains real code; registers under `src/module.js`)

The flow is:

1. Initial build creates proxy at `module.js?rolldown-lazy=1` with stub template
2. User triggers lazy load → `/@vite/lazy?id=...?rolldown-lazy=1`
3. DevEngine marks proxy as fetched
4. Partial scan from proxy → plugin returns fetched template
5. Fetched template imports actual module → triggers compilation
6. The lazy chunk re-registers the proxy id with the real initializer, and the stub resolves via `loadExports("src/module.js?rolldown-lazy=1")['rolldown:exports']`
7. After the background rebuild, both proxy (fetched) and actual module are in the output

### Issue 8: Proxy-ID Creation Must Be Idempotent (#9439)

**Problem**: With an alias plugin present, `ctx.resolve` re-entered the lazy plugin's `resolve_id`, appending `?rolldown-lazy=1` twice. The doubled suffix desynced the proxy id from the stub template's `delete modules[$STABLE_PROXY_MODULE_ID]` invalidation key, so the real module's exports never registered (`mod.foo` came back undefined — regression vitejs/vite#22454).

**Solution**: Before appending the marker, check whether the resolved id already ends with `?rolldown-lazy=1` and reuse it. Pinned by the aliased-import spec.

### Issue 9: Fetched Template Must Read Exports From the Registry (#9132)

**Problem**: The fetched template originally returned the dynamic import's namespace object. When a shared lazy module landed in a common chunk, chunk-level renaming minified the export names and the namespace lookup yielded `undefined`.

**Solution**: `await import($MODULE_ID)` for side effects only, then `return __rolldown_runtime__.loadExports($STABLE_MODULE_ID)` — the runtime registry preserves original export names. Pinned by the shared-module spec.

### Issue 10: Init Errors Must Reject the Consumer's Promise (#9981)

**Problem**: An error thrown while a lazily-compiled module initialized escaped as an unhandled rejection instead of surfacing at the consumer's `await import(...)`.

**Solution**: The stub template awaits the **re-registered proxy's own `'rolldown:exports'` promise** (`return await loadExports($STABLE_PROXY_MODULE_ID)['rolldown:exports']`) rather than handing back a namespace — so a rejection anywhere in the chain rejects `lazyExports` and the consumer's import promise. Pinned by the two lazy-init-error specs (cold and warm paths).

## Implementation Notes

### Naming Convention for Injected Helpers

The lazy compilation plugin injects helper functions with double-underscore prefix (e.g., `__unwrap_lazy_compilation_entry`). This is a standard convention for internal/reserved identifiers in JavaScript bundlers and should not conflict with user code.

### Directive Prologue Handling

The injected helper function is inserted **after** any directive prologues (e.g., `"use strict"`) to preserve their semantics. The plugin counts leading string literal expression statements and inserts the helper after them. The helper is only injected when at least one dynamic import in the module was actually wrapped.

## Test Coverage

E2E playground: `packages/test-dev-server/tests/playground/lazy-compilation/` (one dev server config with `experimental.devMode.lazy: true` + an alias plugin):

| Spec                        | Pins                                                                              |
| --------------------------- | --------------------------------------------------------------------------------- |
| `basic`                     | lazy module arrives as two separate JS requests (proxy chunk + real chunk)        |
| `aliased-import`            | idempotent proxy-id creation under alias re-entrancy (vite#22454)                 |
| `emitted-asset`             | assets emitted during lazy compile are servable on first load (vite#22596)        |
| `lazy-init-error`           | init errors catchable with try/catch — cold and warm paths (#9975/#9981)          |
| `lazy-init-error-unhandled` | exactly one `unhandledrejection` without a handler — cold and warm paths          |
| `nested-dynamic-import`     | nested lazy `import()` inside a lazy chunk resolves on first click                |
| `shared-module`             | export-name preservation in shared chunks (#9132) + watch/auto-reload after fetch |

Several specs use `retry: 0` because the bugs only reproduce on the first interaction with a fresh server. Unit test: `packages/rolldown/tests/dev/dev-lazy-compile.test.ts` pins the unknown-id rejection (#9969).

## Files Changed (Reference)

For future debugging, these files handle lazy compilation:

### Core Plugin

1. **`crates/rolldown_plugin_lazy_compilation/src/lazy_compilation_plugin.rs`** - Plugin with `resolve_id`, `load`, and `transform_ast` hooks; `LazyCompilationContext` with fetched-state tracking; `render_proxy_template`
2. **`crates/rolldown_plugin_lazy_compilation/src/runtime_injector.rs`** - AST visitor for wrapping dynamic imports and generating `__unwrap_lazy_compilation_entry`
3. **`crates/rolldown_plugin_lazy_compilation/src/proxy-module-template.js`** - Stub template (not fetched)
4. **`crates/rolldown_plugin_lazy_compilation/src/proxy-module-template-fetched.js`** - Fetched template
5. **`crates/rolldown/src/utils/apply_inner_plugins.rs`** - registers the plugin when `experimental.dev_mode.lazy == true`

### Dev Engine

6. **`crates/rolldown_dev/src/dev_engine.rs`** - `compile_lazy_entry()` (executed_modules lookup, mark-as-fetched, asset delivery, `notify_module_changed()`), client sessions
7. **`crates/rolldown_dev/src/types/coordinator_msg.rs`** - `ModuleChanged` message variant
8. **`crates/rolldown_dev/src/bundle_coordinator.rs`** - Handles `ModuleChanged` (`update_watch_paths` + rebuild), state machine
9. **`crates/rolldown_binding/src/binding_dev_engine.rs`** - napi surface (`compile_entry`, `register_modules`, `remove_client`)

### HMR/Build

10. **`crates/rolldown/src/hmr/hmr_stage.rs`** - `compile_lazy_entry()`: cache gate, partial scan, per-client dep collection, chunk rendering
11. **`crates/rolldown/src/hmr/hmr_ast_finalizer.rs`** + **`impl_traverse_for_hmr_ast_finalizer.rs`** - initializer wrappers, dedup flag, dynamic-import rewrites (incl. the `/@vite/lazy` rewrite), computed-property exports
12. **`crates/rolldown/src/hmr/utils.rs`** - register-module / hot-context statement builders (`__rolldown_module_id__` param)
13. **`crates/rolldown/src/bundler/impl_bundler_hmr.rs`** - `Bundler::compile_lazy_entry` entry point
14. **`crates/rolldown_plugin_hmr/src/runtime/runtime-extra-dev-common.js`** - browser runtime: `createEsm/CjsInitializer` (dedup gate), `registerModule`, `loadExports`, module-registered batching

### Reference Dev Server (Vite full bundle mode, vendored at `packages/test-dev-server/vite`)

15. **`packages/vite/src/node/server/middlewares/triggerLazyBundling.ts`** - the `/@vite/lazy` middleware (500 on error, `application/javascript` on success)
16. **`packages/vite/src/node/server/bundledDev.ts`** - `triggerLazyBundling` (`devEngine.compileEntry`), `onAdditionalAssets` storage, rebuild/reload handling
17. **`packages/vite/src/node/plugins/asset.ts`** - the bundled-dev branch resolves asset imports eagerly at `load`

## References

- [design.md](./design.md) — goals, scope, and key design decisions
- Current implementation: `crates/rolldown_plugin_lazy_compilation/`
- Dev engine: `crates/rolldown_dev/` (see also `internal-docs/dev-engine/`)
- Example: `examples/lazy-compilation/`
