# Lazy Compilation — Implementation

> Goals, scope, and key design decisions live in [design.md](./design.md).

## Data Lifecycle

### Overview

Lazy compilation involves data in two places:

1. **Session Scope** (server) - Shared by all browser tabs, lives for the entire dev server lifetime
2. **Client State** (browser) - Per browser tab, kept by the runtime and sent in each lazy request

### Session Scope

Data shared across all connected browser tabs:

| Data              | Description                                                                                              |
| ----------------- | -------------------------------------------------------------------------------------------------------- |
| Module Graph      | All resolved and compiled modules                                                                        |
| `lazy_entries`    | Set of proxy module IDs discovered during resolution                                                     |
| `fetched_entries` | Set of proxy modules that have been fetched via a `/@vite/lazy` request                                  |
| HMR session       | `HmrSession`: epoch, generation, a stamp and a number per module, the last 64 updates (`hmr_session.rs`) |
| Build Output      | Bundled JS files on disk/memory                                                                          |
| Watched Files     | Files monitored for changes                                                                              |

**Key behavior**: Once a lazy module is fetched by any client, the graph keeps the proxy in fetched form and holds the route's modules. Pages still load every lazy route through a `/lazy` request (the full build emits `requestLazy` for every proxy), but a later compile of a fetched route skips the scan (see [design.md](./design.md#6-repeat-visits)). No compile queues a build: the build output keeps the stub until the next build of any kind takes the route from the graph.

### Client State

The server keeps nothing per tab. The runtime (`DevRuntime` in `runtime-extra-dev-common.js`) keeps what the request needs, and `encodeHave` writes it into the query of every lazy request:

| Query key | Runtime data       | Meaning                                                                                                                                         |
| --------- | ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `s`       | `epoch`            | the engine whose output the page runs, from the full-build rows                                                                                 |
| `g`       | `knownGen`         | the generation up to which the client heard of every update; every held factory is current as of it. Left out while the client does not know it |
| `e`       | `moduleCache` keys | the modules the client executed                                                                                                                 |
| `f`       | `factories` keys   | the modules whose factory the client holds and that it did not execute                                                                          |
| `n`       | `requestNonce`     | a nonce, so each URL is new                                                                                                                     |

Sets are module numbers (`nums` from the `registerGraph` rows), each sent as a bitset (`b…`); an empty set is left out. The format and the reasons are in [hmr/design.md](../hmr/design.md), principles 2 and 6. A lazy request has no `u`: nothing runs again.

### Fetched vs Held

These are distinct concepts at different scopes:

- **Fetched** (session-level): The browser sent a `/lazy` request for this proxy module. The server has compiled the actual module and its dependencies, and the graph holds them. A later compile of this route, for any client, skips the scan (see [design.md](./design.md#6-repeat-visits)).

- **Held** (client-level): The client ran the module or holds its current factory, as its request says (`e`, and `f` with a stamp at or below `g`). Used to size the lazy chunk for that request. Whether a module has **executed** is known only in the browser (`isExecuted` = module cache lookup), and the HMR boundary decision runs there (Vite `BundledDevHMRClient`).

A module can be fetched but not held by a particular client (e.g., Client A fetched it, Client B hasn't navigated to that route yet).

Per-client outcomes when a fetched lazy module is later edited (see "Editing a fetched lazy module"):

- Every connected client gets the same push with `changedIds`, `staleIds` and `gen`. Each client marks its stale factories, then walks. The walk decides the outcome: a hot update (the client pulls a patch with its own have-set, and the patch carries only what that client cannot run), a client no-op (no changed id executed, so no request), or a reload request
- `HmrUpdate::Noop` is produced when nothing is left to stamp, e.g. an unfetched lazy file that maps to no graph module, or an edit whose output is unchanged

### Build Output Refresh

A lazy compile does not refresh the build output. No page loads a lazy route from it: every lazy `import()` calls `requestLazy`. The graph holds the route after the compile's `merge`, so the next build of any kind puts the fetched proxy and the route in the output.

After a compile that returns, and after a compile that fails past its `merge`:

1. `DevEngine` notifies the coordinator via `ModuleChanged` (carrying the **raw proxy id**, `?rolldown-lazy=1` included, the watch list the compile wrote to, and `rebuild`, which is `Bundler::has_lost_hmr_update()`)
2. Coordinator first registers that watch list — it holds the files the compile loaded. The latest build's list can miss them, because a build that starts before the coordinator handles the message begins with an empty list (see [dev-engine §15](../dev-engine/implementation.md)); this step is what makes later edits to the lazy module trigger updates at all
3. Only when `rebuild` is true: the coordinator queues a `Rebuild` task with the proxy id as the changed file and marks output as stale. The task sees the lost update and becomes a full build that sends `FullReload` to every client, with the proxy id in `changedFiles` (see [dev-engine §9b](../dev-engine/implementation.md))

The raw proxy id is only a label in that task: plugins observe it once through the `watch_change` hook, and it is the `changedFiles` entry of the `FullReload`. If a full build cleared the lost update before the task runs, the task stays a partial `Rebuild` of the proxy. The id then resolves back to itself (the resolver preserves the query) and string-matches the proxy module's key in the incremental cache. `Rebuild` tasks never generate HMR updates and merge only with other `Rebuild`s, so the `?rolldown-lazy=1` pseudo-path never reaches HMR-update computation.

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

1. **Server-side selection**: when collecting the sync deps for a lazy chunk, `collect_unheld_sync_deps` (`hmr_stage.rs`) skips modules the request says the client ran (`e`) or holds a current factory of (`f`)
2. **Runtime module-cache gate**: every module in a chunk is a `__rolldown_runtime__.registerFactory(stableId, factory)` call, and `initModule` (`runtime-extra-dev-common.js`) runs the factory only when the id is not yet in the module cache. Registering a factory twice overwrites the map entry; the module body runs once

Two `/lazy` requests in quick succession, the second sent before the first chunk lands, both leave `shared.js` out of `f`, and both chunks carry it — duplicate bytes, safe: the second registration overwrites the first and the module cache gate runs the body once.

An HMR patch re-runs a module for a different reason: the client's apply step removes the module from the module cache first (`removeModuleCache`), so `initModule` will run the factory again. The factory shape is the same in both payload kinds (see [hmr/design.md](../hmr/design.md), principle 9).

### Top-Level Await

A module whose own body uses top-level await (`await`, `for await`, `await using`) gets an `async` factory: `async function (__rolldown_module_id__) { … }` (`enter_program` in `impl_traverse_for_hmr_ast_finalizer.rs`). This applies to lazy chunks and HMR patches alike. Without it, the `await` is a syntax error and the whole chunk fails to parse (#11110).

`initModule` does not await the factory. The factory registers the module before its first `await`, so importers get the exports object right away. The rest of the body runs later:

```js
// tla.js
export const value = await Promise.resolve(1);

// importer.js, in the same lazy chunk
import { value } from './tla.js';
console.log(value); // TDZ error: `value` is set only after the await
```

An error thrown in the body becomes a rejected promise that `initModule` drops, so it shows up only as an unhandled rejection, not at the importer.

We do not keep the order of top-level await in lazy chunks and HMR patches. For now, we only make sure the syntax is correct, not the semantics.

### Link-Stage-Synthesized Exports (JSON, text, base64, dataurl)

Modules whose exports are synthesized at link time are **broken inside lazy chunks** (and HMR patches): JSON/text/base64/dataurl modules are scanned as a bare expression statement with `ExportsKind::None`, and the `export default` is materialized only by the link stage's `generate_lazy_export` — which the lazy/HMR render path never runs (it renders pristine scan-time AST clones). The lazy chunk registers them with no exports holder — `registerModule(id)`, which the runtime fills in as `{ exports: {} }` — so importers see **empty exports on first lazy load**; the same import still sees empty exports after a page refresh, because the route still arrives as a lazy chunk. No playground fixture covers this yet.

### CSS

CSS bundling was removed from rolldown (#4271), and the lazy boundary is created without loading the target — so `import('./style.css')` builds fine and the hard error (`Bundling CSS is no longer supported`) is **deferred to the first `/lazy` request**: HTTP 500, catchable rejection at the consumer's `await import()`.

### Assets

Rolldown core has no built-in asset handling: an extension outside the default `module_types` map is read as UTF-8 and parsed as JS, so a binary file statically imported in a lazy subtree fails the lazy compile at request time. Asset imports inside a lazy subtree work only when a plugin converts them to JS in its `load` hook (as the dev server's ported `vite:asset` plugin does — see "Emitted assets").

### Sourcemaps

No file is kept for a lazy chunk or an HMR patch, so there is no map file to point at. `HmrStage::finish_payload_sourcemap` (`hmr_stage.rs`) handles both payload kinds:

- `'inline'`: the engine appends `//# sourceMappingURL=data:...` to the code. The consumer must not add its own comment.
- `sourcemap: true` / `'file'` and `'hidden'`: no comment; the map comes back as JSON in `HmrLazyChunkOutput::sourcemap` (`crates/rolldown_common/src/hmr/lazy_chunk_output.rs`), and the napi layer forwards it in `BindingLazyChunkResponse` (`binding_dev_engine.rs`). For `true` / `'file'`, the consumer serves the map and adds the comment that points to it. So the browser downloads the map only when devtools asks for it, and the server does not take an inline map out of the code again.

The map is prepared for the fixed name `__rolldown_lazy_chunk.js` in the out dir (`__rolldown_hmr_patch.js` for a patch), so its `sources` are relative to the out dir. Vite keeps the maps of the lazy chunks it served lately (`BundledDev#lazyMaps`, a `PayloadMapStore` in `bundledDevPayload.ts`, apart from the patch maps, at most 256 maps and 64 MiB, the least recently used go first, one entry for each distinct map) and serves each one at `<base>__rolldown_payload_map_<id>.map`, next to the output files. So the sources resolve as they do for an output chunk, also for a lazy chunk served from `/@vite/lazy`. The module runner gets the map with the payload and passes it to `ssrTransform`. Pinned by the lazy sourcemap test in `dev-lazy-compile.test.ts` (`'inline'`, `true` and `'hidden'`).

## Implementation Details

### Module ID Format

**IMPORTANT**: All runtime module lookups use **stable IDs** (`stable_id`), relative paths from the cwd (e.g., `src/module.js`). Emitters read them from each module's `stable_id` field.

This ensures consistency between:

- The `requestLazy("src/module.js", …)` call and the `initModule("src/module.js")` it runs
- Compiled module wrappers: `registerFactory("src/module.js", function (__rolldown_module_id__) { … })` (inside the wrapper body, `registerModule` / `createModuleHotContext` receive the id via the `__rolldown_module_id__` parameter)
- `import.meta.hot.accept("src/dep.js", ...)` specifiers
- `changedIds` in the HMR push and the `registerGraph` rows the client walk reads

Absolute paths survive in exactly two places: the proxy id in `requestLazy(..., "<absProxyId>")` (sent by `loadLazyChunk` as the `/@vite/lazy?id=` query value) and the fetched template's `import($MODULE_ID)` (used for resolution).

`render_proxy_template` (`lazy_compilation_plugin.rs`) fills the fetched template's one placeholder, `$MODULE_ID` (the absolute path, query stripped). It inserts the id as a serde_json-quoted JS string literal, so Windows backslash paths stay escaped (#9102). The stub template is an empty `export {};` with no placeholder.

### Fetched State Tracking

The `LazyCompilationPlugin` maintains two sets. Only `fetched_entries` is shared with the `DevEngine`, through the `LazyCompilationContext` that `plugin.context()` returns.

- `lazy_entries` - All proxy module IDs created during resolution
- `fetched_entries` - Proxy module IDs that have been fetched (requested at runtime via `/lazy`)

When `resolve_id` is called for a dynamic import:

1. If the importer is a **fetched proxy** (`?rolldown-lazy=1` + in `fetched_entries`), skip proxy creation and resolve the actual module with the call the proxy replaced (`resolve_with_original_call`, see "Creating the actual module" below)
2. Otherwise → resolve the specifier via `ctx.resolve` (`skip_self: true`, forwarding `args.custom`). If that `ResolvedId` is external (`external.is_external()`: `Bool(true)` or `Absolute`, since `resolve_id_check_external` turns `Relative` into `Bool(true)`), return `None` and do **not** append `?rolldown-lazy=1`. The normal resolution then runs again and builds the full `ResolvedId`, the same as a build without lazy compilation. Passing the `ctx.resolve` result back through `HookResolveIdOutput` would drop fields it cannot carry, such as `is_external_without_side_effects` on Node built-ins. The cost is a second resolution, only for external dynamic imports. An external never gets a factory in the dev runtime, so `requestLazy` cannot run it. That covers every path `ctx.resolve` can mark external: a plugin `resolveId` of `{ id, external: true }`, config `external` matching the resolved id (applied after plugins in `resolve_id_check_external`), Node built-ins, and http(s) URLs (#11273, pinned by `packages/rolldown/tests/dev/dev-lazy-compile.test.ts`). Config `external` matching the raw specifier never reaches this hook: `resolve_id_check_external` applies it before any plugin runs. The rendered dynamic import stays a plain `import("<external>")`.
3. Otherwise append `?rolldown-lazy=1`. The append is **idempotent** (#9439): `ctx.resolve` can re-enter other plugins' resolve hooks (e.g. an alias plugin), so if the resolved id already ends with the marker it is reused — a doubled suffix would desync the proxy id from the runtime invalidation key in the stub template (regression vitejs/vite#22454, pinned by the aliased-import spec). The plugin stores the call (specifier, importer, kind, attributes, `custom`) in `original_calls` under the proxy id. Each resolution of the proxy replaces the stored call, so the fetch runs the call of the latest build (for example after an edit gives the import another specifier for the same file). Like `lazy_entries`, nothing is ever removed.

**Creating the actual module.** Without lazy compilation, the `resolveId` call of the `import()` creates the actual module, so the module starts with that call's `meta` and `moduleSideEffects` (see [cache/implementation.md](../cache/implementation.md)). The probe above does not create it, and the proxy result carries no options. So when the fetched template imports the actual module by its resolved id, `resolve_with_original_call` runs `ctx.resolve` again with the stored call (`skip_self: true`) in the current build and returns that result, receipt included (`HookResolveIdOutput::from_resolved_id`). User hooks see the original specifier and importer, never the resolved id with the proxy as importer, so a virtual-module plugin that resolves only its bare id (`virtual:x`, not `\0virtual:x`) works too. It falls back to `None` (the next plugins resolve the resolved id with the proxy as importer, and the module starts with empty options) when no call is stored, the stored importer is an absolute path that is no longer on disk, the call fails (the error is dropped), or the call now gives an external or another id (for example after an edit of the importer). Pinned by `packages/rolldown/tests/dev/dev-lazy-compile.test.ts`.

Re-resolution of a known proxy id (any import kind — e.g. a dev server resolving the stub id as an entry to serve a lazy compilation request) is claimed by the lazy plugin itself: if the specifier ends with `?rolldown-lazy=1` and is present in `lazy_entries`, it resolves to itself. Unknown proxy ids fall through and stay unresolvable (cf. the #9969 gate below). User-land `resolve_id` hooks never see proxy ids at all — the `PluginDriver` skips them for `?rolldown-lazy=1` specifiers — because a virtual-module plugin would not recognize its own id with the query appended and nothing else can resolve a virtual id (regression vitejs/vite#23124, pinned by `packages/rolldown/tests/dev/dev-lazy-compile.test.ts`).

When `load` is called for a proxy module:

1. Only ids present in `lazy_entries` are served at all — any other `?rolldown-lazy=1` id falls through to `Ok(None)`
2. If in `fetched_entries` → return fetched template; otherwise → return stub template
3. User-land build hooks (`resolve_id`, `load`, `transform`, `transform_ast`, `module_parsed`) are skipped for proxy ids (`?rolldown-lazy=1`) so plugins only see real modules; the lazy plugin itself still runs to serve the stub/fetched template.

**Security gate — unknown module rejection (#9969)**: the id passed to `compileEntry` / `compile_lazy_entry` is treated purely as a lookup key into the build cache, never resolved as a filesystem path. An id not present from a prior build is rejected in `HmrStage::compile_lazy_entry` with `Lazy entry module not found in cache` — so a malicious `/@vite/lazy` request cannot bundle an arbitrary file (analogous to Vite's `server.fs.strict`; pinned by `packages/rolldown/tests/dev/dev-lazy-compile.test.ts`). Note the ordering: `DevEngine::compile_lazy_entry` calls `mark_as_fetched` unconditionally **before** this validation, so an unknown id still lands in `fetched_entries` (harmless, but worth knowing when debugging).

### Lazy Chunk Rendering

`DevEngine::compile_lazy_entry(proxy_module_id, query)` (`dev_engine.rs`) parses the query into a `ClientHave` (`crates/rolldown_common/src/hmr/client_have.rs`) and checks it with `Bundler::hmr_reload_reason` before it marks the proxy as fetched. Then `Bundler::compile_lazy_entry(module_id, &have)` (`impl_bundler_hmr.rs`) → `HmrStage::compile_lazy_entry(module_id, &have)`:

1. Read the catch-up from the session (`HmrSession::catch_up`): a `g` behind the session gets a `catchUp` statement, an unknown epoch or a `g` too old to catch up gets `Reload`. Look the proxy up in the module cache (the #9969 gate), then run `ScanMode::Partial([proxy's resolved id])`, `merge` and `apply_module_side_effects_overrides`. The loader runs with `set_retry_pending_rescans(false)`, so it takes from `pending_rescans` only the proxy. A repeat visit skips all three: the proxy already has a resolved `import()` record (only the fetched template has one), and the proxy does not wait in `pending_rescans` (see [design.md](./design.md#6-repeat-visits)). The out-parameter `merged` becomes `true` just before `merge`, so `DevEngine` can tell a compile that failed after its `merge` from one that failed before it. After the merge it still delivers the emitted assets and sends `ModuleChanged` (see [Notifying the Coordinator](#notifying-the-coordinator)).
2. `collect_unheld_sync_deps` walks the proxy's static deps plus the proxy's own dynamic import. A module the client ran (`e`) **stops** the walk. A module whose current factory the client holds (`holds_current`: in `f`, stamp at or below `g`) is not carried, but the walk goes through it: a patch may have carried it to a client that never ran it, so its deps may be missing. Every other module is carried; external modules are dropped and the rest sorted by id. The proxy is only the walk root and is never carried: it never runs in the client, because `requestLazy` runs the real module. The rules are the ones of a patch (see [hmr/design.md](../hmr/design.md), principle 7)
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

   A module prints the same factory code in every payload, wherever it sits in the payload. So the binding names do not use the payload position (`hmr_import_binding_name` in `hmr_ast_finalizer.rs`):
   - a normal importee is a `var` inside the factory: `import_<importee repr>_<record id>`
   - an external importee is an `import * as` at the payload top level, shared by every module in the payload: `import$<escaped importer stable id>_<record id>_<importee repr>`. The escape keeps ASCII letters and digits and writes every other byte as `$` plus two hex digits, so two importers never get the same name

4. Dynamic imports inside the rendered modules are rewritten:
   - importee id contains `?rolldown-lazy=1` (a nested lazy proxy) → `__rolldown_runtime__.requestLazy("<stableRealId>", "<absProxyId>"[, 1])` — the same shape the full build emits, built by `create_request_lazy_call` in `crates/rolldown/src/hmr/utils.rs` (pinned by the nested-dynamic-import spec). The emitted code carries ids only; `DevRuntime#loadLazyChunk` builds the URL (see [design.md](./design.md#4-dev-server-integration)). The trailing `1` is the importer's node-mode interop flag; whether the importee is CommonJS is decided in the runtime from the module's own registration (see design.md "CommonJS interop happens in the runtime")
   - ordinary `import()` → `__rolldown_runtime__.loadDynamic("<stableId>")` (`try_rewrite_dynamic_import` in `hmr_ast_finalizer.rs`); it runs the module when the client can and pulls it first otherwise (see [hmr/design.md](../hmr/design.md), principle 5). A CommonJS importee is known here, so the call becomes `loadDynamic("<stableId>").then(() => __rolldown_runtime__.__toESM(__rolldown_runtime__.loadExports("<stableId>")[, 1]))`
5. The chunk starts with the prelude `const __rolldown_payload__ = __rolldown_runtime__.beginPayload(G)`, then the `catchUp` statement when one is needed, then one `__rolldown_payload__.registerGraph(delta)` statement (`render_register_graph_source`, `module_graph_delta.rs`) with `nums`, and then carries `__rolldown_payload__.registerFactory` calls only — no execute-entry tail. Numbers are assigned to the carried modules and the targets of their edges before the rows render. `requestLazy` runs the module once the chunk has evaluated, so a throw from the module body reaches the importer's `await import(...)` instead of becoming a floating rejection inside the proxy's async wrapper
6. The result is returned as `HmrPullResponse::Payload(HmrLazyChunkOutput { code, sourcemap })` (see "Sourcemaps"). One `tracing::debug!` line (target `hmr`, message `compile_lazy_entry: done`) logs `scanned`, `modules` (the payload size), and the time of the scan and of the render. The napi layer turns it into `{ type: 'Chunk', code, sourcemap? }`, and a reload into `{ type: 'Reload', reason }` (`BindingLazyChunkResponse`)

### Emitted Assets (#9815)

Lazy compiles (like HMR patches) never run the generate stage, so assets emitted during the compile have no `onOutput` path. Instead, on success and after a failure past the `merge`, `DevEngine::compile_lazy_entry` drains `file_emitter.add_additional_files` into a `BundleOutput` and fires the `onAdditionalAssets` dev callback **before** returning the code — so the consumer can register/serve the assets (test-dev-server puts them in `memoryFiles`) before the browser requests them (fixes vitejs/vite#22596, pinned by the emitted-asset spec).

Design constraint for consumers: asset URLs must be resolved **eagerly at `load`** (`emitFile` + `getFileName`, as the dev server's Vite-style asset plugin does) — a `renderChunk`-time placeholder scheme would leak, because the lazy render path never runs `renderChunk`.

### Notifying the Coordinator

After a compile that returns, or that failed after its `merge`, the dev engine does two things, in order:

```rust
// In DevEngine::compile_lazy_entry
if result.is_ok() || merged {
  // 1. deliver assets emitted during the compile (before the code returns)
  if let Some(on_additional_assets) = ... { ... }
  // 2. watch the compile's files; queue a build only for a lost HMR update
  let rebuild = bundler.has_lost_hmr_update();
  self.notify_module_changed(proxy_module_id, Arc::clone(bundler.watch_files()), rebuild);
}
```

The coordinator handles `ModuleChanged`:

1. Register the watch list the message carries (see "Data Lifecycle → Build Output Refresh" for why)
2. Stop here when `rebuild` is false
3. Queue a `TaskInput::Rebuild` with the raw proxy id as the changed file
4. Set `has_stale_bundle_output = true`
5. Schedule build if stale (runs immediately only when the coordinator is Idle/Failed; otherwise waits in the queue)

A compile that fails before its `merge` sends nothing: the graph did not change (the proxy stays marked fetched). A failed lazy scan queues its proxy in `pending_rescans` (if the graph still imports it), next to the files that were already queued.

### Error Handling

The error contract (no longer "POC — Err or panic is fine"):

- **Failures cross the napi boundary as diagnostics**, not as a message: `compile_entry` returns `BindingResult<BindingLazyChunkResponse>` and `DevEngine.compileEntry` unwraps it, the same contract the other single-result methods use (see `internal-docs/dev-engine/implementation.md` §16b). This is what keeps a throwing plugin identifiable — a lazy compile is the first time a lazy module runs the plugin pipeline, so the caller gets the plugin name, the hook, the module id and the original stack rather than a Debug-formatted string. Pinned by a `dev-lazy-compile.test.ts` case
- **Unknown module id** → `Err("Lazy entry module not found in cache. module_id=...")` in `HmrStage::compile_lazy_entry`; the dev-server middleware answers HTTP 500 (success sets `Content-Type: text/javascript`)
- **A client that must reload** → `{ type: 'Reload', reason }`, not an error: a query that does not parse, another engine's epoch, or a `g` the session cannot catch up. The check runs before `mark_as_fetched`, so the server state does not change. Vite serves it with status 200 and a body that calls `__rolldown_runtime__.reloadNeeded(reason)`, so the page reloads even when the `import()` that started the load has a `catch` (see [design.md](./design.md#4-dev-server-integration))
- **A chunk that is still not runnable** → `requestLazy` loads the chunk at most twice (a second request describes the client as it is then). If the module still cannot run, it calls `hooks.onMissingFactory` and rejects with `MissingFactoryError`. A failed load (this case, an HTTP 500 or a network error) is not memoized, so the next `import()` asks again
- **A client that missed an update** → `requestLazy` loads the chunk even when it could run the module from the factories it holds, so it gets the catch-up before it runs a factory that may be old (`missedUpdate()`)
- **Init errors are catchable (#9981)**: the lazy chunk only registers factories; `requestLazy` runs `initModule` itself once the chunk has evaluated, inside the promise it hands back to the consumer. An error thrown while the lazy module initializes therefore rejects that promise, hence the consumer's `await import(...)` — try/catch works, and without a handler exactly one `unhandledrejection` fires. This rejection is memoized like a native `import()` of a throwing module; a retry could not work, since a factory registers its module before running its body. Pinned on both the **cold** path (first `/lazy` compile) and the **warm** path (a reload, then a repeat visit served from the graph) by the lazy-init-error specs (#9975 added the original failing spec; #9981 rewrote and split it)
- **Runtime `loadExports` miss** does not throw — it warns and returns `{}`
- The one remaining panic: calling `compile_lazy_entry` before any bundle has been built. After a failed full build (no module graph) the compile returns an error instead

### The Have-Set Query

- Built by `DevRuntime#encodeHave` (`runtime-extra-dev-common.js`) when `loadLazyChunk` runs: `id=<proxy id>&s=…&g=…&e=…&f=…&n=…`
- `loadLazyChunk` gets the query from `encodeHave` and the URL from `lazyUrl(proxyModuleId, query)`. Each set is a `b` bitset (see [hmr/design.md](../hmr/design.md), principle 6)
- Its only role is **chunk sizing**: the returned chunk omits what the client ran or holds current. Nothing is "routed" — the compiled code returns in the HTTP response
- A module with no number on the client is left out of `e` and `f`, so the server carries it. That can only add bytes
- `ClientHave::parse_query` ignores keys it does not know (`id`, `n`). A missing set is empty. A set that names a number at or above the session's number count, or is not in the wire format, makes the query fail to parse, which is a Reload

### Editing a Fetched Lazy Module

After `/lazy`, the real module and its sync deps are ordinary watched graph modules (thanks to the coordinator registering the compile's watch list), and an edit flows through the standard watch → HMR path:

- The server has no importer walk. It stamps the changed modules and pushes their ids. A proxy is never in `changedIds` or `staleIds`: it never runs in the client, because `requestLazy` runs the real module. `rerendered_importers` (importers whose factory reads a changed interop fact) leaves proxies out too, and the modules that call `import()` of a proxy emit `requestLazy`, which reads only ids. So a patch never carries a proxy, and neither does a lazy chunk (see "Lazy Chunk Rendering")
- The boundary decision runs in the browser (Vite `BundledDevHMRClient`). `render_register_graph_source` (`module_graph_delta.rs`) gives a proxy no row and points an `import()` edge into a proxy at the real module, whether or not the proxy was fetched. So the client walk climbs from the lazy module to the module that called `import()`, as for any `import()`:
  - A boundary on the way (the lazy module or an importer self-accepts, or an importer calls `import.meta.hot.accept('./lazy.js')`) applies a hot update. `ensureRunnable` pulls the patch it needs first
  - With no boundary, the walk reaches the entry and the client sends `vite:bundled-dev:reload-needed`. The server answers that client with a full reload once the rebuild output lands (see [hmr/design.md](../hmr/design.md), "Failure policy")
  - Pinned by the `hmr-lazy-dynamic-import-self-accept` and `hmr-lazy-dynamic-import-accept-dep` playgrounds, the shared-module spec's watch/auto-reload test, and `the client graph links a lazy import to the real module, not to its proxy` in `packages/rolldown/tests/dev/dev-lazy-compile.test.ts`
- A client where no changed id executed computes a client no-op and makes no request (see "Fetched vs Held")

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
│  - The first full-build rows set the runtime's epoch and the module     │
│    numbers. The server tells the page the generation of the output      │
│    (`setOutputGeneration`)                                              │
│  - The proxy's chunk is never fetched; nothing registers under the      │
│    proxy id (a factory-less cache entry would confuse the HMR walk)     │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 3. USER CODE HITS: import('./lazy-module')                              │
├─────────────────────────────────────────────────────────────────────────┤
│  - requestLazy("src/lazy-module.js", proxyId) memoizes one promise      │
│    per real module id (dedup across importers)                          │
│  - Module and its static imports all ran or have factories here         │
│    (isRunnableHere) → initModule, no request. A factory alone is not    │
│    enough: a patch can ship a module without its deps (#11028)          │
│  - Otherwise loadLazyChunk(proxyId):                                    │
│      /@vite/lazy?id=<encoded proxy id>&s=…&g=…&e=…&f=…&n=…              │
│  - Browser waits on the memoized promise                                │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 4. DEV SERVER RECEIVES /lazy REQUEST                                    │
├─────────────────────────────────────────────────────────────────────────┤
│  - Calls DevEngine.compileEntry(proxyModuleId, query)                   │
│  - Engine parses the have-set; wrong epoch or too old → Reload          │
│  - Marks proxy as FETCHED in LazyCompilationContext                     │
│  - Rejects ids not in the module cache (security gate, #9969)           │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 5. PARTIAL SCAN + RENDER                                                │
├─────────────────────────────────────────────────────────────────────────┤
│  - ScanMode::Partial([proxyModuleId]), skipped on a repeat visit        │
│  - Plugin's load hook sees proxy is fetched → returns FETCHED template  │
│  - Fetched template: import("/abs/path/lazy-module.js")                 │
│  - resolve_id sees importer is a fetched proxy → returns None           │
│  - Actual module + sync deps carried — minus modules the client ran     │
│    or holds current (e / f in the query); never the proxy               │
│  - catchUp (if g is behind), registerGraph rows, then                   │
│    registerFactory(stableId, fn) per module; no execute-entry tail      │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 6. RETURN COMPILED JS TO BROWSER                                        │
├─────────────────────────────────────────────────────────────────────────┤
│  - Assets emitted during the compile already delivered via              │
│    onAdditionalAssets (#9815)                                           │
│  - Response is a single JS string; Vite links its sourcemap (inline     │
│    only for sourcemap: 'inline')                                        │
│  - Browser loads it as an ES module; initializers register each module  │
│  - requestLazy then runs initModule("src/lazy-module.js") itself        │
│  - Original import() promise resolves with the module's exports         │
│    (or rejects catchably, #9981)                                        │
│  - Still not runnable → requestLazy loads once more, then reloads       │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 7. NOTIFY COORDINATOR                                                   │
├─────────────────────────────────────────────────────────────────────────┤
│  - DevEngine sends ModuleChanged { proxyModuleId,                       │
│    rebuild = has_lost_hmr_update() }                                    │
│  - Coordinator watches the compile's files; it queues a Rebuild only    │
│    for a lost HMR update                                                │
│  - The build output keeps the stub until the next build; a reload       │
│    still loads the route via /lazy                                      │
└─────────────────────────────────────────────────────────────────────────┘
```

## Lessons Learned

### Issue 1: Module ID Consistency is Critical

**Problem**: The proxy module, compiled module, and HMR runtime must use the same ID format for module lookups to work.

**Solution**: Use **stable IDs** (`stable_id`, relative paths from cwd) consistently in the runtime:

- `__rolldown_payload__.registerFactory(stableId, factory)` — inside the wrapper, `registerModule(__rolldown_module_id__, { exports })` (`registerModule(__rolldown_module_id__, __rolldown_module__, 1)` for CommonJS) and `createModuleHotContext(__rolldown_module_id__)` receive the id via the wrapper parameter (the main-bundle path still emits stable-id string literals)
- `loadExports(stableId)` / `initModule(stableId)`
- `import.meta.hot.accept(stableId, callback)`
- `registerGraph` rows and `changedIds` in the HMR push

Emitters read the stable id from each module's `stable_id` field. The proxy templates contain no stable id.

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

**Solution**: In `resolve_id`, skip proxy creation when the importer is a fetched proxy, and resolve the actual module instead (`resolve_with_original_call`, see "Creating the actual module" above). This allows the fetched template's dynamic import to resolve to the actual module.

### Issue 4: The Build Output Does Not Follow Each Lazy Compile

**Problem**: A whole-output rebuild after each lazy compile made N compiles cost N + 1 builds. Each build also split the shared chunks again, so most chunk hashes changed, and the dev server kept a copy of each build (#11489).

**Solution**: No page loads a lazy route from the build output, so a compile queues no build. The graph holds the route, and the next build of any kind puts it in the output. `ModuleChanged` still carries the watch list, and asks for a build only while an HMR update is lost:

```rust
// In DevEngine::compile_lazy_entry
let rebuild = bundler.has_lost_hmr_update();
self.notify_module_changed(proxy_module_id, watch_files, rebuild);
```

See "Build Output Refresh".

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
7. After the next build, both proxy (fetched) and actual module are in the output

### Issue 8: Proxy-ID Creation Must Be Idempotent (#9439)

**Problem**: With an alias plugin present, `ctx.resolve` re-entered the lazy plugin's `resolve_id`, appending `?rolldown-lazy=1` twice. The doubled suffix desynced the proxy id from the id the real module registers under, so the importer never saw the real exports (`mod.foo` came back undefined — regression vitejs/vite#22454).

**Solution**: Before appending the marker, check whether the resolved id already ends with `?rolldown-lazy=1` and reuse it. Pinned by the aliased-import spec.

### Issue 9: Fetched Template Must Read Exports From the Registry (#9132)

**Problem**: The fetched template originally returned the dynamic import's namespace object. When a shared lazy module landed in a common chunk, chunk-level renaming minified the export names and the namespace lookup yielded `undefined`.

**Solution**: read the exports from the runtime registry by stable id, which preserves original export names. Today `requestLazy` resolves with `initModule("<stableRealId>")` directly; the fetched template's body is never executed and its `import($MODULE_ID)` only roots the partial scan. Pinned by the shared-module spec.

### Issue 10: Init Errors Must Reject the Consumer's Promise (#9981)

**Problem**: An error thrown while a lazily-compiled module initialized escaped as an unhandled rejection instead of surfacing at the consumer's `await import(...)`.

**Solution**: the lazy chunk carries no execute-entry tail. `requestLazy` runs `initModule` itself after `loadLazyChunk` resolves, inside the promise it returns, so a throw from the module body rejects the consumer's import promise instead of floating out of an async wrapper. Pinned by the two lazy-init-error specs (a first visit, then a repeat visit after a reload).

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

| Spec                        | Pins                                                                                                                                                                                                                          |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `basic`                     | lazy module arrives in exactly one JS request (`/@vite/lazy`), no stub chunk                                                                                                                                                  |
| `aliased-import`            | idempotent proxy-id creation under alias re-entrancy (vite#22454)                                                                                                                                                             |
| `emitted-asset`             | assets emitted during lazy compile are servable on first load (vite#22596)                                                                                                                                                    |
| `late-lazy-chunk`           | a lazy chunk that lands after a newer HMR patch does not replace its code                                                                                                                                                     |
| `lazy-init-error`           | init errors catchable with try/catch — cold and warm paths (#9975/#9981)                                                                                                                                                      |
| `lazy-init-error-unhandled` | exactly one `unhandledrejection` without a handler — cold and warm paths                                                                                                                                                      |
| `nested-dynamic-import`     | nested lazy `import()` inside a lazy chunk resolves on first click                                                                                                                                                            |
| `shared-module`             | export-name preservation in shared chunks (#9132) + watch/auto-reload after fetch + a module shared by a visited and a new route runs once after a build and a reload, and the visited route's second compile starts no build |
| `top-level-await`           | a lazy module with top-level await parses and runs (#11110); its importer gets a TDZ error                                                                                                                                    |

Separate playground: `inline-common-chunks-lazy` runs lazy compilation with `experimentalInlineCommonChunks`. Two lazy routes share modules: they run once, also after a reload, the page loads no built route chunk, and an edit of a shared module hot-updates both routes.

Several specs use `retry: 0` because the bugs only reproduce on the first interaction with a fresh server. Unit test: `packages/rolldown/tests/dev/dev-lazy-compile.test.ts` pins the unknown-id rejection (#9969), the `requestLazy` rewrite, that both emitters carry ids only and load the chunk through `DevRuntime#loadLazyChunk`, that external import names do not clash when factories from different payloads meet in one payload, and that a lazy chunk carries no factory for its proxy. Its tests build real have-sets: a `DevRuntime` runs the `registerGraph` rows of the built entry, and `encodeHave` writes the query. `crates/rolldown/src/hmr/hmr_ast_finalizer.rs` (`cargo test -p rolldown hmr_import_binding_name`) pins the escape in external import names, and that the names differ for each importer id and record number. `packages/rolldown/tests/dev/dev-watch.test.ts` pins that an HMR patch carries no factory for a proxy. `packages/rolldown/tests/dev/dev-runtime.test.ts` pins that `requestLazy` loads the chunk again once and then asks for a reload, and that a Reload answer stops all requests and rejects `requestLazy`. `crates/rolldown_dev/src/dev_engine.rs` (`cargo test -p rolldown_dev --features testing lazy_`) pins the scan and build rules: a first visit queues no build and watches the route's files; a repeat visit does not scan and serves the same code and sourcemap; a retry after a failed scan scans again with no reload, and takes its proxy out of `pending_rescans`, so the next visit does not scan; a compile that failed past its `merge` still delivers its asset and watches its files, and the next visit is a repeat visit; a broken file of another route, or a broken unrelated file, does not fail a visit; a failed visit keeps the other queued files for the next HMR task; a visit with a lost HMR update reloads every client through one full build; a deleted route file still reports its error. The tests see a scan through the `resolveId` calls of a counting plugin: user plugins never see the proxy, but a scan resolves the call that the proxy replaced. `compile_lazy_entry_reloads_or_catches_up_the_client` pins that a client that must reload changes nothing, and that a client behind the session gets the catch-up. `packages/rolldown/tests/dev/dev-lazy-compile.test.ts` also pins that N compiles leave one build (#11489), and that a build after a compile still emits `requestLazy` for the fetched route.

## Files Changed (Reference)

For future debugging, these files handle lazy compilation:

### Core Plugin

1. **`crates/rolldown_plugin_lazy_compilation/src/lazy_compilation_plugin.rs`** - Plugin with `resolve_id` and `load` hooks; `LazyCompilationContext` with fetched-state tracking; `render_proxy_template`
2. **`crates/rolldown_plugin_lazy_compilation/src/proxy-module-template.js`** - Stub template (not fetched)
3. **`crates/rolldown_plugin_lazy_compilation/src/proxy-module-template-fetched.js`** - Fetched template
4. **`crates/rolldown/src/hmr/utils.rs`** - `create_request_lazy_call`, the one emitted shape for a lazy boundary
5. **`crates/rolldown_plugin_hmr/src/runtime/runtime-extra-dev-common.js`** - `requestLazy`, the runtime entry point
6. **`crates/rolldown/src/utils/apply_inner_plugins.rs`** - registers the plugin when `experimental.dev_mode.lazy == true`
7. **`crates/rolldown_common/src/hmr/lazy_proxy_id.rs`** - `LAZY_PROXY_QUERY` and the proxy-id helpers (`is_lazy_proxy`, `lazy_proxy_target`) that every crate uses to recognize a proxy

### Dev Engine

6. **`crates/rolldown_dev/src/dev_engine.rs`** - `compile_lazy_entry()` (query parse, epoch / generation check, mark-as-fetched, asset delivery, `notify_module_changed()`)
7. **`crates/rolldown_dev/src/types/coordinator_msg.rs`** - `ModuleChanged` message variant
8. **`crates/rolldown_dev/src/bundle_coordinator.rs`** - Handles `ModuleChanged` (watch the compile's files; rebuild only for a lost HMR update), state machine
9. **`crates/rolldown_binding/src/binding_dev_engine.rs`** - napi surface (`compile_entry` → `BindingLazyChunkResponse`)
10. **`crates/rolldown_common/src/hmr/client_have.rs`** + **`hmr_session.rs`** - the request format (`ClientHave`, `HmrNumSet` with its bitset form and decoding limits) and the session (numbers, stamps, catch-up)

### HMR/Build

11. **`crates/rolldown/src/hmr/hmr_stage.rs`** - `compile_lazy_entry()`: catch-up, cache gate, partial scan, dep collection from the have-set, chunk rendering
12. **`crates/rolldown/src/hmr/hmr_ast_finalizer.rs`** + **`impl_traverse_for_hmr_ast_finalizer.rs`** - `registerFactory` wrappers, dynamic-import rewrites (incl. the `/@vite/lazy` rewrite), computed-property exports
13. **`crates/rolldown/src/hmr/utils.rs`** - register-module / hot-context statement builders (`__rolldown_module_id__` param)
14. **`crates/rolldown/src/bundler/impl_bundler_hmr.rs`** - `Bundler::compile_lazy_entry` entry point
15. **`crates/rolldown_plugin_hmr/src/runtime/runtime-extra-dev-common.js`** - browser runtime: `registerFactory`, `initModule` (module-cache gate), `registerModule`, `loadExports`, `requestLazy`, `loadLazyChunk`, `encodeHave`, `lazyUrl`

### Reference Dev Server (Vite full bundle mode, vendored at `vite/` (repo root))

16. **`packages/vite/src/node/server/middlewares/triggerLazyBundling.ts`** - the `/@vite/lazy` middleware (500 on error, `text/javascript` on success and for a Reload body, through `sendPayload` in `bundledDevPayload.ts`, which ends every payload with `; export {}` so it cannot load as a classic script)
17. **`packages/vite/src/node/server/bundledDev.ts`** - `triggerLazyBundling` (fills in `g` when the query has none, then `devEngine.compileEntry(id, query)`; a `Reload` becomes the `reloadNeeded` body), `onAdditionalAssets` storage, rebuild/reload handling
18. **`packages/vite/src/node/plugins/asset.ts`** - the bundled-dev branch resolves asset imports eagerly at `load`

## References

- [design.md](./design.md) — goals, scope, and key design decisions
- Current implementation: `crates/rolldown_plugin_lazy_compilation/`
- Dev engine: `crates/rolldown_dev/` (see also `internal-docs/dev-engine/`)
- Example: `examples/lazy-compilation/`
