# Lazy Compilation — Design

> The implementation — data lifecycle, module-ID handling, end-to-end flow, and lessons learned: see [implementation.md](./implementation.md).

## Key Notes (TL;DR)

1. **Transparent UX** - `import('./module')` just works; the finalizer rewrites the import into a `requestLazy` call
2. **Dynamic imports only** - static imports always compiled immediately. Boundary creation is module-type-blind (see Scope)
3. **`requestLazy` entry point** - a lazy `import()` compiles to `__rolldown_runtime__.requestLazy(realStableId, proxyId)`; the runtime loads the chunk once through `loadLazyChunk`, then runs the module through the ordinary `initModule` gate
4. **Compilation granularity** - lazy module + the sync deps that the requesting client neither ran nor holds a current factory of; nested `import()` become new lazy boundaries
5. **Dev server returns JS directly** - `/@vite/lazy` returns the compiled code as a single JS string; the browser loads it as an ES module. The engine returns the sourcemap next to the code, or inlines it for `sourcemap: 'inline'` (see implementation.md)
6. **Module IDs** - runtime module-map lookups use **stable IDs** (cwd-relative); absolute paths appear only in the proxy id that `requestLazy` passes to `loadLazyChunk` (sent as the `/@vite/lazy?id=` param) and the fetched template's `import($MODULE_ID)`
7. **Proxy module states** - a proxy is a server-side graph node, never fetched as a chunk. Two states: **not fetched** (empty body, keeps the real module out of the build) and **fetched** (imports the real module, rooting the partial scan)
8. **No build after a compile** - a lazy compile merges the route into the graph and queues no build: no page loads a lazy route from the build output. The next build of any kind takes the route from the graph. A repeat visit of a fetched route also skips the scan (see [Repeat Visits](#6-repeat-visits))
9. **Dedup** - the server skips modules the client already ran or holds a current factory of, as the request says (`HmrStage::compile_lazy_entry`); in the browser `initModule` runs a factory only when the module is not yet in the module cache — a shared module never executes twice
10. **Error handling** - unknown module ids are rejected (security gate, #9969); init errors in lazy modules reject the consumer's `await import()` catchably (#9981)
11. **The request describes the client** - the lazy URL carries the same have-set as an HMR patch request: epoch, known generation, executed modules and held factories (see [hmr/design.md](../hmr/design.md), principle 2). The server keeps no state per tab
12. **After boot, only factories** - a page gets scope-hoisted code only at boot; every lazy route reaches it as factories, even a route the server compiled before (see [After Boot, Only Factories](#5-after-boot-only-factories))

## What is Lazy Compilation?

Lazy compilation is a **development optimization** that defers compilation of dynamically imported modules until they are actually requested at runtime.

### Goals

1. **Faster cold starts** - Only compile entry points and their synchronous dependencies on startup
2. **On-demand compilation** - Code behind `import()` is compiled just-in-time when the browser executes it
3. **Transparent to users** - No code changes required; `import('./foo')` should just work

## Enabling

Lazy compilation is opt-in, nested inside dev mode:

```js
export default {
  experimental: {
    devMode: { lazy: true },
  },
};
```

- `devMode` alone enables the dev/HMR machinery (`HmrPlugin`); `lazy: true` additionally prepends `LazyCompilationPlugin` before user plugins (`crates/rolldown/src/utils/apply_inner_plugins.rs`).
- The plugin's `context()` exposes the shared `fetched_entries` set as a `LazyCompilationContext`, which is handed to the `DevEngine` so it can call `mark_as_fetched` before each lazy compile.
- rolldown-vite's bundled dev mode enables `lazy: true` by default.

## Scope

- **Dynamic imports only** (`import()`) - static imports are always compiled
- **Standalone feature** - Reuses the HMR runtime/rendering path for module output. The `/@vite/lazy` request itself emits no HMR update — but once fetched, the lazy module is an ordinary watched graph module, and later edits to it flow through the normal HMR pipeline (see implementation.md "Editing a fetched lazy module")
- **Module-type-blind boundary** - `resolve_id` proxies every dynamic import that resolves to a module in this build, with no extension or module-type filter, so the real target is not loaded until the first `/@vite/lazy` request. External resolutions are the exception: there is nothing to defer, and the dev runtime never gets a factory for an external, so `requestLazy` cannot run it. Config `external` and a plugin `resolveId` that returns `{ external: true }` both keep the original `import()` (#11273). Everything in the compiled unit must render as an ECMAScript AST:
  - **CSS** - unsupported in rolldown (removed, #4271); lazy compilation _defers_ the hard error from server startup to the first `/lazy` request (HTTP 500, catchable rejection at the consumer's `await import()`)
  - **JSON / text / base64 / dataurl** - currently broken inside lazy chunks: their exports are synthesized at link time, which the lazy render path skips, so they register empty exports until a rebuild + page refresh (see implementation.md Known Limitations)
  - **Binary assets** - work only when a plugin converts them to JS in its `load` hook (e.g. the dev server's Vite-style asset plugin); emitted bytes are delivered via `onAdditionalAssets` (#9815)
- The compiled unit includes all static deps of the lazy module, and `new URL(...)` references count as static

## Compilation Granularity

When a lazy module is requested:

- Compile **that module + its synchronous dependencies** — minus what the requesting client says it has. A module it ran (`e`) stops the walk: its exports stay live. A module whose current factory it holds (`f`, stamp at or below its `g`) is walked through but not carried. The request format and the rules are in [hmr/design.md](../hmr/design.md), principles 2 and 7
- Nested dynamic imports (`import()` within the lazy module) are **not** compiled - they become their own lazy boundaries
- This creates a natural "lazy boundary" at each dynamic import

```
Entry
├── sync-dep-1 (compiled immediately)
├── sync-dep-2 (compiled immediately)
└── import('./lazy-a')  ← lazy boundary
    ├── sync-dep-3 (compiled when lazy-a is requested)
    ├── sync-dep-4 (compiled when lazy-a is requested)
    └── import('./lazy-b')  ← another lazy boundary (NOT compiled yet)
```

Inside a rendered lazy chunk (or HMR patch), a nested `import()` of another lazy proxy is rewritten by the HMR finalizer into the same `requestLazy` call the full build emits (see "The `requestLazy` Entry Point" below), so a nested boundary also costs one request (see implementation.md "Lazy chunk rendering").

## Key Design Decisions

### 1. Transparent User Experience

Users should not need to change their code. `import('./module')` just works.

### 2. The `requestLazy` Entry Point

A lazy `import()` is rewritten by the finalizer into a single runtime call:

```js
// User code (unchanged)
const mod = await import('./lazy.js');

// Emitted
const mod = await __rolldown_runtime__.requestLazy('lazy.js', '/abs/path/lazy.js?rolldown-lazy=1');
```

Both codegen paths emit this same shape (`create_request_lazy_call`) — the full build from `try_rewrite_import_expression`, HMR patches from `rewrite_dynamic_import` — so a lazy boundary never becomes a chunk the browser fetches, and a cold lazy route costs one request.

```mermaid
sequenceDiagram
  participant P as page
  participant R as runtime
  participant V as dev server
  participant E as DevEngine
  P->>R: requestLazy('lazy.js', proxyId)
  alt lazy.js and its static imports already ran or have factories here
    R->>R: no request
  else
    R->>V: loadLazyChunk(proxyId): GET /@vite/lazy?id=proxyId&s=…&g=…&e=…&f=…&n=…
    V->>E: compileEntry(proxyId, query)
    E-->>V: catch-up (if needed), registerGraph, then one registerFactory per module the query lacks
    V-->>R: JS
    R->>R: still not runnable → load once more; then onMissingFactory
  end
  R->>R: initModule('lazy.js')
  R-->>P: exports
```

A route whose factories the page does not hold always takes the request path, also after a reload and also when the server compiled the route before. What the server skips on such a repeat visit is in [Repeat Visits](#6-repeat-visits).

The first argument is the **real** module's stable id, known at compile time. `requestLazy` memoizes the promise under that id, which is the dedup point: one request no matter how many places import the module. `removeModuleCache` drops the memo, so a module edited after being lazily loaded is re-run rather than served stale.

The second argument is the boundary's own id (absolute path + `?rolldown-lazy=1`). `requestLazy` passes it to `loadLazyChunk`, which makes the request (see [Dev Server Integration](#4-dev-server-integration)).

Nothing is registered under the proxy id in the client. A cache entry with no factory behind it reads as "executed" to the HMR boundary walk, which would turn a patch arriving mid-load into a full page reload.

The client graph has no proxy node either: `registerGraph` links the importer straight to the real module. A proxy node would be the real module's only importer, and since it never runs, the boundary walk would stop at it and drop the update. The client rows link the importer to the real module whether or not the proxy was fetched. The server has no importer walk of its own: it walks only down the static imports of what the client asks for.

**CommonJS interop happens in the runtime, not at the call site.** A production build rewrites `import()` of a CommonJS module into `__toESM(require_x(), isNodeMode)`, so the importer sees `default` = `module.exports`. The lazy call site cannot do the same: the importer is compiled while the importee is still a stub, so its exports kind is unknown (vitejs/vite#23558). The split is:

- The call site passes only what the importer knows: `is_node_mode` (`should_consider_node_esm_spec_for_dynamic_import`, the same choice a production build makes) as a third argument `1`.
- A CommonJS module registers itself with `registerModule(id, module, 1)`. This is the same statement for the initial bundle, HMR patches and lazy chunks (`create_register_module_stmt`).
- `requestLazy` memoizes the module run under the real id, then hands each caller `__toESM(exports, isNodeMode)` when the module registered as CommonJS, and the namespace as is otherwise. Interop is applied per caller because the mode is per importer.

Pinned by the cjs-interop browser spec (fetched and resident module) and the CommonJS tests in `dev-lazy-compile.test.ts`.

### 3. Proxy Module States

A proxy module has two states that determine what content the `LazyCompilationPlugin` returns:

#### Not Fetched (Initial State)

Returns the **stub template** (`proxy-module-template.js`), an empty module:

```js
export {};
```

The body is never executed — every `import()` of this id was rewritten to `requestLazy`. The module exists only so the graph has a node under this id, and an empty body is what keeps the real module out of the initial build.

#### Fetched (After First Request)

Returns the **fetched template** (`proxy-module-template-fetched.js`), which imports the real module:

```js
import($MODULE_ID);
```

This body is not executed either. Its `import($MODULE_ID)` matters at **scan** time only: it is the edge `compile_lazy_entry` follows to pull the real module and its dependencies into the partial build, and it keeps a fetched route in later builds. `$MODULE_ID` is the absolute path, used for resolution only.

The state transition is managed by `LazyCompilationContext.mark_as_fetched()`.

### 4. Dev Server Integration

The dev server owns the lazy endpoint URL. The client loads a lazy chunk through `DevRuntime#loadLazyChunk(proxyModuleId)` (`runtime-extra-dev-common.js`):

- The default imports `lazyUrl(proxyModuleId, query)`, which is `/@vite/lazy?id=<proxy id>&s=<epoch>&g=<knownGen>&e=<E>&f=<F>&n=<nonce>`. The query after `id` is `encodeHave` with an empty update set, the same have-set a patch request sends.
- A server that serves the endpoint at another URL overrides `lazyUrl` in its `DevRuntime` subclass. A client that does not load through `import()`, such as Vite's module runner, overrides `loadLazyChunk`.
- Emitted code carries ids only. It never contains the URL, the have-set, or `encodeURIComponent` (a user binding of that name in the importer's scope cannot break the request).

The dev server handles these requests:

1. Receive request with the **proxy module ID** (absolute path with `?rolldown-lazy=1`) and the client's have-set in the query
2. Call `DevEngine.compileEntry(moduleId, query)` (TS) / `DevEngine::compile_lazy_entry` (Rust)
3. `DevEngine::compile_lazy_entry` parses the query and checks the epoch and generation first. A bad query, another engine's epoch, or a generation the session cannot catch up gives `{ type: 'Reload', reason }` and changes nothing on the server. Otherwise it marks the proxy as fetched
4. **Security gate**: the id is only a lookup key into the build cache — an id not already in the module graph is rejected with `Lazy entry module not found in cache` (never resolved from the filesystem, so a malicious request cannot bundle arbitrary files; analogous to Vite's `server.fs.strict`, pinned by test, #9969)
5. Partial scan from the proxy module - plugin returns the fetched template, whose `import($MODULE_ID)` triggers compilation of the actual module. A repeat visit skips this scan (see [Repeat Visits](#6-repeat-visits)). The chunk carries `registerFactory` calls only; `requestLazy` runs the module once the chunk has evaluated
6. Assets emitted during the compile are delivered via the `onAdditionalAssets` callback **before** the code is returned, so they are servable when the chunk executes (#9815). A compile that fails after its merge delivers them too
7. **Return compiled JS directly** (`Content-Type: text/javascript`) - the browser loads it as an ES module; compile failures answer HTTP 500, and the user's `import()` rejects. A `Reload` is served like the one of a patch request: status 200 and a body that calls `__rolldown_runtime__.reloadNeeded(reason)`, so the page reloads and the `import()` rejects (see [hmr/design.md](../hmr/design.md), principle 6). The chunk starts with the `beginPayload(G)` prelude, and with a `catchUp` statement when the client's `g` is behind the session, as a patch does. The server records nothing about the client
8. **Notify coordinator** - the coordinator watches the files the compile loaded. It queues no build: no page loads the route from the build output, because every lazy `import()` calls `requestLazy`. The one exception is a lost HMR update (see [Repeat Visits](#6-repeat-visits))

### 5. After Boot, Only Factories

A module can reach the page in two shapes. They stop a second run in different ways, and neither knows about the other:

| Shape                                                | When the code runs              | What stops a second run                  |
| ---------------------------------------------------- | ------------------------------- | ---------------------------------------- |
| Scope-hoisted chunk (full build)                     | as soon as the file loads       | the browser, by file URL                 |
| Factory (`registerFactory`, lazy chunk or HMR patch) | when `initModule(id)` is called | the runtime, by module id (module cache) |

When one page mixes the shapes, a module can pass both checks and run twice. So:

> **A page gets scope-hoisted code only at boot. Everything that arrives after boot is a factory.**

Lazy mode follows this. The full build emits `requestLazy` for every lazy `import()`, fetched or not, so a route never loads from the build output. Example: `route-a.js` and `route-b.js` both import `store.js`. The user visits A, reloads, then visits B and then A. B arrives as factories and runs `store.js`. A also arrives as factories, and `initModule` returns the `store.js` that already ran. Had A come back as a built chunk, `store.js` would run a second time and A would get its own store. A visit starts no build, so the spec edits a file of A after the visit: the reload then serves an output built with A's fetched proxy. Pinned by the shared-module spec and by `a build after a lazy compile keeps the fetched route behind requestLazy` in `dev-lazy-compile.test.ts`.

Rejected options for making a repeat visit cheaper:

- **Redirect `/@vite/lazy` to a built chunk** (vitejs/vite#23502). It brings built chunks back after boot, so it mixes the shapes. It also needs server-side guesses: the build generation is known at WebSocket connect, not at page load; the redirect must stop after the first payload; and Vite would copy state that `DevEngine` already has.
- **A loader made by the build** (`requestLazy(id, proxyId, () => import('./x-HASH.js'))`). It mixes the shapes too, and needs page-side rules (no payload and no push since load) plus checks for timing cases.

### 6. Repeat Visits

A page that reloads loses its factories, so a visited route costs one `/@vite/lazy` request again. That compile does not need to scan:

```mermaid
flowchart TD
  R["/@vite/lazy?id=P&s=…&g=…&e=…&f=…"] --> F{"P has no resolved import() record<br/>OR P waits in pending_rescans?"}
  F -- "yes (first visit, a retry, a deleted route file)" --> Scan["partial scan of P only, merge, apply_module_side_effects_overrides"]
  F -- "no (repeat visit)" --> Sel
  Scan --> Sel["walk, render, join"]
  Sel --> MC["ModuleChanged { rebuild: has_lost_hmr_update() }<br/>watch files; queue a Rebuild only for a lost update"]
```

The rule, in `HmrStage::compile_lazy_entry`. A compile scans in two cases:

1. **The proxy is a stub (first visit).** Only the fetched template has an `import()`, so a resolved `import()` record on the proxy means the route's closure is already in the graph. This reads the graph, not `fetched_entries`: `mark_as_fetched` runs before every compile, so a retry after a failed first compile would look like a repeat visit. A failed scan is reverted and never reaches `merge`, so after a failed scan the proxy stays a stub, and the next compile scans again.
2. **The route's own proxy waits in `pending_rescans`.** Example: the route file was deleted. The delete event rescans the proxy (an importer of the deleted module), and that scan fails to resolve the file. The failed scan queues the proxy in `pending_rescans`, so the next compile scans and reports the same error, as a scan-first compile would.

Every other visit renders from the graph. A compile that failed after its `merge` (in `apply_module_side_effects_overrides`, or in the sourcemap step, where a user callback such as `sourcemapIgnoreList` can throw) left the route in the graph, so the next visit is a repeat visit. Skipping `apply_module_side_effects_overrides` then is safe: it writes only module side effects, which no HMR render reads, and every later HMR stage and build runs it again.

Why skipping is safe: with no scan, `merge` does not run and the graph does not change. A scan of the proxy would not reload the real module either, because a partial scan invalidates only the ids it lists.

**The lazy scan retries only its own proxy.** The loader option `retry_pending_rescans: false` (`ModuleLoader::set_retry_pending_rescans`) makes the scan take from `pending_rescans` only the ids it fetches. Other queued files stay queued, and a failed lazy scan adds its own ids back next to them. The HMR stage and the builds keep retrying the whole queue. Example: an edit breaks `other.js`, and the HMR scan fails. The graph keeps the last good `other.js`, which is what the tabs run, and `other.js` waits in the queue. A visit of any route is served and does not fail on `other.js`. The next file event retries `other.js` in the HMR stage. A lazy scan that retried the queue would fail on the broken file in every visit.

**No build follows a compile.** No page loads a lazy route from the build output: every lazy `import()` calls `requestLazy`, also for a route the output contains (see [After Boot, Only Factories](#5-after-boot-only-factories)). So a compile, also a first visit, leaves `has_stale_bundle_output` as it was. The graph holds the route, so the next build of any kind puts it in the output. A build per compile would cost N + 1 builds for N compiles, and each build splits the shared chunks again, so most chunk hashes change (#11489).

The one exception is a lost HMR update (`Bundler::has_lost_hmr_update`, see [dev-engine/design.md](../dev-engine/design.md#lost-hmr-updates)). Then `ModuleChanged` carries `rebuild: true`, and the coordinator queues a `Rebuild`. The task upgrades it to the full build that reloads every client. Correctness does not depend on this: the next task of any kind upgrades the same way. The visit only starts the recovery sooner.

A compile that fails after its `merge` still sends `ModuleChanged` (the coordinator must watch the route's files) and still delivers its emitted assets. The repeat visit renders from the graph and does not scan, so it would not emit them again.

Trade-offs:

- **A link-stage error of a new route shows at the next build.** The lazy compile runs no link stage. So an error such as `MISSING_EXPORT` in the new route does not fail the visit. It shows when the next build links the graph, as for an HMR patch.
- **A lazy visit is not a recovery trigger.** It queues no build, so it does not end a failed state. Only new input does (see [dev-engine/design.md](../dev-engine/design.md), principle 3).
- **A lazy visit does not retry a file that failed in an earlier scan.** Example: an edit of `a.js` is valid, but a `transform` throws once. The HMR scan reverts, the graph keeps the last good `a.js`, and `a.js` waits in `pending_rescans`. A visit of a route that imports `a.js` gets this last good `a.js`. The open tabs run the same code, because the failed update reached no tab. The engine cannot tell a one-time error from a broken file. A retry in every visit would fail every route that imports a broken file, although the graph holds a good copy. The next change of any watched file retries the whole queue. A page reload after an HMR-stage failure also recovers, through the consumer's full build.

The compile logs whether it scanned, with the payload size (`modules`) and per-phase timings (`tracing` target `hmr`, message `compile_lazy_entry: done`).

**Rejected option: a factory cache.** A repeat visit skips the scan, but it still renders every module of the route. A cache of rendered factories (one entry per module, checked against the module's stamp, dropped by `merge` when a render input changes) would let a repeat visit join the cached factories instead of rendering them. Measured on a route of about 1500 modules:

- A repeat click goes from about 10 ms to about 2.8 ms. Skipping the scan alone already brings a repeat click from about 60 ms to about 10 ms.
- The cache holds about 12 MB per 1500 modules with sourcemaps on, and about 3 MB without. Each entry keeps the code and the sourcemap, and the sourcemap's `sourcesContent` is a second copy of the module's text. It grows with every module rendered into a lazy chunk until the next full scan.
- Only lazy payloads after the first one read it. The first visit of a route, HMR patches, and the initial build gain nothing.

It was rejected because about 7 ms per repeat click does not pay for that memory. It also adds rules that must stay correct: `merge` must drop every entry whose render read a changed module, the module itself or an importee reached through its import records.

## Related

- [implementation.md](./implementation.md) — the lazy-compilation implementation
- [hmr/design.md](../hmr/design.md) — the client-side HMR model; a lazy request carries the same have-set as a patch request
