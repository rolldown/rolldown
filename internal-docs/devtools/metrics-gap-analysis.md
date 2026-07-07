# Build-metrics gaps for performance optimization

> Living document. Findings from running `devtools: { mode: 'metrics' }` (see
> [rolldown_devtools_metrics](../../crates/rolldown_devtools_metrics)) across the
> `top1000` corpus and asking, per project: **to actually improve chunking / bundle
> size / build performance, what does the report still not tell me?**

## Method

- ~666 real projects (`/Users/xiangjun/Documents/top1000/<owner>/<repo>`), processed
  **one at a time** (sequential, isolated child per project; resumable).
- Each: best-effort `npm install` (prod deps only) → bundle the largest `src/index`
  entry with the **local** rolldown + metrics mode. When install succeeds, deps are
  bundled (real **package graph**, `mode: deps`); otherwise bare imports are
  externalized (`mode: source`) so the module/chunk/timing graphs still populate.
- The per-project reports live in `/tmp/metrics-runs/<owner>__<repo>/`.

## What the report emits today (baseline)

`entry.md` (summary) + `timing.md`, `chunks.md`, `modules.md`, `packages.md`,
`graph.md`, `delta.md`. Concretely: build time (approx), output/JS/CSS/asset bytes,
module/chunk/asset/package **counts**, per-**package** rendered size (direct vs
transitive), chunk **reasons** + composition + import edges, import-**kind** histogram,
**most-imported** modules (fan-in ≥2), per-(plugin, hook) **call counts + approx time**,
and build-over-build **delta**.

That answers _"what did the build produce and roughly where did plugin time go"_. It does
**not** answer most _"what should I change to make it faster / smaller"_ questions. Those
gaps, split by whether they're **derivable now** from the existing devtools event stream
(pure `rolldown_devtools_metrics` adapter work) or need **new core instrumentation**:

## Priority summary

| #   | Missing metric                                                              | Goal          | Source                         |
| --- | --------------------------------------------------------------------------- | ------------- | ------------------------------ |
| 1   | Per-module **rendered byte size**                                           | chunking/size | core (cheap — data exists)     |
| 2   | **Cross-chunk module duplication**                                          | chunking/size | ✅ **implemented** (adapter)   |
| 3   | **Reachable-from-N-entries** (true shared-chunk signal)                     | chunking      | ✅ **implemented** (adapter)   |
| 4   | **Initial-load / critical-path bytes** per entry                            | chunking      | ✅ **implemented** (adapter)   |
| 5   | **Transfer size** (gzip/brotli), not just raw                               | size          | core (or adapter over content) |
| 6   | **Duplicate package versions**                                              | size          | ✅ **implemented** (adapter)   |
| 7   | **Tree-shaking effectiveness** (unused/eliminated)                          | size          | core                           |
| 8   | **Per-stage build time** (scan/resolve/load/transform/link/generate/minify) | build perf    | core                           |
| 9   | **Core transform time per module** (oxc TS/JSX)                             | build perf    | core                           |
| 10  | **Resolution cost / cache hit-rate**                                        | build perf    | core (counts derivable)        |
| 11  | **Wall-clock vs summed** (parallelism utilization)                          | build perf    | core                           |
| 12  | **Peak memory / RSS**                                                       | build perf    | core                           |
| 13  | **Incremental / HMR rebuild time**                                          | dev perf      | core (dev engine)              |
| 14  | **Budgets / baselines** + prescriptive suggestions                          | both          | adapter                        |

**Implemented (2026-07-02):** #2, #3, #4, #6 — the adapter-derivable set — now render in
`chunks.md` (duplication), `modules.md` (shared-across-entries), `graph.md` (initial-load per
entry) and `packages.md` (duplicate versions). Validated on `maizzle/framework`: #6 surfaced
real bloat (`boolbase`, `confbox`, `css-select`, `dom-serializer` each shipped at two versions)
and #3 flagged the modules reachable from the most entry points. Note: #2 usually reports
"none" — rolldown de-duplicates shared modules into common chunks by design, so it acts as a
confirmation/guard rather than a frequent finding.

---

## Chunking & bundle-size gaps

### 1. Per-module rendered byte size — _the single most-requested missing number_

Today sizes stop at chunk and package granularity. To decide a split you need **which
modules are big**. `ModuleGraphReady.modules[]` carries no size; only
`PackageGraphReady.size` aggregates per package. Yet rolldown core already computes it
(`RenderedModule::rendered_length()`), so this is cheap: add a `size` field to the
per-module entries of `ChunkGraphReady` (or a new `ModuleSizes` action). Unlocks a
"largest modules" table that actually drives size work.

### 2. Cross-chunk module duplication — **derivable now**

A module bundled into >1 chunk is duplicated weight. `ChunkGraphReady.chunks[].modules[]`
already lists module ids per chunk, so the adapter can flag `module → [chunkA, chunkB]`
and sum the wasted bytes (with #1). This is a top real-world win and needs no core change —
`rolldown_devtools_metrics` just doesn't compute it yet.

### 3. Reachable-from-N-entries — the real shared-chunk signal — **derivable now**

`modules.md`'s "most-imported" is _import fan-in_, which (see phosphor-icons: `IconBase`
imported 1513×) mostly reflects barrels, not split opportunities. The actionable signal is
**"module M is reachable from entries {A,B}"** → candidate for a shared chunk. Derivable
from `ModuleGraphReady` (import graph) + entry chunks in `ChunkGraphReady`. The
bundle-analyzer builtin already does reachability; the metrics adapter should too.

### 4. Initial-load / critical-path bytes per entry — **derivable now**

`graph.md` lists an entry's imports but not the **total bytes an entry pulls on first
load** (entry chunk + transitive static imports). That number is _the_ code-splitting KPI.
Derivable from `ChunkGraphReady` static-import edges + chunk sizes (`AssetsReady`).

### 5. Transfer size (gzip/brotli) — raw ≠ what ships

Perf is about bytes over the wire. All sizes today are raw. `AssetsReady.content` is
present, so an adapter _could_ gzip it, but that means re-touching the large payloads we
deliberately avoid retaining — better emitted from core (cheap: compress at asset-emit).

### 6. Duplicate package versions — **derivable now**

Two copies of the same package at different versions (classic bloat) are visible in
`PackageGraphReady` (same `name`, different `version`) but not flagged. Easy adapter win.

### 7. Tree-shaking effectiveness

No signal on unused/eliminated exports or `sideEffects` misses. "This module contributes
40 kB but only 2 of its 30 exports are used" is a prime size lever and is entirely absent.
Needs core instrumentation (retained-vs-authored exports / eliminated bytes).

---

## Build-performance gaps

### 8. Per-stage build time — _where the seconds actually go_

Today: total (approx) + per-plugin hook time. Missing the **phase breakdown**
(scan / resolve / load / transform / link / generate / minify). Core tracks some of this
internally (e.g. link-stage micros in `HookTimingCollector`) but doesn't emit stage
boundaries as devtools actions. This is the first question when a build is slow.

### 9. Core transform time per module (oxc TS/JSX) — **biggest blind spot**

`timing.md`'s "transform hotspots" is empty for normal builds because TS/JSX transform is
**core**, not a plugin hook — so the dominant per-module CPU cost is invisible. On
phosphor-icons (4544 `.tsx` modules, 6 s build) the report attributes time only to
resolveId/load builtins; the actual transform cost is unmeasured. Needs core to emit
per-module parse+transform timing.

### 10. Resolution cost / cache hit-rate

`resolveId` dominates call volume (phosphor: 9080 calls, ~680 ms per builtin). We surface
counts+approx time, but not **cache hit/miss** or slowest specifiers — resolution is a
frequent build-perf bottleneck. Counts are derivable now; hit-rate needs core.

### 11. Wall-clock vs summed work (parallelism)

Plugin/hook times are summed across concurrent invocations and can exceed wall-clock
(noted in `timing.md`). There's no measure of **parallelism utilization** (summed work ÷
wall-clock) to show whether the build is CPU-bound, serialized, or I/O-bound.

### 12. Peak memory / RSS

Build perf includes memory pressure; large graphs OOM. No memory metric is captured.

### 13. Incremental / HMR rebuild time

The metric is one-shot full builds. Dev-loop perf (cold start, HMR update latency) — often
what users feel most — is absent; devtools isn't wired to the dev/HMR engine yet.

---

## Cross-cutting

### 14. Budgets, baselines, and prescriptive output

Every number is descriptive with no context: _is 140 kB big? is 6 s slow?_ No budgets, no
per-chunk size thresholds, no historical baseline beyond the immediate `delta.md`. And the
report stops at _description_ — it never says _"modules X,Y are in 2 entries → extract a
shared chunk"_ or _"package Z ships 2 versions."_ A prescriptive layer (adapter) turns the
data into actions.

---

## Feasibility grouping (what to build first)

- **Adapter-only (no core change):** ✅ #2 cross-chunk duplication, #3 reachable-from-N-entries,
  #4 critical-path bytes, #6 duplicate package versions — **done** in `rolldown_devtools_metrics`.
  Remaining: #14 budgets/recommendations.
- **Cheap core instrumentation (data already exists, just not emitted):**
  #1 per-module size, #10 resolve counts (already), part of #8 (link stage exists).
- **New core instrumentation:** #5 transfer size, #7 tree-shaking, #8 full stage timing,
  #9 core-transform timing, #11 parallelism, #12 memory, #13 HMR.

## Corpus evidence (666 projects)

**Coverage:** 666 processed → **495 ok** (74%), 72 no-entry (no bundleable `src/index`),
99 bundle-fail (decorators / unsupported syntax). Of the 495: **172 `deps` mode** (install
succeeded → real package graph), 323 `source` mode.

**The dominant finding — most builds are single-chunk libraries:** only **37 / 495 (7.5%)**
produced more than one chunk (chunk count p50 = 1, p90 = 1, max = 193). Module count is
small-tailed-huge: p50 = **16**, p90 = **124**, max = **4544**. Package count (deps mode):
p50 = 4, p90 = 16, max = 164.

**What this means for priorities:**

- **Size + build-perf metrics apply to ~every project** and should be table stakes:
  per-module size (#1), transfer size (#5), tree-shaking (#7), and especially \*\*per-stage
  - core-transform build time (#8, #9)**. The biggest graphs — phosphor-icons (4544 mod,
    **1 chunk\**), causalens (1985, 1 chunk), chakra-ui/ark (993, 1 chunk) — don't split at
    all, so their only levers are *module size* and *transform time\*, both currently invisible.
- **Chunking metrics (#2 duplication, #3 reachable-from-N-entries, #4 critical-path) are
  high-value but for the multi-chunk minority** (apps/frameworks). They're worth building
  (all derivable now) but shouldn't block the broadly-applicable size/perf metrics.

**Concrete multi-chunk case — `maizzle/framework`** (929 modules, **47 chunks**, 164
packages, 14.5 MB, deps mode) shows every chunking gap at once:

- A `cloneConfig-*.js` **common chunk of 463 modules / 3.2 MB** — is it duplicated across
  entries? which entries reach it? _(#2, #3 — unanswerable today)._ And which of those 463
  modules dominate the 3.2 MB? _(#1 — unanswerable today)._
- ~30 single-module entry chunks that are just big deps lazily split (typescript 1.0 MB,
  prettier 578 kB, babel 375 kB…) — initial-load cost per entry _(#4)_ isn't computed.
- `packages.md` does work well here: it correctly ranks oxfmt (8.0 MB), vite (1.7 MB),
  undici (901 kB) as the size targets — validating the package-graph format at scale.

**Takeaway:** the report is a solid _inventory_ but a weak _optimization guide_. The fastest
wins are the adapter-derivable chunking metrics (#2–4, #6) plus the two cheap-core numbers
that apply to nearly all 495 projects — **per-module size (#1)** and **core-transform time
per module (#9)**.
