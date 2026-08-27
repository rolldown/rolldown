# Lazy Compilation — Design

> The implementation — data lifecycle, module-ID handling, end-to-end flow, and lessons learned: see [implementation.md](./implementation.md).

## Key Notes (TL;DR)

1. **Transparent UX** - `import('./module')` just works; the finalizer rewrites the import into a `requestLazy` call
2. **Dynamic imports only** - static imports always compiled immediately. Boundary creation is module-type-blind (see Scope)
3. **`requestLazy` entry point** - a lazy `import()` compiles to `__rolldown_runtime__.requestLazy(realStableId, fetchChunk)`; the runtime fetches the chunk once, then runs the module through the ordinary `initModule` gate
4. **Compilation granularity** - lazy module + the sync deps the requesting client has not yet executed; nested `import()` become new lazy boundaries
5. **Dev server returns JS directly** - `/@vite/lazy` returns the compiled code as a single JS string; the browser loads it as an ES module (only inline sourcemaps survive — see implementation.md)
6. **Module IDs** - runtime module-map lookups use **stable IDs** (cwd-relative); absolute paths appear only in the `/@vite/lazy?id=` param and the fetched template's `import($MODULE_ID)`
7. **Proxy module states** - a proxy is a server-side graph node, never fetched as a chunk. Two states: **not fetched** (empty body, keeps the real module out of the build) and **fetched** (imports the real module, rooting the partial scan)
8. **Build output refresh** - after lazy compilation, the dev engine triggers a background rebuild to update build output; the rebuild is silent to connected clients
9. **Dedup** - the server prunes modules the client already executed from the lazy patch, and lazy-chunk initializers carry a runtime dedup flag — a shared module never executes twice
10. **Error handling** - unknown module ids are rejected (security gate, #9969); init errors in lazy modules reject the consumer's `await import()` catchably (#9981)
11. **ClientId** - browser-generated UUID per tab; selects the per-client `executed_modules` set used to prune the lazy patch

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
- The plugin's `context()` exposes the shared `lazy_entries` / `fetched_entries` sets as a `LazyCompilationContext`, which is handed to the `DevEngine` so it can call `mark_as_fetched` before each lazy compile.
- rolldown-vite's bundled dev mode enables `lazy: true` by default.

## Scope

- **Dynamic imports only** (`import()`) - static imports are always compiled
- **Standalone feature** - Reuses the HMR runtime/rendering path for module output. The `/@vite/lazy` request itself emits no HMR update — but once fetched, the lazy module is an ordinary watched graph module, and later edits to it flow through the normal per-client HMR pipeline (see implementation.md "Editing a fetched lazy module")
- **Module-type-blind boundary** - `resolve_id` proxies _every_ dynamic import, with no extension or module-type filter, so the real target is not loaded until the first `/@vite/lazy` request. Everything in the compiled unit must render as an ECMAScript AST:
  - **CSS** - unsupported in rolldown (removed, #4271); lazy compilation _defers_ the hard error from server startup to the first `/lazy` request (HTTP 500, catchable rejection at the consumer's `await import()`)
  - **JSON / text / base64 / dataurl** - currently broken inside lazy chunks: their exports are synthesized at link time, which the lazy render path skips, so they register empty exports until a rebuild + page refresh (see implementation.md Known Limitations)
  - **Binary assets** - work only when a plugin converts them to JS in its `load` hook (e.g. the dev server's Vite-style asset plugin); emitted bytes are delivered via `onAdditionalAssets` (#9815)
- The compiled unit includes all static deps of the lazy module, and `new URL(...)` references count as static

## Compilation Granularity

When a lazy module is requested:

- Compile **that module + its synchronous dependencies** — minus any module the requesting client has already executed (per-client pruning via `executed_modules`)
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

Inside a rendered lazy chunk (or HMR patch), a nested `import()` of another lazy proxy is rewritten by the HMR finalizer to fetch `/@vite/lazy?...` and then read the proxy's registered exports via `loadExports(stableProxyId)` — partial bundles have no separately bundled proxy chunk, so the proxy's top-level export would otherwise be lost (see implementation.md "Lazy chunk rendering").

## Key Design Decisions

### 1. Transparent User Experience

Users should not need to change their code. `import('./module')` just works.

### 2. The `requestLazy` Entry Point

A lazy `import()` is rewritten by the finalizer into a single runtime call:

```js
// User code (unchanged)
const mod = await import('./lazy.js');

// Emitted
const mod = await __rolldown_runtime__.requestLazy(
  'lazy.js',
  () =>
    import(
      `/@vite/lazy?id=${encodeURIComponent(absProxyId)}&clientId=${__rolldown_runtime__.clientId}`
    ),
);
```

Both codegen paths emit this same shape — the full build from `try_rewrite_import_expression`, HMR patches from `rewrite_dynamic_import` — so a lazy boundary never becomes a chunk the browser fetches, and a cold lazy route costs one request.

The first argument is the **real** module's stable id, known at compile time; the boundary's own id appears only inside the thunk's URL. `requestLazy` memoizes the promise under that id, which is the dedup point: one request no matter how many places import the module. `removeModuleCache` drops the memo, so a module edited after being lazily loaded is re-run rather than served stale.

Nothing is registered under the proxy id in the client. A cache entry with no factory behind it reads as "executed" to the HMR boundary walk, which would turn a patch arriving mid-load into a full page reload.

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
const lazyExports = (async () => {
  await import($MODULE_ID);
  return __rolldown_runtime__.loadExports($STABLE_MODULE_ID);
})();

export { lazyExports as 'rolldown:exports' };
```

This body is not executed either. Its `import($MODULE_ID)` is load-bearing at **scan** time: it is the edge `compile_lazy_entry` follows to pull the real module and its dependencies into the partial build. `$MODULE_ID` is the absolute path (used for resolution only); `$STABLE_MODULE_ID` is the cwd-relative stable id.

The state transition is managed by `LazyCompilationContext.mark_as_fetched()`.

### 4. Dev Server Integration

The dev server handles `/@vite/lazy?id=...&clientId=...` requests:

1. Receive request with the **proxy module ID** (absolute path with `?rolldown-lazy=1`) and the client's UUID
2. Call `DevEngine.compileEntry(moduleId, clientId)` (TS) / `DevEngine::compile_lazy_entry` (Rust)
3. DevEngine looks up that client's `executed_modules` and marks the proxy as fetched
4. **Security gate**: the id is only a lookup key into the build cache — an id not already in the module graph is rejected with `Lazy entry module not found in cache` (never resolved from the filesystem, so a malicious request cannot bundle arbitrary files; analogous to Vite's `server.fs.strict`, pinned by test, #9969)
5. Partial scan from the proxy module - plugin returns the fetched template, whose `import($MODULE_ID)` triggers compilation of the actual module. The chunk carries `registerFactory` calls only; `requestLazy` runs the module once the chunk has evaluated
6. Assets emitted during the compile are delivered via the `onAdditionalAssets` callback **before** the code is returned, so they are servable when the chunk executes (#9815)
7. **Return compiled JS directly** (`Content-Type: application/javascript`) - the browser loads it as an ES module; compile failures answer HTTP 500
8. **Notify coordinator** - trigger a background rebuild so future page loads get the fetched template without a `/lazy` request

## Related

- [implementation.md](./implementation.md) — the lazy-compilation implementation
