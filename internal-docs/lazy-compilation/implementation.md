# Lazy Compilation — Implementation

> Goals, scope, and key design decisions live in [design.md](./design.md).

## Data Lifecycle

### Overview

Lazy compilation involves data in two places:

1. **Session Scope** (server) - Shared by all browser tabs, lives for the entire dev server lifetime
2. **Client State** (browser) - Per browser tab, kept by the runtime and sent in each pull

### Session Scope

Data shared across all connected browser tabs:

| Data              | Description                                                                                              |
| ----------------- | -------------------------------------------------------------------------------------------------------- |
| Module Graph      | All resolved and compiled modules                                                                        |
| `lazy_entries`    | Set of proxy module IDs discovered during resolution                                                     |
| `fetched_entries` | Set of proxy modules whose route a lazy pull scanned                                                     |
| HMR session       | `HmrSession`: epoch, generation, a stamp and a number per module, the last 64 updates (`hmr_session.rs`) |
| Build Output      | Bundled JS files on disk/memory                                                                          |
| Watched Files     | Files monitored for changes                                                                              |

**Key behavior**: Once a lazy module is fetched by any client, the graph keeps the proxy in fetched form and holds the route's modules. Pages still load every lazy route through a lazy pull (the full build emits `requestLazy` for every proxy), but a later pull of a fetched route skips the scan (see [design.md](./design.md#6-repeat-visits)). No lazy pull queues a build: the build output keeps the stub until the next build of any kind takes the route from the graph.

### Client State

The server keeps nothing per tab. The runtime (`DevRuntime` in `runtime-extra-dev-common.js`) keeps what the request needs, and `encodeHave` writes it into every pull: in the query, or, when the query is longer than `maxQueryLength`, in a POST body first (`requestQuery`, see "The Have-Set Query" below):

| Query key | Runtime data             | Meaning                                                                                                                                         |
| --------- | ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `s`       | `epoch`                  | the engine whose output the page runs, from the full-build rows                                                                                 |
| `g`       | `knownGen`               | the generation up to which the client heard of every update; every held factory is current as of it. Left out while the client does not know it |
| `u`       | `droppedDeps`            | dropped factories that a module which ran still imports; every pull asks for them again                                                         |
| `l`       | the `requestLazy` target | the module the lazy `import()` is for                                                                                                           |
| `e`       | `moduleCache` keys       | the modules the client executed                                                                                                                 |
| `f`       | `factories` keys         | the modules whose factory the client holds and that it did not execute                                                                          |
| `n`       | `requestNonce`           | a nonce, so each URL is new                                                                                                                     |

Sets are module numbers (`nums` from the `registerGraph` rows), each sent as the shorter of a bitset (`b…`) and a gap list (`d…`); an empty set is left out. The format and the reasons are in [hmr/design.md](../hmr/design.md), principles 2 and 6. A lazy target has a number before its first visit: the rows number the targets of dynamic edges, and a lazy proxy target under the target's own id (`module_graph_delta.rs`).

### Fetched vs Held

These are distinct concepts at different scopes:

- **Fetched** (session-level): A lazy pull scanned the route of this proxy. The server has compiled the actual module and its dependencies, and the graph holds them. A later pull of this route, for any client, skips the scan (see [design.md](./design.md#6-repeat-visits)).

- **Held** (client-level): The client ran the module or holds its current factory, as its request says (`e`, and `f` with a stamp at or below `g`). Used to size the payload for that request. Whether a module has **executed** is known only in the browser (`isExecuted` = module cache lookup), and the HMR boundary decision runs there (Vite `BundledDevHMRClient`).

A module can be fetched but not held by a particular client (e.g., Client A fetched it, Client B hasn't navigated to that route yet).

Per-client outcomes when a fetched lazy module is later edited (see "Editing a fetched lazy module"):

- Every connected client gets the same push with `changedIds`, `staleIds` and `gen`. Each client marks its stale factories, then walks. The walk decides the outcome: a hot update (the client pulls a patch with its own have-set, and the patch carries only what that client cannot run), a client no-op (no changed id executed, so no request), or a reload request
- `HmrUpdate::Noop` is produced when nothing is left to stamp, e.g. an unfetched lazy file that maps to no graph module, or an edit whose output is unchanged

### Build Output Refresh

A lazy pull does not refresh the build output. No page loads a lazy route from it: every lazy `import()` calls `requestLazy`. The graph holds the route after the scan's `merge`, so the next build of any kind puts the fetched proxy and the route in the output.

After a lazy pull whose scan merged (also when a later step failed), or that finds a lost HMR update:

1. `DevEngine::render_patch` sends `CoordinatorMsg::LazyCompiled { watch_files, recover_lost_update }`. `watch_files` is the watch list the scan wrote to, and `recover_lost_update` is `Bundler::has_lost_hmr_update()`
2. The coordinator first registers that watch list — it holds the files the scan loaded. The latest build's list can miss them, because a build that starts before the coordinator handles the message begins with an empty list (see [dev-engine §15](../dev-engine/implementation.md)); this step is what makes later edits to the lazy module trigger updates at all
3. Only when `recover_lost_update` is true: the coordinator queues a `FullBuild` (unless one is queued already) and marks output as stale. The task sees the lost update and sends `FullReload` to every client (see [dev-engine §9b](../dev-engine/implementation.md))

A pull that did not merge and sees no lost update sends nothing. A pull with no `l` never sends it.

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

1. **Server-side selection**: when collecting the sync deps for a lazy payload, `collect_unheld_sync_deps` (`hmr_stage.rs`) skips modules the request says the client ran (`e`) or holds a current factory of (`f`)
2. **Runtime module-cache gate**: every module in a payload is a `__rolldown_payload__.registerFactory(stableId, factory)` call, and `initModule` (`runtime-extra-dev-common.js`) runs the factory only when the id is not yet in the module cache. A second registration replaces the map entry unless its payload generation is older than the stored one (`factoryGens`); the module body runs once

Two lazy pulls in quick succession, the second sent before the first payload lands, both leave `shared.js` out of `f`, and both payloads carry it — duplicate bytes, safe: registration keeps the copy with the newer or equal generation, and the module cache gate runs the body once.

An HMR update re-runs a module for a different reason: the client's apply step removes the module from the module cache first (`removeModuleCache`), so `initModule` will run the factory again. The factory shape is the same for every pull (see [hmr/design.md](../hmr/design.md), principle 9).

### Top-Level Await

A module whose own body uses top-level await (`await`, `for await`, `await using`) gets an `async` factory: `async function (__rolldown_module_id__) { … }` (`enter_program` in `impl_traverse_for_hmr_ast_finalizer.rs`). This applies to every pulled payload. Without it, the `await` is a syntax error and the whole payload fails to parse (#11110).

`initModule` does not await the factory. The factory registers the module before its first `await`, so importers get the exports object right away. The rest of the body runs later:

```js
// tla.js
export const value = await Promise.resolve(1);

// importer.js, in the same lazy payload
import { value } from './tla.js';
console.log(value); // TDZ error: `value` is set only after the await
```

An error thrown in the body becomes a rejected promise that `initModule` drops, so it shows up only as an unhandled rejection, not at the importer.

We do not keep the order of top-level await in pulled payloads. For now, we only make sure the syntax is correct, not the semantics.

### Link-Stage-Synthesized Exports (JSON, text, base64, dataurl)

Modules whose exports are synthesized at link time are **broken inside pulled payloads**: JSON/text/base64/dataurl modules are scanned as a bare expression statement with `ExportsKind::None`, and the `export default` is materialized only by the link stage's `generate_lazy_export` — which the lazy/HMR render path never runs (it renders pristine scan-time AST clones). The lazy payload registers them with no exports holder — `registerModule(id)`, which the runtime fills in as `{ exports: {} }` — so importers see **empty exports on first lazy load**; the same import still sees empty exports after a page refresh, because the route still arrives as a lazy payload. No playground fixture covers this yet.

### CSS

CSS bundling was removed from rolldown (#4271), and the lazy boundary is created without loading the target — so `import('./style.css')` builds fine and the hard error (`Bundling CSS is no longer supported`) is **deferred to the first lazy pull**: HTTP 500, catchable rejection at the consumer's `await import()`.

### Assets

Rolldown core has no built-in asset handling: an extension outside the default `module_types` map is read as UTF-8 and parsed as JS, so a binary file statically imported in a lazy subtree fails the lazy compile at request time. Asset imports inside a lazy subtree work only when a plugin converts them to JS in its `load` hook (as the dev server's ported `vite:asset` plugin does — see "Emitted assets").

### Sourcemaps

No file is kept for a pulled payload, so there is no map file to point at. `HmrStage::finish_payload_sourcemap` (`hmr_stage.rs`) handles every pull:

- `'inline'`: the engine appends `//# sourceMappingURL=data:...` to the code. The consumer must not add its own comment.
- `sourcemap: true` / `'file'` and `'hidden'`: no comment; the map comes back as JSON in `HmrPatch::sourcemap` (`crates/rolldown_common/src/hmr/hmr_patch.rs`), and the napi layer forwards it in `BindingPatchResponse` (`binding_dev_engine.rs`). For `true` / `'file'`, the consumer serves the map and adds the comment that points to it. So the browser downloads the map only when devtools asks for it, and the server does not take an inline map out of the code again.

The map is prepared for the fixed name `__rolldown_hmr_patch.js` in the out dir, so its `sources` are relative to the out dir. Vite keeps the maps it served lately in two `PayloadMapStore`s (`bundledDevPayload.ts`): `BundledDev#lazyMaps` for a pull whose query has `l`, and `patchMaps` for the others, so many HMR updates cannot push out the map of a route that stays in use. Each store holds at most 256 maps and 64 MiB, the least recently used go first, one entry for each distinct map. Vite serves each map at `<base>__rolldown_payload_map_<id>.map`, next to the output files, so the sources resolve as they do for an output chunk. Vite reads `l` from the query after it resolved a token, because a token URL has no `l`. The module runner gets the map with the payload and passes it to `ssrTransform`. It keeps a payload's map while a module that the payload installed is still installed (`ModuleRunner#releasePayloadMaps`; the payload scope lists the installed ids), so stack traces of lazy route code stay mapped. Pinned by the lazy sourcemap test in `dev-lazy-compile.test.ts` (`'inline'`, `true` and `'hidden'`) and by `server-source-maps.spec.ts` in Vite.

## Implementation Details

### Module ID Format

**IMPORTANT**: All runtime module lookups use **stable IDs** (`stable_id`), relative paths from the cwd (e.g., `src/module.js`). Emitters read them from each module's `stable_id` field.

This ensures consistency between:

- The `requestLazy("src/module.js"[, 1])` call and the `initModule("src/module.js")` it runs
- Compiled module wrappers: `registerFactory("src/module.js", function (__rolldown_module_id__) { … })` (inside the wrapper body, `registerModule` / `createModuleHotContext` receive the id via the `__rolldown_module_id__` parameter)
- `import.meta.hot.accept("src/dep.js", ...)` specifiers
- `changedIds` in the HMR push and the `registerGraph` rows the client walk reads

Absolute paths survive in exactly one place: the fetched template's `import($MODULE_ID)` (used for resolution). Emitted code carries stable ids, and request URLs carry module numbers.

`render_proxy_template` (`lazy_compilation_plugin.rs`) fills the fetched template's one placeholder, `$MODULE_ID` (the absolute path, query stripped). It inserts the id as a serde_json-quoted JS string literal, so Windows backslash paths stay escaped (#9102). The stub template is an empty `export {};` with no placeholder.

### Fetched State Tracking

The `LazyCompilationPlugin` maintains two sets. Only `fetched_entries` is shared with the `DevEngine`, through the `LazyCompilationContext` that `plugin.context()` returns.

- `lazy_entries` - All proxy module IDs created during resolution
- `fetched_entries` - Proxy module IDs whose route a lazy pull scanned

When `resolve_id` is called for a dynamic import:

1. If the importer is a **fetched proxy** (`?rolldown-lazy=1` + in `fetched_entries`), skip proxy creation and resolve the actual module with the call the proxy replaced (`resolve_with_original_call`, see "Creating the actual module" below)
2. Otherwise → resolve the specifier via `ctx.resolve` (`skip_self: true`, forwarding `args.custom`). If that `ResolvedId` is external (`external.is_external()`: `Bool(true)` or `Absolute`, since `resolve_id_check_external` turns `Relative` into `Bool(true)`), return `None` and do **not** append `?rolldown-lazy=1`. The normal resolution then runs again and builds the full `ResolvedId`, the same as a build without lazy compilation. Passing the `ctx.resolve` result back through `HookResolveIdOutput` would drop fields it cannot carry, such as `is_external_without_side_effects` on Node built-ins. The cost is a second resolution, only for external dynamic imports. An external never gets a factory in the dev runtime, so `requestLazy` cannot run it. That covers every path `ctx.resolve` can mark external: a plugin `resolveId` of `{ id, external: true }`, config `external` matching the resolved id (applied after plugins in `resolve_id_check_external`), Node built-ins, and http(s) URLs (#11273, pinned by `packages/rolldown/tests/dev/dev-lazy-compile.test.ts`). Config `external` matching the raw specifier never reaches this hook: `resolve_id_check_external` applies it before any plugin runs. The rendered dynamic import stays a plain `import("<external>")`.
3. Otherwise append `?rolldown-lazy=1`. The append is **idempotent** (#9439): `ctx.resolve` can re-enter other plugins' resolve hooks (e.g. an alias plugin), so if the resolved id already ends with the marker it is reused — a doubled suffix would desync the proxy id from the runtime invalidation key in the stub template (regression vitejs/vite#22454, pinned by the aliased-import spec). The plugin stores the call (specifier, importer, kind, attributes, `custom`) in `original_calls` under the proxy id. Each resolution of the proxy replaces the stored call, so the fetch runs the call of the latest build (for example after an edit gives the import another specifier for the same file). Like `lazy_entries`, nothing is ever removed.

**Creating the actual module.** Without lazy compilation, the `resolveId` call of the `import()` creates the actual module, so the module starts with that call's `meta` and `moduleSideEffects` (see [cache/implementation.md](../cache/implementation.md)). The probe above does not create it, and the proxy result carries no options. So when the fetched template imports the actual module by its resolved id, `resolve_with_original_call` runs `ctx.resolve` again with the stored call (`skip_self: true`) in the current build and returns that result, receipt included (`HookResolveIdOutput::from_resolved_id`). User hooks see the original specifier and importer, never the resolved id with the proxy as importer, so a virtual-module plugin that resolves only its bare id (`virtual:x`, not `\0virtual:x`) works too. It falls back to `None` (the next plugins resolve the resolved id with the proxy as importer, and the module starts with empty options) when no call is stored, the stored importer is an absolute path that is no longer on disk, the call fails (the error is dropped), or the call now gives an external or another id (for example after an edit of the importer). Pinned by `packages/rolldown/tests/dev/dev-lazy-compile.test.ts`.

Re-resolution of a known proxy id (any import kind — e.g. the lazy scan, which starts at the proxy) is claimed by the lazy plugin itself: if the specifier ends with `?rolldown-lazy=1` and is present in `lazy_entries`, it resolves to itself. Unknown proxy ids fall through and stay unresolvable (cf. the #9969 gate below). User-land `resolve_id` hooks never see proxy ids at all — the `PluginDriver` skips them for `?rolldown-lazy=1` specifiers — because a virtual-module plugin would not recognize its own id with the query appended and nothing else can resolve a virtual id (regression vitejs/vite#23124, pinned by `packages/rolldown/tests/dev/dev-lazy-compile.test.ts`).

When `load` is called for a proxy module:

1. Only ids present in `lazy_entries` are served at all — any other `?rolldown-lazy=1` id falls through to `Ok(None)`
2. If in `fetched_entries` → return fetched template; otherwise → return stub template
3. User-land build hooks (`resolve_id`, `load`, `transform`, `transform_ast`, `module_parsed`) are skipped for proxy ids (`?rolldown-lazy=1`) so plugins only see real modules; the lazy plugin itself still runs to serve the stub/fetched template.

**Security gate — unknown module rejection (#9969)**: a lazy pull names its target by a session number, never by a path. `ClientHave::parse_query` rejects a number the session did not give out (a Reload), and `HmrStage::lazy_routes` maps each number to the target and its proxy through the module table. A number whose target and proxy are both missing fails with `Lazy entry module not found in cache. module_id=<stable id>`. So a request cannot bundle an arbitrary file (analogous to Vite's `server.fs.strict`; pinned by `dev-lazy-compile.test.ts` and `a_lazy_pull_of_a_module_not_in_the_graph_fails` in `dev_engine.rs`). `mark_as_fetched` runs only in the scan branch, after this gate, so a bad request never writes `fetched_entries`.

### Lazy Payload Rendering

`DevEngine::render_patch(query)` (`dev_engine.rs`) parses the query into a `ClientHave` (`crates/rolldown_common/src/hmr/client_have.rs`), with the lazy targets in `have.lazy`. Then `Bundler::render_patch(&have, &mut merged)` (`impl_bundler_hmr.rs`) → `HmrStage::render_patch(&have, lazy_ctx, &mut merged)`:

1. Read the catch-up from the session (`HmrSession::catch_up`): a `g` behind the session gets a `catchUp` statement, an unknown epoch or a `g` too old to catch up gets `Reload`. Nothing on the server changes before this check
2. `lazy_routes` maps each number to its target and proxy (the #9969 gate). `scan_lazy_routes` scans the proxies whose target is not in the graph, or whose own entry waits in `pending_rescans`: it marks each one as fetched (`mark_as_fetched` returns whether it inserted the id), runs `ScanMode::Partial([proxies' resolved ids])` with `set_retry_pending_rescans(false)`, so it takes from `pending_rescans` only its own ids, then `merge` and `apply_module_side_effects_overrides`. A failed scan undoes the marks it made (`unmark_fetched`) before it returns the error. The out-parameter `merged` becomes `true` just before `merge`, so `DevEngine` can tell a pull that failed after its `merge` from one that failed before it. A repeat visit skips all of this (see [design.md](./design.md#6-repeat-visits))
3. The roots are the update set `u` and the lazy targets. A lazy target the client ran keeps its exports and is skipped. Every other root the client does not hold current is carried. `collect_unheld_sync_deps` walks the static deps of the roots. A module the client ran (`e`) **stops** the walk. A module whose current factory the client holds (`holds_current`: in `f`, stamp at or below `g`) is not carried, but the walk goes through it: a patch may have carried it to a client that never ran it, so its deps may be missing. Every other module is carried; external modules are dropped and the rest sorted by id. The proxy is never carried: it never runs in the client, because `requestLazy` runs the real module. The rules are the ones of a patch (see [hmr/design.md](../hmr/design.md), principle 7)
4. Each module is rendered by `HmrAstFinalizer` into a factory registration (`impl_traverse_for_hmr_ast_finalizer.rs`):

   ```js
   __rolldown_payload__.registerFactory('src/foo.js', function (__rolldown_module_id__) {
     try {
       // registerModule/createModuleHotContext use __rolldown_module_id__;
       // ESM exports are published as:
       // var __rolldown_exports__ = __rolldown_runtime__.__exportAll({ ... })
     } finally {
     }
   });
   ```

   (CJS modules get `var __rolldown_module__ = { exports: {} }` / `var __rolldown_exports__ = __rolldown_module__.exports` locals at the top of the body.) The same shape serves every pull; re-execution is decided by the module cache, not by the wrapper.

   A module prints the same factory code in every payload, wherever it sits in the payload. So the binding names do not use the payload position (`hmr_import_binding_name` in `hmr_ast_finalizer.rs`):
   - a normal importee is a `var` inside the factory: `import_<importee repr>_<record id>`
   - an external importee is an `import * as` at the payload top level, shared by every module in the payload: `import$<escaped importer stable id>_<record id>_<importee repr>`. The escape keeps ASCII letters and digits and writes every other byte as `$` plus two hex digits, so two importers never get the same name

5. Dynamic imports inside the rendered modules are rewritten:
   - importee id contains `?rolldown-lazy=1` (a nested lazy proxy) → `__rolldown_runtime__.requestLazy("<stableRealId>"[, 1])` — the same shape the full build emits, built by `create_request_lazy_call` in `crates/rolldown/src/hmr/utils.rs` (pinned by the nested-dynamic-import spec). The emitted code carries the stable id only; the runtime builds the request (see [design.md](./design.md#4-dev-server-integration)). The trailing `1` is the importer's node-mode interop flag; whether the importee is CommonJS is decided in the runtime from the module's own registration (see design.md "CommonJS interop happens in the runtime")
   - ordinary `import()` → `__rolldown_runtime__.loadDynamic("<stableId>")` (`try_rewrite_dynamic_import` in `hmr_ast_finalizer.rs`); it runs the module when the client can and pulls it first otherwise (see [hmr/design.md](../hmr/design.md), principle 5). A CommonJS importee is known here, so the call becomes `loadDynamic("<stableId>").then(() => __rolldown_runtime__.__toESM(__rolldown_runtime__.loadExports("<stableId>")[, 1]))`
6. The payload starts with the prelude `const __rolldown_payload__ = __rolldown_runtime__.beginPayload(G)`, then the `catchUp` statement when one is needed, then one `__rolldown_payload__.registerGraph(delta)` statement (`render_register_graph_source`, `module_graph_delta.rs`) with `nums`, and then carries `__rolldown_payload__.registerFactory` calls only — no execute-entry tail. Numbers are assigned to the carried modules and the targets of their edges before the rows render. `requestLazy` runs the module once the payload has evaluated, so a throw from the module body reaches the importer's `await import(...)` instead of becoming a floating rejection
7. The result is returned as `HmrPullResponse::Payload(HmrPatch { code, sourcemap })` (see "Sourcemaps"). For a pull with `l`, one `tracing::debug!` line (target `hmr`, message `render_patch: lazy request done`) logs `scanned`, `modules` (the payload size) and the render time; the scan logs its own time (message `render_patch: scanned lazy routes`). The napi layer turns the payload into `{ type: 'Patch', code, sourcemap? }`, and a reload into `{ type: 'Reload', reason }` (`BindingPatchResponse`)

### Emitted Assets (#9815)

Lazy pulls (like HMR updates) never run the generate stage, so assets emitted during the scan have no `onOutput` path. Instead, after a scan that merged (also when a later step failed), `DevEngine::render_patch` drains `file_emitter.add_additional_files` into a `BundleOutput` and fires the `onAdditionalAssets` dev callback **before** returning the code — so the consumer can register/serve the assets (test-dev-server puts them in `memoryFiles`) before the browser requests them (fixes vitejs/vite#22596, pinned by the emitted-asset spec).

Design constraint for consumers: asset URLs must be resolved **eagerly at `load`** (`emitFile` + `getFileName`, as the dev server's Vite-style asset plugin does) — a `renderChunk`-time placeholder scheme would leak, because the lazy render path never runs `renderChunk`.

### Notifying the Coordinator

After a lazy pull, the dev engine does two things, in order:

```rust
// In DevEngine::render_patch, for a pull with `l` that got no Reload
if merged { /* 1. deliver the assets emitted during the scan */ }
// 2. watch the scan's files; recover a lost HMR update
let recover_lost_update = bundler.has_lost_hmr_update();
if merged || recover_lost_update {
  send(CoordinatorMsg::LazyCompiled { watch_files, recover_lost_update });
}
```

The coordinator handles `LazyCompiled`:

1. Register the watch list the message carries (see "Data Lifecycle → Build Output Refresh" for why)
2. Stop here when `recover_lost_update` is false
3. Queue a `TaskInput::FullBuild`, unless one is queued already
4. Set `has_stale_bundle_output = true`
5. Schedule build if stale (runs immediately only when the coordinator is Idle/Failed; otherwise waits in the queue)

A scan that fails before its `merge` sends nothing: the graph did not change, and the proxy is a stub again. A failed lazy scan queues its proxy in `pending_rescans` (if the graph still imports it), next to the files that were already queued.

### Error Handling

The error contract:

- **Failures cross the napi boundary as diagnostics**, not as a message: `render_patch` returns `BindingResult<BindingPatchResponse>` and `DevEngine.renderPatch` unwraps it, the same contract the other single-result methods use (see `internal-docs/dev-engine/implementation.md` §16b). This is what keeps a throwing plugin identifiable — a lazy scan is the first time a lazy module runs the plugin pipeline, so the caller gets the plugin name, the hook, the module id and the original stack rather than a Debug-formatted string. Pinned by a `dev-lazy-compile.test.ts` case
- **Unknown module number** → `Err("Lazy entry module not found in cache. module_id=...")` in `HmrStage::lazy_routes`. Vite logs every error with `logger.error` ("Failed to render the HMR payload") and answers HTTP 500 itself, with no error overlay (success sets `Content-Type: text/javascript`)
- **A client that must reload** → `{ type: 'Reload', reason }`, not an error: a query that does not parse, another engine's epoch, or a `g` the session cannot catch up. The check runs before any scan, so the server state does not change. Vite serves it with status 200 and a body that calls `__rolldown_runtime__.reloadNeeded(reason)`, so the page reloads even when the `import()` that started the load has a `catch` (see [design.md](./design.md#4-dev-server-integration))
- **A payload that is still not runnable** → `requestLazy` pulls at most twice (a second request describes the client as it is then). If the module still cannot run, it calls `hooks.onMissingFactory` and rejects with `MissingFactoryError`. A failed load (this case, an HTTP 500 or a network error) is not memoized, so the next `import()` asks again. A failed pull puts the `droppedDeps` it took back
- **A client that missed an update** → `requestLazy` pulls even when it could run the module from the factories it holds, so it gets the catch-up before it runs a factory that may be old (`missedUpdate()`)
- **Init errors are catchable (#9981)**: the lazy payload only registers factories; `requestLazy` runs `initModule` itself once the payload has evaluated, inside the promise it hands back to the consumer. An error thrown while the lazy module initializes therefore rejects that promise, hence the consumer's `await import(...)` — try/catch works, and without a handler exactly one `unhandledrejection` fires. This rejection is memoized like a native `import()` of a throwing module; a retry could not work, since a factory registers its module before running its body. Pinned on both the **cold** path (first lazy pull) and the **warm** path (a reload, then a repeat visit served from the graph) by the lazy-init-error specs (#9975 added the original failing spec; #9981 rewrote and split it)
- **Runtime `loadExports` miss** does not throw — it warns and returns `{}`
- **No module graph** (no build ran, or the last full build failed) → `Reload`

### The Have-Set Query

- Built by `DevRuntime#encodeHave(updateIds, lazyIds)` (`runtime-extra-dev-common.js`) when `pullPatch` runs: `s=…&g=…&u=…&l=…&e=…&f=…&n=…`
- `pullPatch` gets the query from `requestQuery` and the URL from `patchUrl(query)`. Each set is a `b` bitset or a `d` gap list (see [hmr/design.md](../hmr/design.md), principle 6). A query longer than `maxQueryLength` is POSTed first, and the URL carries only `t=<token>&n=<nonce>`; the dev server resolves the token before it calls `renderPatch`
- Besides naming the lazy target, its role is **payload sizing**: the returned payload omits what the client ran or holds current
- A module with no number on the client is left out of `e` and `f`, so the server carries it. That can only add bytes
- `ClientHave::parse_query` ignores keys it does not know (`n`). A missing set is empty. A set that names a number at or above the session's number count, or is not in the wire format, makes the query fail to parse, which is a Reload

### Editing a Fetched Lazy Module

After the first lazy pull, the real module and its sync deps are ordinary watched graph modules (thanks to the coordinator registering the scan's watch list), and an edit flows through the standard watch → HMR path:

- The server has no importer walk. It stamps the changed modules and pushes their ids. A proxy is never in `changedIds` or `staleIds`: it never runs in the client, because `requestLazy` runs the real module. `rerendered_importers` (importers whose factory reads a changed interop fact) leaves proxies out too, and the modules that call `import()` of a proxy emit `requestLazy`, which reads only ids. So no payload carries a proxy (see "Lazy Payload Rendering")
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
│  - import() of a proxy → __rolldown_runtime__.requestLazy(realId[, 1])  │
│  - The rows give the real module a number (dynamic edge target)         │
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
│  - requestLazy("src/lazy-module.js") memoizes one promise per real      │
│    module id (dedup across importers)                                   │
│  - Module and its static imports all ran or have factories here         │
│    (isRunnableHere), and no update was missed → initModule, no          │
│    request. A factory alone is not enough: a patch can ship a module    │
│    without its deps (#11028)                                            │
│  - Otherwise pullPatch(∅, {id}):                                        │
│      __rolldown_hmr_patch.js?s=…&g=…&l=<num>&e=…&f=…&n=…                │
│  - Browser waits on the memoized promise                                │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 4. DEV SERVER RECEIVES THE PULL                                         │
├─────────────────────────────────────────────────────────────────────────┤
│  - Calls DevEngine.renderPatch(query)                                   │
│  - Engine parses the have-set; wrong epoch or too old → Reload          │
│  - Maps the number to the target and its proxy (security gate, #9969)   │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 5. PARTIAL SCAN + RENDER                                                │
├─────────────────────────────────────────────────────────────────────────┤
│  - First visit only: marks the proxy as FETCHED, then                   │
│    ScanMode::Partial([proxy]) (a failed scan unmarks it)                │
│  - Plugin's load hook sees proxy is fetched → returns FETCHED template  │
│  - Fetched template: import("/abs/path/lazy-module.js")                 │
│  - resolve_id sees importer is a fetched proxy → resolves the real one  │
│  - Actual module + sync deps carried — minus modules the client ran     │
│    or holds current (e / f in the query); never the proxy               │
│  - beginPayload(G), catchUp (if g is behind), registerGraph rows, then  │
│    registerFactory(stableId, fn) per module; no execute-entry tail      │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 6. RETURN COMPILED JS TO BROWSER                                        │
├─────────────────────────────────────────────────────────────────────────┤
│  - Assets emitted during the scan already delivered via                 │
│    onAdditionalAssets (#9815)                                           │
│  - LazyCompiled sent: the coordinator watches the scan's files, and     │
│    queues a FullBuild only for a lost HMR update                        │
│  - Response is a single JS string; Vite links its sourcemap (inline     │
│    only for sourcemap: 'inline')                                        │
│  - Browser loads it as an ES module; initializers register each module  │
│  - requestLazy then runs initModule("src/lazy-module.js") itself        │
│  - Original import() promise resolves with the module's exports         │
│    (or rejects catchably, #9981)                                        │
│  - Still not runnable → requestLazy pulls once more, then rejects       │
│  - The build output keeps the stub until the next build; a reload       │
│    still loads the route through a lazy pull                            │
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
- Stub tried to fetch the lazy route again
- But the actual module was never included in the returned code

**Root cause**: The proxy module content never changed after being fetched. The plugin always returned the same stub template.

**Solution**: Implement two-state proxy modules:

1. Add `fetched_entries` set to `LazyCompilationContext`
2. Mark proxy as fetched before its scan: `lazy_ctx.mark_as_fetched(&proxy_id)`, and undo it when the scan fails
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

**Solution**: No page loads a lazy route from the build output, so a lazy pull queues no build. The graph holds the route, and the next build of any kind puts it in the output. `LazyCompiled` still carries the watch list, and asks for a build only while an HMR update is lost (`recover_lost_update`).

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
2. User triggers lazy load → a pull whose `l` names the number of `src/module.js`
3. The engine maps the number to the proxy, and marks the proxy as fetched
4. Partial scan from proxy → plugin returns fetched template
5. Fetched template imports actual module → triggers compilation
6. The lazy payload registers the actual module's factory under `src/module.js`; `requestLazy("src/module.js")` then runs it and resolves the importer's promise
7. After the next build, both proxy (fetched) and actual module are in the output

### Issue 8: Proxy-ID Creation Must Be Idempotent (#9439)

**Problem**: With an alias plugin present, `ctx.resolve` re-entered the lazy plugin's `resolve_id`, appending `?rolldown-lazy=1` twice. The doubled suffix desynced the proxy id from the id the real module registers under, so the importer never saw the real exports (`mod.foo` came back undefined — regression vitejs/vite#22454).

**Solution**: Before appending the marker, check whether the resolved id already ends with `?rolldown-lazy=1` and reuse it. Pinned by the aliased-import spec.

### Issue 9: Fetched Template Must Read Exports From the Registry (#9132)

**Problem**: The fetched template originally returned the dynamic import's namespace object. When a shared lazy module landed in a common chunk, chunk-level renaming minified the export names and the namespace lookup yielded `undefined`.

**Solution**: read the exports from the runtime registry by stable id, which preserves original export names. Today `requestLazy` resolves with `initModule("<stableRealId>")` directly; the fetched template's body is never executed and its `import($MODULE_ID)` only roots the partial scan. Pinned by the shared-module spec.

### Issue 10: Init Errors Must Reject the Consumer's Promise (#9981)

**Problem**: An error thrown while a lazily-compiled module initialized escaped as an unhandled rejection instead of surfacing at the consumer's `await import(...)`.

**Solution**: the lazy payload carries no execute-entry tail. `requestLazy` runs `initModule` itself after the pull resolves, inside the promise it returns, so a throw from the module body rejects the consumer's import promise instead of floating out of an async wrapper. Pinned by the two lazy-init-error specs (a first visit, then a repeat visit after a reload).

### Issue 11: `export * as ns from` Is Not `export * from`

**Problem**: `export * as ns from './dep'` and `export * from './dep'` are the same oxc AST node (`ExportAllDeclaration`), distinguished only by whether `exported` is set. The HMR finalizer ignored that field and rendered both as a star re-export — `__reExport(__rolldown_exports__, import_dep)` — so the re-exporting module's namespace object never carried `ns`, and every consumer read `undefined`. Only the module wrappers were affected (pulled payloads); the scope-hoisted build resolves the same source correctly.

**Solution**: When `exported` is present, bind the importee's `loadExports` result under that single name in the namespace object (`{ ns: () => import_dep }`, computed when the name is not a valid identifier) and emit no `__reExport`. Pinned by `crates/rolldown/tests/rolldown/topics/hmr/export_star_as/`.

### Issue 12: Re-exports From Externals Must Keep a Real Import (#10478)

**Problem**: The dev runtime registry only holds modules this build wrapped, so an external is never in it — `loadExports('<external id>')` warns `Module <id> not found` and returns `{}`. The plain-import arm of the HMR finalizer knew this and emitted a real `import * as X from 'ext'` hoisted outside the wrapper; the three re-export arms (`export * from`, `export * as ns from`, `export { x } from`) did not, and asked the registry instead. Every re-exported name read as `undefined`, silently. When the same module also imported that external, both arms named the binding through `ensure_static_import_info` but deduplicated against different sets, so both declarations were emitted under one name — and since the real import sits at chunk top level while the `var` sits inside the factory, the `var` legally shadowed it and the module's own uses of the external broke too.

**Solution**: Route all four arms through one `create_importee_binding_stmt`, which picks `loadExports` for normal modules and a real import statement for externals. The per-kind deduplication sets then stay disjoint by construction, so the shadowing `var` cannot be emitted. Pinned by `crates/rolldown/tests/rolldown/topics/hmr/reexport_external/` (HMR patch, executed) and a `dev-lazy-compile.test.ts` case (lazy payload).

## Implementation Notes

### Directive Prologue Handling

The injected helper function is inserted **after** any directive prologues (e.g., `"use strict"`) to preserve their semantics. The plugin counts leading string literal expression statements and inserts the helper after them. The helper is only injected when at least one dynamic import in the module was actually wrapped.

## Test Coverage

E2E playground: `packages/test-dev-server/tests/playground/lazy-compilation/` (one dev server config with `experimental.devMode.lazy: true` + an alias plugin):

| Spec                        | Pins                                                                                                                                                                                                                          |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `basic`                     | lazy module arrives in exactly one JS request (a pull whose `l` names it, with no path in the URL), no stub chunk                                                                                                             |
| `aliased-import`            | idempotent proxy-id creation under alias re-entrancy (vite#22454)                                                                                                                                                             |
| `emitted-asset`             | assets emitted during lazy compile are servable on first load (vite#22596)                                                                                                                                                    |
| `have-post`                 | with `maxQueryLength = 0`, a lazy load and an HMR update each POST their query first, and the GET carries only the token                                                                                                      |
| `late-lazy-chunk`           | a lazy payload that lands after a newer HMR patch does not replace its code                                                                                                                                                   |
| `lazy-init-error`           | init errors catchable with try/catch — cold and warm paths (#9975/#9981)                                                                                                                                                      |
| `lazy-init-error-unhandled` | exactly one `unhandledrejection` without a handler — cold and warm paths                                                                                                                                                      |
| `nested-dynamic-import`     | nested lazy `import()` inside a lazy payload resolves on first click                                                                                                                                                          |
| `shared-module`             | export-name preservation in shared chunks (#9132) + watch/auto-reload after fetch + a module shared by a visited and a new route runs once after a build and a reload, and the visited route's second compile starts no build |
| `top-level-await`           | a lazy module with top-level await parses and runs (#11110); its importer gets a TDZ error                                                                                                                                    |

Separate playground: `inline-common-chunks-lazy` runs lazy compilation with `experimentalInlineCommonChunks`. Two lazy routes share modules: they run once, also after a reload, the page loads no built route chunk, and an edit of a shared module hot-updates both routes.

The specs find a lazy request by its `l` set: `lazyTargetsOf` in `playground/test-utils.ts` decodes the numbers and maps them to stable ids through the page's `__rolldown_runtime__`, because session numbers differ between runs. Several specs use `retry: 0` because the bugs only reproduce on the first interaction with a fresh server. Unit test: `packages/rolldown/tests/dev/dev-lazy-compile.test.ts` pins that a path in the query is never resolved and an unknown number gets a Reload (#9969), the `requestLazy` rewrite, that both emitters carry stable ids only and pull through `DevRuntime#importPatch` with the target number in `l`, that external import names do not clash when factories from different payloads meet in one payload, and that a lazy payload carries no factory for its proxy. Its tests build real have-sets: a `DevRuntime` runs the `registerGraph` rows of the built entry, and `encodeHave` writes the query. `crates/rolldown/src/hmr/hmr_ast_finalizer.rs` (`cargo test -p rolldown hmr_import_binding_name`) pins the escape in external import names, and that the names differ for each importer id and record number. `packages/rolldown/tests/dev/dev-watch.test.ts` pins that an HMR patch carries no factory for a proxy. `packages/rolldown/tests/dev/dev-runtime.test.ts` pins that `requestLazy` sends `l` and never a path, pulls again once and then rejects with a load failure that is not memoized (an init error is), keeps the `droppedDeps` when a lazy pull fails, posts a long query with its `l`, and that a Reload answer stops all requests and rejects `requestLazy`. `crates/rolldown_dev/src/dev_engine.rs` (`cargo test -p rolldown_dev --features testing lazy`) pins the scan and build rules: a first visit queues no build and watches the route's files; a repeat visit does not scan and serves the same code and sourcemap; a target in the graph through a static import is served with no scan and leaves the proxy a stub; a failed first scan undoes its mark as fetched, so the next full build keeps the stub; a number whose module left the graph gets the not-found error and marks nothing; a retry after a failed scan scans again with no reload, and takes its proxy out of `pending_rescans`, so the next visit does not scan; a compile that failed past its `merge` still delivers its asset and watches its files, and the next visit is a repeat visit; a broken file of another route, or a broken unrelated file, does not fail a visit; a failed visit keeps the other queued files for the next HMR task; a visit with a lost HMR update reloads every client through one full build; a deleted route file still reports its error. The tests see a scan through the `resolveId` calls of a counting plugin: user plugins never see the proxy, but a scan resolves the call that the proxy replaced. `a_lazy_pull_reloads_or_catches_up_the_client` pins that a client that must reload changes nothing, and that a client behind the session gets the catch-up. `packages/rolldown/tests/dev/dev-lazy-compile.test.ts` also pins that N lazy pulls leave one build (#11489), and that a build after a lazy pull still emits `requestLazy` for the fetched route.

## Files Changed (Reference)

For future debugging, these files handle lazy compilation:

### Core Plugin

1. **`crates/rolldown_plugin_lazy_compilation/src/lazy_compilation_plugin.rs`** - Plugin with `resolve_id` and `load` hooks; `LazyCompilationContext` with fetched-state tracking (`mark_as_fetched`, `unmark_fetched`); `render_proxy_template`
2. **`crates/rolldown_plugin_lazy_compilation/src/proxy-module-template.js`** - Stub template (not fetched)
3. **`crates/rolldown_plugin_lazy_compilation/src/proxy-module-template-fetched.js`** - Fetched template
4. **`crates/rolldown/src/hmr/utils.rs`** - `create_request_lazy_call`, the one emitted shape for a lazy boundary
5. **`crates/rolldown_plugin_hmr/src/runtime/runtime-extra-dev-common.js`** - `requestLazy`, the runtime entry point
6. **`crates/rolldown/src/utils/apply_inner_plugins.rs`** - registers the plugin when `experimental.dev_mode.lazy == true`
7. **`crates/rolldown_common/src/hmr/lazy_proxy_id.rs`** - `LAZY_PROXY_QUERY` and the proxy-id helpers (`is_lazy_proxy`, `lazy_proxy_target`) that every crate uses to recognize a proxy

### Dev Engine

6. **`crates/rolldown_dev/src/dev_engine.rs`** - `render_patch()` (query parse, asset delivery and `LazyCompiled` for a pull with `l`)
7. **`crates/rolldown_dev/src/types/coordinator_msg.rs`** - `LazyCompiled` message variant
8. **`crates/rolldown_dev/src/bundle_coordinator.rs`** - Handles `LazyCompiled` (watch the scan's files; a full build only for a lost HMR update), state machine
9. **`crates/rolldown_binding/src/binding_dev_engine.rs`** - napi surface (`render_patch` → `BindingPatchResponse`)
10. **`crates/rolldown_common/src/hmr/client_have.rs`** + **`hmr_session.rs`** - the request format (`ClientHave`, `HmrNumSet` with its two forms and decoding limits) and the session (numbers, stamps, catch-up)

### HMR/Build

11. **`crates/rolldown/src/hmr/hmr_stage.rs`** - `render_patch()`: catch-up, lazy routes (`lazy_routes`, the #9969 gate), the lazy scan (`scan_lazy_routes`), dep collection from the have-set, payload rendering
12. **`crates/rolldown/src/hmr/hmr_ast_finalizer.rs`** + **`impl_traverse_for_hmr_ast_finalizer.rs`** - `registerFactory` wrappers, dynamic-import rewrites (incl. the `requestLazy` rewrite), computed-property exports
13. **`crates/rolldown/src/hmr/utils.rs`** - register-module / hot-context statement builders (`__rolldown_module_id__` param)
14. **`crates/rolldown/src/bundler/impl_bundler_hmr.rs`** - `Bundler::render_patch` entry point, which hands the stage the `LazyCompilationContext`
15. **`crates/rolldown_plugin_hmr/src/runtime/runtime-extra-dev-common.js`** - browser runtime: `registerFactory`, `initModule` (module-cache gate), `registerModule`, `loadExports`, `requestLazy`, `pullPatch`, `encodeHave`, `requestQuery`, `patchUrl`, `haveUrl`

### Reference Dev Server (Vite full bundle mode, vendored at `vite/` (repo root))

16. **`packages/vite/src/node/server/middlewares/bundledDevPayload.ts`** - `bundledDevPatchMiddleware`, the one pull route (500 with no error overlay on error, `text/javascript` on success and for a Reload body, through `sendPayload`, which ends every payload with `; export {}` so it cannot load as a classic script; the map store follows `lazy`)
17. **`packages/vite/src/node/server/bundledDev.ts`** - `renderPatch` (resolves a token once, fills in `g` when the query has none, then `devEngine.renderPatch(query)`; returns `{ payload, lazy }`, with `lazy` from the resolved query's `l`; a `Reload` becomes the `reloadNeeded` body), `onAdditionalAssets` storage, rebuild/reload handling
18. **`packages/vite/src/module-runner/runner.ts`** - `_importPayload` and `releasePayloadMaps`: a payload's sourcemap stays while a module it installed is installed
19. **`packages/vite/src/node/plugins/asset.ts`** - the bundled-dev branch resolves asset imports eagerly at `load`

## References

- [design.md](./design.md) — goals, scope, and key design decisions
- Current implementation: `crates/rolldown_plugin_lazy_compilation/`
- Dev engine: `crates/rolldown_dev/` (see also `internal-docs/dev-engine/`)
- Example: `examples/lazy-compilation/`
