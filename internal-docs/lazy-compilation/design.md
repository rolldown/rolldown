# Lazy Compilation — Design

> The implementation — data lifecycle, module-ID handling, end-to-end flow, and lessons learned: see [implementation.md](./implementation.md).

## Key Notes (TL;DR)

1. **Transparent UX** - `import('./module')` just works, no user code changes (future goal)
2. **Dynamic imports only** - static imports always compiled immediately
3. **`rolldown:exports` contract** - proxy modules export this; POC uses `lazyMagic` helper, later Rolldown runtime will unwrap automatically
4. **Compilation granularity** - lazy module + all sync deps; nested `import()` become new lazy boundaries
5. **Dev server returns JS directly** - `/lazy` request returns compiled code, browser loads it as an ES module
6. **Module IDs** - use **absolute paths** (`module.id`) consistently throughout the runtime
7. **Proxy module states** - proxies have two states: **not fetched** (stub template) and **fetched** (imports real module)
8. **Build output refresh** - after lazy compilation, dev engine triggers rebuild to update build output
9. **Caching** - AST cached internally; duplicate execution across entries is acceptable for POC
10. **Error handling** - `Err` or panic is fine for POC
11. **ClientId** - tracks multiple browser tabs/clients

## What is Lazy Compilation?

Lazy compilation is a **development optimization** that defers compilation of dynamically imported modules until they are actually requested at runtime.

### Goals

1. **Faster cold starts** - Only compile entry points and their synchronous dependencies on startup
2. **On-demand compilation** - Code behind `import()` is compiled just-in-time when the browser executes it
3. **Transparent to users** - No code changes required; `import('./foo')` should just work

## Scope

- **Dynamic imports only** (`import()`) - static imports are always compiled
- **Standalone feature** - Reuses the HMR runtime/rendering path for module output, but does not emit HMR updates

## Compilation Granularity

When a lazy module is requested:

- Compile **that module + all its synchronous dependencies**
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

## Key Design Decisions

### 1. Transparent User Experience

Users should not need to change their code. `import('./module')` just works.

### 2. The `rolldown:exports` Contract

Proxy modules export a special named export `'rolldown:exports'`:

```js
// Proxy module for lazy ./foo.js (NOT EXECUTED state)
const lazyExports = (async () => {
  await import(`/@vite/lazy?id=${encodeURIComponent($PROXY_MODULE_ID)}&clientId=...`);
  return __rolldown_runtime__.loadExports($MODULE_ID);
})();

export { lazyExports as 'rolldown:exports' };
```

- `'rolldown:exports'` is a promise that resolves to the real module's exports
- Rolldown's `transform_ast` hook automatically wraps all dynamic imports with an unwrapping helper:

  ```js
  // User code (unchanged)
  const mod = await import('./lazy.js');

  // Transformed by lazy compilation plugin
  const mod = await import('./lazy.js').then(__unwrap_lazy_compilation_entry);
  ```

- The helper is injected into each module that has dynamic imports:

  ```js
  function __unwrap_lazy_compilation_entry(m) {
    var e = m['rolldown:exports'];
    return e ? e : m;
  }
  ```

- This is safe for ALL dynamic imports: lazy modules return the promise, non-lazy modules pass through unchanged

### 3. Proxy Module States

A proxy module has two states that determine what content the `LazyCompilationPlugin` returns:

#### Not Executed (Initial State)

Returns the **stub template** that fetches via `/lazy` endpoint:

```js
// proxy-module-template.js
const lazyExports = (async () => {
  await import(
    `/@vite/lazy?id=${encodeURIComponent($PROXY_MODULE_ID)}&clientId=${__rolldown_runtime__.clientId}`
  );
  return __rolldown_runtime__.loadExports($MODULE_ID);
})();

export { lazyExports as 'rolldown:exports' };
```

#### Fetched (After First Request)

Returns the **fetched template** that directly imports the real module:

```js
// proxy-module-template-fetched.js
const lazyExports = (async () => {
  const mod = await import($MODULE_ID);
  return mod;
})();

export { lazyExports as 'rolldown:exports' };
```

The state transition is managed by `LazyCompilationContext.mark_as_fetched()`.

### 4. Dev Server Integration

The dev server handles `/@vite/lazy?id=...&clientId=...` requests:

1. Receive request with **proxy module ID** (e.g., `/abs/path/foo.js?rolldown-lazy=1`)
2. Call `DevEngine.compile_lazy_entry(proxyModuleId, clientId)` (Rust) / `DevEngine.compileEntry(moduleId, clientId)` (TS)
3. DevEngine marks the proxy as fetched
4. Partial scan from proxy module - plugin returns fetched template
5. Fetched template's `import($MODULE_ID)` triggers compilation of actual module
6. **Return compiled JS directly** - browser loads it as an ES module
7. **Notify coordinator** - trigger rebuild to update build output for future page loads

## Related

- [implementation.md](./implementation.md) — the lazy-compilation implementation
