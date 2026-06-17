# Lazy Compilation — Implementation

> Goals, scope, and key design decisions live in [design.md](./design.md).

## Data Lifecycle

### Overview

Lazy compilation involves data at two scopes:

1. **Session Scope** - Shared by all browser tabs, lives for the entire dev server lifetime
2. **Client Scope** - Per browser tab, identified by `clientId`

### Session Scope

Data shared across all connected browser tabs:

| Data              | Description                                                     |
| ----------------- | --------------------------------------------------------------- |
| Module Graph      | All resolved and compiled modules                               |
| `lazy_entries`    | Set of proxy module IDs discovered during resolution            |
| `fetched_entries` | Set of proxy modules that have been fetched via `/lazy` request |
| Build Output      | Bundled JS files on disk/memory                                 |
| Watched Files     | Files monitored for changes                                     |

**Key behavior**: Once a lazy module is fetched by any client, all subsequent clients receive the fetched template (which imports the real module directly). The build output is refreshed after lazy compilation, so future page loads get the fetched template without needing a `/lazy` request.

### Client Scope

Data specific to each browser tab:

| Data               | Description                                                                   |
| ------------------ | ----------------------------------------------------------------------------- |
| `clientId`         | Unique identifier for the browser tab                                         |
| `executed_modules` | Modules the browser has actually executed (used for HMR boundary computation) |

### Fetched vs Executed

These are distinct concepts at different scopes:

- **Fetched** (session-level): The browser sent a `/lazy` request for this proxy module. The server has compiled the actual module and its dependencies. All clients now receive the fetched template.

- **Executed** (client-level): The browser has actually run the module's code. Used for HMR to determine which modules need updates for a specific client.

A module can be fetched but not executed by a particular client (e.g., Client A fetched it, Client B hasn't navigated to that route yet).

### Build Output Refresh

After successful lazy compilation:

1. `DevEngine` notifies the coordinator via `ModuleChanged` message
2. Coordinator queues a `Rebuild` task and marks output as stale
3. Rebuild updates build output with fetched template
4. Future page loads get fetched template directly (no `/lazy` request needed)

### Known Limitations

#### Race Condition in Shared Module Deduplication

When multiple lazy entries share common dependencies, the server filters out modules the client has already executed using `executed_modules` (populated via `hmr:module-registered` messages from the browser).

```
Entry
├── import('./lazy-a')  ← lazy boundary
│   └── shared.js (sync dep)
└── import('./lazy-b')  ← lazy boundary
    └── shared.js (sync dep)
```

**Normal flow (works correctly):**

1. Browser requests `/@vite/lazy?id=lazy-a` → Server returns patch with `lazy-a` + `shared.js`
2. Browser executes patch → `shared.js` runs, sends `hmr:module-registered`
3. Server updates `executed_modules` with `shared.js`
4. Browser requests `/@vite/lazy?id=lazy-b` → Server filters out `shared.js`
5. Server returns patch with `lazy-b` only → No duplicate execution ✓

**Race condition (edge case):**

If the browser sends two `/@vite/lazy` requests in rapid succession (before the `hmr:module-registered` message from the first patch arrives), the server may not know about executed modules yet:

1. Browser requests `/@vite/lazy?id=lazy-a`
2. Browser immediately requests `/@vite/lazy?id=lazy-b` (before `lazy-a` patch executes)
3. Server returns both patches with `shared.js` included
4. Browser executes both → `shared.js` runs twice ✗

**Potential future enhancement:** Add a runtime guard in generated init functions to check if a module is already registered before executing:

```javascript
function init_shared_0() {
  // Guard: skip if already initialized
  if (__rolldown_runtime__.modules['shared.js']) {
    return;
  }
  // ... module code
}
```

This would provide defense-in-depth against the race condition.

## Implementation Details

### Module ID Format

**IMPORTANT**: All runtime module lookups use **stable IDs** (`stable_id`), which are relative paths from the cwd (e.g., `src/module.js`).

This ensures consistency between:

- Proxy module's `loadExports("src/module.js")` call
- Compiled module's `registerModule("src/module.js", ...)` call
- `createModuleHotContext("src/module.js")` call
- `import.meta.hot.accept("src/dep.js", ...)` specifiers
- `applyUpdates([["src/boundary.js", "src/acceptedVia.js"]])` boundaries

The lazy compilation plugin computes the stable ID in-place using the `cwd` obtained from the `build_start` hook.

Note: The proxy module's `/@vite/lazy?id=...` request still uses the absolute path (with `?rolldown-lazy=1`), and the fetched template's `import($MODULE_ID)` also uses the absolute path for module resolution.

### Fetched State Tracking

The `LazyCompilationPlugin` maintains two sets in `LazyCompilationContext`:

- `lazy_entries` - All proxy module IDs created during resolution
- `fetched_entries` - Proxy module IDs that have been fetched (requested at runtime via `/lazy`)

When `resolve_id` is called for a dynamic import:

1. If importer is a **fetched proxy** → return `None` (skip proxy creation, resolve to actual module)
2. Otherwise → create proxy module ID and add to `lazy_entries`

When `load` is called for a proxy module:

1. If in `fetched_entries` → return fetched template
2. Otherwise → return stub template

### Build Output Refresh

After successful lazy compilation, the dev engine notifies the coordinator:

```rust
// In DevEngine::compile_lazy_entry
if result.is_ok() {
  self.notify_module_changed(proxy_module_id);
}
```

The coordinator handles `ModuleChanged`:

1. Queue a `TaskInput::Rebuild` with the module as changed
2. Set `has_stale_bundle_output = true`
3. Schedule build if stale

This ensures future page loads get the fetched template directly (no `/lazy` request needed).
Note: the current implementation notifies with the proxy module ID (includes `?rolldown-lazy=1`), so the rebuild path should resolve or normalize it to a real module ID.

### Caching Strategy

- AST and compilation results are cached internally - that's sufficient for now
- **Known limitation (POC)**: Two different entries importing the same module may cause it to execute twice
- This is acceptable for the initial implementation

### Error Handling

- Compilation errors: Return `Err` or panic - fine for POC
- No graceful error recovery needed initially

### ClientId

- Used to track multiple browser tabs/clients
- Each browser tab gets a unique `clientId`
- Dev server uses this to route compiled modules to the correct client

## End-to-End Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│ 1. INITIAL BUILD                                                        │
├─────────────────────────────────────────────────────────────────────────┤
│  - Entry + sync dependencies compiled normally                          │
│  - Dynamic imports (import()) → replaced with proxy modules             │
│  - Proxy module ID: /abs/path/module.js?rolldown-lazy=1                 │
│  - Proxy contains STUB template (fetches via /lazy endpoint)            │
│  - Proxy exports 'rolldown:exports' promise                             │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 2. BROWSER LOADS INITIAL BUNDLE                                         │
├─────────────────────────────────────────────────────────────────────────┤
│  - Runtime initializes                                                  │
│  - Proxy module registers: registerModule("/abs/.../mod.js?rolldown-lazy=1")
│  - Stub template is ready to fetch on demand                            │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 3. USER CODE HITS: import('./lazy-module')                              │
├─────────────────────────────────────────────────────────────────────────┤
│  - Proxy module executes (stub template)                                │
│  - Fetches: /@vite/lazy?id=/abs/path/lazy-module.js?rolldown-lazy=1&clientId=xxx
│  - Browser waits on the promise                                         │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 4. DEV SERVER RECEIVES /lazy REQUEST                                    │
├─────────────────────────────────────────────────────────────────────────┤
│  - Receives proxyModuleId = "/abs/path/lazy-module.js?rolldown-lazy=1"  │
│  - Calls DevEngine.compile_lazy_entry(proxyModuleId, clientId)          │
│  - DevEngine marks proxy as FETCHED in LazyCompilationContext           │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 5. PARTIAL SCAN FROM PROXY MODULE                                       │
├─────────────────────────────────────────────────────────────────────────┤
│  - ScanMode::Partial([proxyModuleId])                                   │
│  - Plugin's load hook sees proxy is fetched → returns FETCHED template  │
│  - Fetched template: import("/abs/path/lazy-module.js")                 │
│  - Plugin's resolve_id sees importer is fetched proxy → returns None    │
│  - Dynamic import resolves to ACTUAL module (no new proxy)              │
│  - Actual module + sync dependencies are compiled                       │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 6. RETURN COMPILED JS TO BROWSER                                        │
├─────────────────────────────────────────────────────────────────────────┤
│  - Response contains:                                                   │
│    - Proxy module (with fetched template)                               │
│    - Actual module (/abs/path/lazy-module.js)                           │
│    - All sync dependencies of actual module                             │
│  - Browser loads the code as an ES module                               │
│  - registerModule() called for each module                              │
│  - loadExports() finds actual module → returns real exports             │
│  - Original import() promise resolves                                   │
└─────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ 7. BUILD OUTPUT REFRESH (Background)                                    │
├─────────────────────────────────────────────────────────────────────────┤
│  - DevEngine sends CoordinatorMsg::ModuleChanged { proxyModuleId }      │
│  - Coordinator queues TaskInput::Rebuild                                │
│  - has_stale_bundle_output = true                                       │
│  - Rebuild updates build output with fetched template                   │
│  - Future page loads get fetched template directly (no /lazy needed)    │
└─────────────────────────────────────────────────────────────────────────┘
```

## Lessons Learned

### Issue 1: Module ID Consistency is Critical

**Problem**: The proxy module, compiled module, and HMR runtime must use the same ID format for module lookups to work.

**Solution**: Use **stable IDs** (`stable_id`, relative paths from cwd) consistently in the runtime:

- `registerModule(stableId, exports)`
- `loadExports(stableId)`
- `createModuleHotContext(stableId)`
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

**Solution**: Notify the coordinator to trigger a rebuild after successful lazy compilation (ideally with a real module ID):

```rust
// In DevEngine::compile_lazy_entry
if result.is_ok() {
  self.notify_module_changed(proxy_module_id);
}

// notify_module_changed sends:
CoordinatorMsg::ModuleChanged { module_id }

// Coordinator handles it:
self.queued_tasks.push_back(TaskInput::Rebuild { changed_files });
self.has_stale_bundle_output = true;
```

### Issue 5: Non-Identifier Export Names Need Computed Property Syntax

**Problem**: The HMR finalizer was generating invalid JavaScript:

```js
// INVALID - colon in identifier
var exports = __rolldown_runtime__.__export({ rolldown:exports: () => lazyExports });
```

**Solution**: Use `is_validate_identifier_name()` to detect non-identifier export names and use computed property syntax:

```rust
let computed = !is_validate_identifier_name(exported.as_str());
self.snippet.object_property_kind_object_property(exported, expr, computed)
```

This generates valid JavaScript:

```js
// VALID - computed property
var exports = __rolldown_runtime__.__export({
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

- **Proxy module**: `/abs/path/module.js?rolldown-lazy=1` (loaded initially, contains stub/fetched code)
- **Actual module**: `/abs/path/module.js` (compiled on-demand, contains real code)

The flow is:

1. Initial build creates proxy at `module.js?rolldown-lazy=1` with stub template
2. User triggers lazy load → `/@vite/lazy?id=...?rolldown-lazy=1`
3. DevEngine marks proxy as fetched
4. Partial scan from proxy → plugin returns fetched template
5. Fetched template imports actual module → triggers compilation
6. Both proxy (fetched) and actual module are in the output
7. `loadExports("/abs/path/module.js")` finds and returns the exports

## Implementation Notes

### Naming Convention for Injected Helpers

The lazy compilation plugin injects helper functions with double-underscore prefix (e.g., `__unwrap_lazy_compilation_entry`). This is a standard convention for internal/reserved identifiers in JavaScript bundlers and should not conflict with user code.

### Directive Prologue Handling

The injected helper function is inserted **after** any directive prologues (e.g., `"use strict"`) to preserve their semantics. The plugin counts leading string literal expression statements and inserts the helper after them.

## Files Changed (Reference)

For future debugging, these files handle lazy compilation:

### Core Plugin

1. **`crates/rolldown_plugin_lazy_compilation/src/lazy_compilation_plugin.rs`** - Plugin with `resolve_id`, `load`, and `transform_ast` hooks, `LazyCompilationContext` with fetched state tracking
2. **`crates/rolldown_plugin_lazy_compilation/src/runtime_injector.rs`** - AST visitor for transforming dynamic imports and helper function generation
3. **`crates/rolldown_plugin_lazy_compilation/src/proxy-module-template.js`** - Stub template (not fetched)
4. **`crates/rolldown_plugin_lazy_compilation/src/proxy-module-template-fetched.js`** - Fetched template

### Dev Engine

5. **`crates/rolldown_dev/src/dev_engine.rs`** - `compile_lazy_entry()`, `notify_module_changed()`
6. **`crates/rolldown_dev/src/types/coordinator_msg.rs`** - `ModuleChanged` message variant
7. **`crates/rolldown_dev/src/bundle_coordinator.rs`** - Handles `ModuleChanged`, triggers rebuild

### HMR/Build

8. **`crates/rolldown/src/hmr/hmr_stage.rs`** - `compile_lazy_entry()` partial scan logic
9. **`crates/rolldown/src/hmr/hmr_ast_finalizer.rs`** - Export generation with computed property support
10. **`crates/rolldown/src/hmr/utils.rs`** - `create_register_module_stmt()`, `create_module_hot_context_initializer_stmt()`

## References

- [design.md](./design.md) — goals, scope, and key design decisions
- Current implementation: `crates/rolldown_plugin_lazy_compilation/`
- Dev engine: `crates/rolldown_dev/`
- Example: `examples/lazy-compilation/`
