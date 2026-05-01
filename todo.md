# Already-Loaded Atom Propagation — Default-On Regressions

After flipping `experimental.alreadyLoadedAtomPropagation` default to `true`,
3 fixtures fail in the rolldown integration suite (1697 pass, 3 fail).

Root pattern: the optimization is mathematically correct (it strips
dynamic-entry bits from modules guaranteed already-in-memory) but is unaware
of rolldown-specific chunking constraints that the post-grouping
`chunk_optimizer.rs` and `rehome_runtime_module` pipeline currently enforce.
Pre-grouping bit mutation moves modules across chunks in ways those
downstream passes don't expect.

## Failing fixtures

### 1. `crates/rolldown/tests/rolldown/issues/9225/_config.json`

**Symptom**

```
AssertionError: Output chunks must not have circular static imports
graph: {"api.js":["main.js"],"lazy.js":["api.js"],"main.js":["api.js"], ...}
cycle: api.js → main.js → api.js
```

**Why it fires**

Manual code splitting extracts `api` into its own chunk. Static-import chain
is `main → api → env → dep → side`. Without the optimization, `env`/`dep`/
`side` carry multiple dynamic-entry bits and land in a common chunk separate
from main, so the chain becomes `main → api → common-chunk` (acyclic).

With the optimization, every dynamic entry's only importer is `main`, which
statically reaches `env`/`dep`/`side`, so all dynamic bits get stripped.
`env` lands in main's chunk. Now `main → api` (static) and
`api → main` (because main holds `env` that api imports) closes the cycle.

**What's needed**

Make the strip step aware that `apply_manual_code_splitting` will pull
modules into manual chunks. Either:
- Run a cycle-detection-and-revert pass after stripping, OR
- Don't strip a bit when stripping would route a static-dep edge through a
  chunk that is itself statically downstream of the move target.

This is genuinely unsound at runtime — ESM evaluation hits TDZ on the
backedge. Fixing it likely requires reasoning about the eventual chunk
graph, not just the bitsets in isolation.

### 2. `crates/rolldown/tests/rolldown/issues/8920_2/_config.json`

**Symptom**

```
AssertionError: entry-2.js should host the runtime module
```

The fixture asserts the runtime helpers (`__exportAll`) live in `entry-2.js`
(the dominator of the runtime's consumer set). With the optimization, the
runtime ends up in a separate `chunk.js`.

**Why it fires**

The optimization moves `node2` out of a `{entry-2, node1-dyn}` common chunk
into `entry-2`'s entry chunk. That changes the runtime-consumer chunk set
seen by `rehome_runtime_module`'s dominator search; the new set has no
dominator, so a fresh `rolldown-runtime.js`-style chunk gets emitted.

I already special-cased the runtime module itself (excluded from the strip).
That is necessary but not sufficient — the issue here is that *consumers* of
runtime helpers (like `node2`) get repositioned, which still perturbs the
dominator search.

**What's needed**

Either: run the strip before bucketing as today but re-evaluate runtime
placement against the post-strip topology (the existing `rehome_runtime_module`
runs after bucketing — verify what it sees), or: exclude any module that
has a non-empty `depended_runtime_helper` from the strip. The latter is
heavy-handed (most modules touch helpers).

### 3. `crates/rolldown/tests/rolldown/issues/9049/_config.json` — `extended-preserve-entry-signatures-strict` variant

**Symptom**

```
AssertionError: Expected 4 chunks but got 3: main.js, route0.js, route1.js
```

`svc0`/`svc1` are statically imported by main and dynamically reachable from
both `route0` and `route1`. Under `preserveEntrySignatures: 'strict'`, the
shared service must stay in its own chunk to avoid altering main's exports.

**Why it fires**

The optimization strips `svc0`/`svc1`'s `route0`/`route1` bits (both routes'
only importer is main, which statically reaches the services). That leaves
the services with bits = `{main}` only. They then land directly in main's
entry chunk during bucketing — bypassing the strict-signature gate inside
`try_insert_into_existing_chunk` (which only fires when a *common* chunk is
about to be merged into a strict entry).

**What's needed**

Make the strip respect `preserveEntrySignatures: 'strict'`: do not strip the
last non-entry bit from a module if the resulting bitset would route the
module into a strict-signature entry chunk. Equivalent rule: `apply_strip`
must check the target entry's `preserve_entry_signature` setting and bail.

The default (non-strict) variant of 9049 actually *passes* with the
optimization — 3 chunks, which matches the non-strict expected count.

## Suggested next steps

1. Add a cycle-detection-and-revert step inside `apply_strip`, or run the
   strip through the same `would_create_circular_dependency` BFS that
   `chunk_optimizer.rs` uses post-grouping (re-targeted to the projected
   post-bucket chunk graph).
2. Plumb `preserveEntrySignatures` info into the strip step and skip strips
   that would land a module into a strict-signature entry.
3. Re-evaluate runtime-helper module placement after the strip changes the
   consumer set.

Until those land, the flag should default off (or these three fixtures must
be deleted/updated, which is not advised — 9225's cycle is a real ESM
evaluation hazard, not a test-too-strict situation).
