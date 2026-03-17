# Wrapped Modules

## What is a wrapped module?

A **wrapped module** is a module whose top-level code is enclosed in a factory function (`init_xxx` for ESM, `require_xxx` for CJS) so that execution is **deferred** until the wrapper is explicitly called. Loading a wrapped module only defines the function — the module's side effects don't run until the init/require call.

```js
// Unwrapped (inline) — runs on load
var value = 'hello';

// Wrapped (deferred) — runs when init_foo() is called
var value;
var init_foo = __esmMin(() => {
  value = 'hello';
});
```

## When does wrapping happen?

Wrapping is controlled by `WrapKind` (None / Cjs / Esm) on each module's linking metadata. A module gets wrapped in these situations:

### Always (regardless of `strictExecutionOrder`)

1. **CJS module imported via `require()`** → `WrapKind::Cjs`
2. **ESM module imported via `require()`** → `WrapKind::Esm`
3. **CJS entry module** (when output format is ESM) → `WrapKind::Cjs`
4. **`import()` with code splitting disabled** (IIFE) — dynamic import becomes synchronous require, so the importee is wrapped
5. **Transitive wrapping** — if a module is wrapped, all its static dependencies are also wrapped recursively

### With `strictExecutionOrder: true`

6. **All modules** (except leaves that can be inlined with `onDemandWrapping`) → `WrapKind::Esm` or `WrapKind::Cjs` based on exports kind

### The init call graph

When module A imports wrapped module B, A's output code contains `init_B()` (ESM) or `require_B()` (CJS). These calls form the **init call graph** — a runtime dependency graph that controls execution order.

```
a.js → b.js → c.js    (import chain)

init_a = __esmMin(() => {
  init_b();             // ← init call
  ...
});

init_b = __esmMin(() => {
  init_c();             // ← init call
  ...
});
```

## Optimization: transitive init-call reduction

### Problem

Every `import` of a wrapped module generates an `init_xxx()` call. When a module imports multiple wrapped dependencies that form a chain, many calls are redundant:

```js
// a.js imports b.js and c.js
// b.js imports c.js

// Without reduction:
init_a = __esmMin(() => {
  init_b(); // init_b already calls init_c internally
  init_c(); // ← redundant
});

// With reduction:
init_a = __esmMin(() => {
  init_b(); // sufficient — cascades to init_c
});
```

### Algorithm

1. **Build adjacency list**: for each module, collect direct dependencies where `wrap_kind == WrapKind::Esm` and `ImportKind::Import`.

2. **Compute transitive reach** via memoized DFS: `reach(m)` = set of all wrapped ESM modules transitively reachable from `m` through static imports. Cycles are handled conservatively (empty reach set for modules mid-computation).

3. **Transitive reduction** per module: given direct wrapped deps `{d1, d2, ..., dn}`, drop `di` if any other retained dep `dj` has `di ∈ reach(dj)`. Only consider deps still in the minimal set as "covering" — this prevents circular deps from eliminating all members.

### Scope

This optimization applies to **all wrapped ESM modules**, not just those from `strictExecutionOrder`. Any scenario that produces `WrapKind::Esm` (require of ESM, transitive wrapping, etc.) benefits from init-call reduction.

CJS wrappers (`require_xxx`) are excluded because they have different call semantics — `require()` returns the module's exports object, so the call can't simply be removed.
