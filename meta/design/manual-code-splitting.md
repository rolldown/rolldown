# Manual Code Splitting

## Summary

Manual code splitting lets users define chunk boundaries via `manualCodeSplitting.groups`. Each group has a `name`, a `test` pattern to match modules, and optional size/priority controls. Matched modules (and optionally their dependencies) are pulled into dedicated chunks instead of being split by the automatic algorithm.

## Important features

### `entriesAware`

When `entriesAware: true`, a group's modules are further split by **which entry points can reach them**. This produces per-entry-set chunks instead of one monolithic group chunk.

#### How it works

Each module has a **bitset** representing which entries can reach it. After collecting all modules into the group, we split them into subgroups by their bitset pattern:

```
Given: 3 entries (A, B, C) and a group matching shared-*.js

Module reachability:
  shared-abc.js  →  bits = {A, B, C}   (all entries)
  shared-ab.js   →  bits = {A, B}      (entries A and B)
  shared-a.js    →  bits = {A}         (entry A only)

Step 1: Build one flat group with all matching modules

  ┌─────────────────────────────────────────┐
  │ vendor group (flat)                     │
  │  shared-abc.js  shared-ab.js  shared-a  │
  └─────────────────────────────────────────┘

Step 2: Split by bitset → subgroups

  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
  │ bits = {A, B, C} │  │ bits = {A, B}    │  │ bits = {A}       │
  │ shared-abc.js    │  │ shared-ab.js     │  │ shared-a.js      │
  └──────────────────┘  └──────────────────┘  └──────────────────┘

Step 3: Each subgroup → chunk

  vendor~entry-a~entry-b~entry-c.js   (loaded by all)
  vendor~entry-a~entry-b.js           (loaded by A and B only)
  vendor~entry-a.js                   (loaded by A only)
```

Entry A loads all three vendor chunks. Entry B loads the first two. Entry C loads only the first. Each entry only downloads what it actually needs.

#### Why flat-then-split matters

The split must happen **after** collecting all modules into a flat group. If subgroups are created during the build phase (per module's own bits), `includeDependenciesRecursively` adds shared dependencies to each subgroup independently:

```
BAD: subgroups during build (dependencies duplicated)

  lib-a.js (bits={A}) matches → subgroup {A}
    └─ deps: shared-dep.js added to subgroup {A}

  lib-b.js (bits={B}) matches → subgroup {B}
    └─ deps: shared-dep.js added to subgroup {B}  ← DUPLICATE

  subgroup {A} = [lib-a, shared-dep]  size: 150
  subgroup {B} = [lib-b, shared-dep]  size: 150
                          ^^^^^^^^^^
                    counted twice → inflated sizes

GOOD: flat group first, split after

  flat group = [lib-a, lib-b, shared-dep]   (each module once)

  split by bits:
    {A}     → [lib-a]       size: 30
    {B}     → [lib-b]       size: 30
    {A, B}  → [shared-dep]  size: 100   ← counted once, correct size
```

Inflated sizes break `entriesAwareMergeThreshold` — subgroups that should merge (because they're actually small) appear too large.

#### `entriesAwareMergeThreshold`

Splitting by bitset can produce many tiny subgroups. `entriesAwareMergeThreshold` merges subgroups below a size threshold into their nearest neighbor (by bitset similarity):

```
Before merge (threshold = 50):
  {A}     → [lib-a]       size: 30  ← below threshold
  {B}     → [lib-b]       size: 30  ← below threshold
  {A, B}  → [shared-dep]  size: 100

Merge: lib-a (30 < 50) merges into {A, B} (smallest symmetric difference)
Merge: lib-b (30 < 50) merges into {A, B}

After merge:
  {A, B}  → [lib-a, lib-b, shared-dep]  size: 160
```

The merge algorithm uses a min-heap to process smallest subgroups first. For each candidate, it finds the best target by minimizing **symmetric difference** of bitsets (prefer similar entry-point patterns), with size as tiebreaker.

## Related

- [app-scenario-chunking.md](./app-scenario-chunking.md) — chunking in app scenario context
- [code-splitting.md](./code-splitting.md) — automatic code splitting
