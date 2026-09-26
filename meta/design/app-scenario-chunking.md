# App Scenario — Chunking

App scenario exists because of chunking. The [forced settings](./app-scenario.md#forced-settings) aren't goals in themselves — they exist to remove constraints that prevent the chunking algorithm from doing what it should: optimize purely for loading performance, free from execution order and signature constraints.

## The problem

Real-world apps (e.g., a large SPA with ~1000 dynamic imports) produce thousands of chunks with automatic code splitting. Each chunk represents a unique reachability pattern — modules reached by the exact same set of entries are grouped together. This is _correct_ but produces too many chunks:

- **Too many network requests** — each chunk is a separate HTTP request
- **Too many tiny chunks** — modules with unique reachability patterns become their own 1-module chunks
- **No user control** — the algorithm is purely structural, with no way to express "these should load together"

Manual code splitting (groups) lets users define chunk boundaries, but without `strictExecutionOrder`, the grouping is constrained by execution order and produces facade chunks to maintain semantics.

## What app scenario enables

**Free module movement.** Any module can be placed in any chunk without correctness risk. This is the foundation — everything else builds on it.

**Unconstrained manual code splitting.** Groups are purely about loading intent. No execution order constraints, no signature constraints.

**Facade elimination.** Facade chunks (entry chunks that only re-export from other chunks) become unnecessary — the init call mechanism already guarantees correct execution order.

## Hollow chunk elimination

When code splitting pulls a dynamic entry's modules into other chunks, the dynamic entry chunk becomes **hollow** — it contains no real code, just re-exports from other chunks. This is wasteful in two ways:

1. **Extra file** — the hollow chunk is wasted bytes
2. **Loading waterfall** — browser loads hollow chunk → it triggers loading of real chunks sequentially

Any entry chunk — initial or dynamic — can become hollow when its modules are pulled into other chunks. But only **dynamic entry** hollow chunks can be eliminated, because the bundler controls the `import()` call site and can rewrite it. Initial entry chunks are the user's explicit entry points — rewriting them would break the expected load semantics.

**Example:**

```js
// app code
const lib = await import('./lib');

// Without optimization: lib.js is hollow
// lib.js (hollow chunk — solid outside, empty inside)
export { Foo } from './lib-a.js';
export { Bar } from './lib-b.js';
```

The browser must: fetch `lib.js` → parse → discover `lib-a.js` and `lib-b.js` → fetch those. Two round trips.

**Optimization:** Eliminate the hollow chunk entirely. Rewrite the `import()` call to load the real chunks directly in parallel:

```js
// Rewritten import site
const lib = await Promise.all([import('./lib-a.js'), import('./lib-b.js')]).then(([a, b]) =>
  constructLibModuleNamespace(a, b),
);
```

Now the browser loads `lib-a.js` and `lib-b.js` in parallel. No hollow chunk, no waterfall. This is what webpack does for its chunk splitting.

**Preserving execution order:** The hollow chunk's init calls ensure correct module execution order. Init is per module — the hollow chunk calls `init_module_Y()`, `init_module_Z()`, `init_module_X()` in the right sequence. Some of these are transitively covered (if X depends on Y, `init_module_X()` already calls `init_module_Y()` internally), but independent side-effect imports are not — they must be explicitly called.

When eliminating the hollow chunk, the init call sequence is **inlined** at the `import()` call site. The hollow chunk file disappears, but its init ordering is preserved:

```js
// Before: hollow chunk handles loading + init sequencing
const lib = await import('./entry');

// After: inline loading + init sequencing at the call site
var __entry_ns;
const lib = await (__entry_ns ??= Promise.all([
  import('./chunk-A.js'),
  import('./chunk-B.js'),
  import('./chunk-C.js'),
]).then(([a, b, c]) => {
  b.init_module_Y(); // side effect
  c.init_module_Z(); // side effect
  a.init_module_X(); // target module
  return Object.freeze({
    get Foo() {
      return a.Foo;
    },
  });
}));
```

**Namespace object semantics:** ESM `import()` must return a **stable module namespace object** with live getters — not a fresh aggregate object per call. The example above preserves this: `??=` ensures the promise (and its resolved namespace) is created once and reused across all `import()` call sites for the same entry. `Object.freeze` + getter preserves live binding semantics (the getter reflects the current value, not a snapshot at construction time).

**TLA (Top Level Await):** App scenario does not support TLA in the current design. TLA introduces async initialization semantics that conflict with the synchronous init call model — supporting it would require `await init_xxx()` and async `.then()` callbacks, adding complexity throughout the chunking and hollow chunk elimination pipeline. TLA support may be considered in the future if needed.

**Trade-off:** We eliminate the hollow chunk and its waterfall, but inline code at every `import()` call site. If the same hollow entry is `import()`-ed from many places, this inlined code is duplicated. A **runtime helper** can reduce this overhead — encoding the chunk list and init sequence as data passed to a shared function:

```js
const lib = await __loadChunks(
  ['./chunk-A.js', './chunk-B.js', './chunk-C.js'], // chunks to load
  (a, b, c) => {
    // init order + namespace
    b.init_module_Y();
    c.init_module_Z();
    a.init_module_X();
    return Object.freeze({
      get Foo() {
        return a.Foo;
      },
    });
  },
);
```

The `__loadChunks` helper handles namespace caching internally — it memoizes the resolved namespace per entry, so multiple `import()` call sites for the same hollow entry resolve to the same object.

**Real-world impact (large Angular app, ~1000 dynamic imports):**

| Metric                | Value                                                      |
| --------------------- | ---------------------------------------------------------- |
| Dynamic entry chunks  | 902                                                        |
| Hollow (eliminable)   | 241 (26.7%)                                                |
| Hollow total size     | 10.8 MB                                                    |
| Avg hollow chunk size | 45.8 KB                                                    |
| Largest hollow chunk  | 163.8 KB (1,653 imports, 1,730 init calls, zero real code) |

241 fewer chunks, 10.8 MB less output, and every eliminated hollow chunk removes a loading waterfall.

**Requirements:**

- `preserveEntrySignatures: false` — otherwise the entry must maintain its original export signature
- `strictExecutionOrder: true` — ensures execution order is maintained without the hollow chunk

## Capabilities needed

### Chunk merging

Small chunks should be mergeable into larger ones. When automatic splitting produces a 200-byte chunk loaded by only one route, it should be absorbable into a larger chunk that's already on that route's critical path.

**Key question:** What's the merging strategy?

- Merge into the requesting entry's chunk?
- Merge into the largest chunk that shares the same entry set?
- Merge based on size thresholds?

### Chunk count control

Users need to set an upper bound or target for chunk count. "I want at most N chunks" or "chunks should be at least X KB". The algorithm should consolidate from the structurally-correct baseline toward the target.

### Entry chunk absorption

Modules from shared chunks can be pulled into entry chunks. If a shared module is only meaningfully used by one entry's critical path, absorbing it into that entry's chunk saves a network request at the cost of duplicating it if other entries also need it (but load it lazily and rarely).

#### Initial entry absorbs modules shared with its direct dynamic imports

When an initial entry chunk and a dynamic entry chunk share modules, and that dynamic import is **directly** `import()`-ed by the initial entry, the shared modules can be placed into the initial entry chunk instead of a common chunk.

The key condition: the optimization applies only to an initial entry and dynamic entry chunks that the initial entry **directly** imports. Modules shared between two different initial entries still go into a common chunk — the optimization does not change that.

**Example:**

```
entry.js  ──imports──▶ shared.js
entry.js  ──dynamic──▶ lazy.js ──imports──▶ shared.js
```

Current output (3 chunks):

```
entry-chunk.js     → [entry.js]
common-chunk.js    → [shared.js]       ← extra request
lazy-chunk.js      → [lazy.js]
```

Optimized output (2 chunks):

```
entry-chunk.js     → [entry.js, shared.js]   ← shared absorbed into entry
lazy-chunk.js      → [lazy.js]
```

One fewer chunk, one fewer request on initial load. `lazy-chunk.js` can reference `shared.js`'s exports from `entry-chunk.js` since the entry is always loaded before any of its dynamic imports execute.

**Condition (precise):**

A module M can be absorbed into initial entry E if:

1. M is shared between E's chunk and one or more dynamic entry chunks
2. Those dynamic entries are **directly** `import()`-ed by E (not transitively via other dynamic imports)
3. M is **not** also shared with a different initial entry — if it is, M stays in a common chunk

Condition 3 is what keeps this scoped: cross-initial-entry sharing is unchanged. The optimization only eliminates common chunks that exist solely because of sharing between an initial entry and its own direct dynamic imports.

**Why this is safe:**

- The initial entry is the browser's entry point — it loads before anything else.
- A direct dynamic import from the entry is, by definition, loaded after the entry. So the entry chunk is guaranteed to be available when the dynamic chunk needs the shared modules.
- With `strictExecutionOrder: true`, execution order is controlled by init calls, not module placement. Moving M into the entry chunk doesn't change when its top-level code runs.
- With `preserveEntrySignatures: false`, the entry chunk can absorb modules without needing facade re-exports.

## Algorithm

```
1. Start from structurally-correct baseline
   - Automatic splitting by reachability pattern (current behavior)
   - Every module is wrapped (strictExecutionOrder: true)

2. Apply manual groups (if configured)
   - Move modules matching group patterns into designated chunks
   - No execution order constraints — init calls handle it
   - No signature constraints — preserveEntrySignatures: false

3. Consolidate
   - Merge chunks below size threshold into neighbors
   - Eliminate hollow chunks
   - Absorb single-consumer shared chunks into their consumer
   - Respect chunk count targets if specified

4. Optimize init calls
   - D1: Unwrap modules with stable ordering across entries
   - D2: Transitive reduction of init call sites
```

## `strictExecutionOrder` must have minimum overhead

App scenario's chunking relies entirely on `strictExecutionOrder`. This makes it a hard dependency — if the wrapping overhead is too high, the chunking gains are negated and app scenario doesn't work. Optimizing `strictExecutionOrder` is not optional, it's a prerequisite for app scenario to be viable.

Two directions (detailed in [strict-execution-order.md](./strict-execution-order.md)):

- **D1: Only wrap when needed** — modules with stable ordering across all entries don't need wrappers
- **D2: Transitive reduction of init calls** — if `a → b → c`, the `init_c()` in `a` is redundant

## Open questions

- What heuristics should drive automatic consolidation? Size-based? Request-count-based? Hybrid?
- How should the algorithm handle dynamic imports that are on the critical path (preloaded) vs lazy (loaded on interaction)?
- What's the interaction with `modulePreload` / preload directives? Chunk boundaries affect what gets preloaded.
- Should there be a "chunk affinity" concept — hints that certain modules should prefer certain chunks even in automatic mode?

## Related

- [app-scenario.md](./app-scenario.md) — parent design doc
- [strict-execution-order.md](./strict-execution-order.md) — wrapping mechanism and init call optimization
- [manual-code-splitting](../../docs/in-depth/manual-code-splitting.md)
- [automatic-code-splitting](../../docs/in-depth/automatic-code-splitting.md)
