# Parallelism opportunities (evidence-ranked)

Where the shared-runtime build (`--no-default-features --features
async-runtime`) actually spends its wall time, and what — if anything — is
worth parallelizing next. Every number in this document traces to one of
three sources:

1. **Task-16 profiles** — 5 symbolicated `samply --rate 2000` runs of
   `apps/10000` and 3 of `apps/three10x` at `e5786dcd7`, analyzed
   programmatically (`analyze-profile.mjs`); the tables below reproduce the
   median run (`apps/10000` run4, wall 422.5 ms; `apps/three10x` run2, wall
   330.9 ms). Artifacts: `/tmp/parallelism-profiles/` (not committed);
   methodology and cross-checks: `.superpowers/sdd/task-16-report.md`.
2. **Sub-attribution over those same two profiles** (`subattr.mjs`, kept
   with the artifacts): every CPU sample inside a Task-16 phase window,
   classified leaf-first by stack regex into finer buckets (minify
   sub-stages, sourcemap, syscall callers, link sub-passes).
3. **The A/B measured for this document** (fs-read-pool experiment, below).

Aggregation rule, stated once and used throughout: shares are
`CPU-ms of the bucket ÷ profiled wall of the same median run`; CPU-ms is
read as ≈ wall-ms **only** inside the serial tail, where the scheduler
reports exactly one active runnable (occupancy 1/18) and one worker thread
executes the whole phase (Task-16, "single-thread tail"). No numbers are
mixed across runs or fixtures.

- **Date**: 2026-07-02
- **Host**: Apple M5 Max, 18 logical cores, macOS 26.5.2, Node v24.12.0
  (same host as [benchmarks.md](./benchmarks.md))
- **Commit**: `e5786dcd7`, shared-runtime binding, `MultiThread`,
  workerThreads=18, maxBlockingTasks=18

## The shape of the problem (Task-16 utilization)

apps/10000 (median profiled run, wall 422.5 ms):

| phase | wall ms | busy cores (phase work) | scheduler occupancy | dominant cost |
| --- | ---: | ---: | ---: | --- |
| scan | 220 | 15.5 | 0.93 | `__open` 2308 ms kernel CPU (68% of phase CPU) |
| link | 47.5 | 0.92 | 0.056 (= 1/18) | tree-shaking include, bind imports |
| generate | 147.5 | 1.29 | 0.056 | minify pipeline + sourcemap on one thread |
| write | 10 | 0.23 | 0.056 | 2 files, 19.2 MB |

apps/three10x (median profiled run, wall 330.9 ms):

| phase | wall ms | busy cores (phase work) | occupancy | dominant cost |
| --- | ---: | ---: | ---: | --- |
| scan | 50 | 12.0 | 0.87 | `__open` 282 ms kernel CPU |
| link | 15 | 0.77 | 0.056 | — |
| generate | 262.5 | 1.18 | 0.056 | single-chunk minify of a 6 MB bundle |
| write | 7.5 | 0.36 | 0.056 | 25.2 MB |

Two facts dominate everything below:

- **The serial tail is real**: exactly one active runnable for the last
  ~202 ms of apps/10000 (53% of wall) and ~87% of apps/three10x. The tail
  is link → render → single-chunk minify (+re-parse) → codegen → sourcemap
  → write, all on one `rolldown-runtime-*` worker.
- **Scan's 15.5 busy cores are mostly kernel**: 10.5 of them are `__open`
  CPU alone (2308 ms / 220 ms window), and resolver `lstat` adds ~1 more
  (210 ms) — ≈11.4 combined, not parse/resolve. Adding scan-side CPU
  parallelism buys nothing; the lever, if any, is syscall-side.

## Sub-attribution (what the tail and the scan actually contain)

Generate-window CPU by bucket (leaf-first stack classification,
`subattr.mjs`; window CPU: 249.3 ms / 367.4 ms):

| bucket | apps/10000 (ms) | apps/three10x (ms) |
| --- | ---: | ---: |
| minify: compress | 22.9 | 82.5 |
| minify: re-parse of rendered chunk | 14.5 | 37.3 |
| minify: semantic rebuild | 12.9 | 52.9 |
| minify: codegen | 11.5 | 21.9 |
| minify: sourcemap | 10.7 | 25.7 |
| minify: unclassified | 0.5 | — |
| **minify total** | **73.0 (17.3% of wall)** | **220.3 (66.6% of wall)** |
| sourcemap, non-minify (collapse, map→JSON, `add_source_mapping`) | 25.1 | 37.8 |
| finalize-modules (already parallel — the ~5-core blip) | 28.6 | 7.1 |
| deconflict/renamer | 18.8 | 6.8 |
| render codegen + concat | 29.8 | 35.7 |
| chunk graph | 11.1 | 1.9 |
| idle-worker wake churn + unclassified | 63.1 | 57.8 |

Scan-window syscall CPU by caller (apps/10000: 2597 ms syscall-leaf CPU of
3612 ms scan-window CPU = 71.9%):

| caller | syscall CPU |
| --- | ---: |
| module-source reads — `load_source` → `spawn_blocking(fs.read_to_string)` (`crates/rolldown/src/utils/load_source.rs:67,120`) | 2384 ms (`__open` 2305, `read` 54, `close` 25) |
| resolver metadata — oxc_resolver `Cache::followed_metadata` | 210 ms (`lstat`) |
| resolver `package.json` reads — `Cache::find_package_json` | 3 ms |

(`rolldown_fs::OsFileSystem` wraps `oxc_resolver::FileSystemOs` —
`crates/rolldown_fs/src/os.rs:13` — which is why module reads and resolver
reads share one profile frame; the stacks above the frame separate them.)

Per-open arithmetic: apps/10000 bundles ≈10,003 source files → ≈230 µs of
kernel CPU per `open(2)` (apps/three10x: 282 ms / ≈6,092 files ≈ 46 µs).
For contrast, a single-threaded warm-cache probe over the fixture's
10,001 source files measures 9–24 µs wall per open+close pair (median run
19.6 µs, including the node syscall-wrapper overhead; supplementary probe
run under the ambient conditions disclosed in the A/B section —
`/tmp/parallelism-profiles/probe-open{-latency.txt,.mjs}`). An
order-of-magnitude gap: this is kernel-side contention (macOS vnode/namei)
from up to 16 concurrent opens, not IO volume. It is not a profiler
artifact: unprofiled `/usr/bin/time` sys-time on apps/10000 is 2.19–2.27 s
for the shared binding vs 1.24 s under tokio's 4-thread dedicated blocking
pool ([benchmarks.md](./benchmarks.md)).

## Ranked candidates

Ranking = measured share × feasibility, judged before the prototype ran.
Every candidate was re-verified in code at the cited location; the plan's
prior guesses are corrected where the profile disagrees.

### 1. Scan-phase FS `open(2)` contention — prototyped, REJECTED by A/B

- **Where**: `crates/rolldown/src/utils/load_source.rs:67,120` (module
  source reads via `spawn_blocking` onto the shared pool's blocking lane).
- **Measured share**: 2305 ms `__open` kernel CPU inside the 220 ms scan
  window = 10.5 of scan's 15.5 busy cores; scan is 52% of profiled wall.
- **Expected win (ceiling)**: scan-window non-syscall CPU is ≈1015 ms; at
  ~15 workers that is ≈68 ms plus resolver dependency chains, so an
  uncontended scan could plausibly land near 100–120 ms instead of 220 ms —
  order 60–120 ms, **14–28% of wall** (60/422.5 = 14.2%, 120/422.5 =
  28.4%). A ceiling, not a promise.
- **The two prior facts to reconcile**: PR #6270 fixed this symptom under
  tokio by capping a **dedicated** blocking pool at 4 threads (sys-time
  −63%); Task 15's cap-4 A/B under the shared runtime **lost 20%** — but it
  capped the blocking **lane** of an 18-thread pool, which starves into
  park/unpark churn instead of reducing the number of OS threads inside
  `open(2)`. Syscall-count reduction has no room (one open per module is
  already minimal; resolver reads are already cached — 3 ms). The untested
  mechanism was #6270's own: a small **dedicated** pool for source reads
  beside the shared pool.
- **Risk**: none to correctness (execution-venue change for reads only;
  flag-on output verified byte-identical); the perf risk is the Task-15
  failure mode — and that is what the A/B found (below).
- **Verdict**: contention is real and reducible (sys-time −24% measured),
  but the naive dedicated pool **loses 9.1% wall** on apps/10000. The wake
  path is the blocker, not the idea — see the A/B section.

### 2. Intra-chunk minify parallelism — top remaining opportunity (upstream)

- **Where**: `crates/rolldown/src/stages/generate_stage/minify_chunks.rs:25`
  — `chunks.par_iter_mut()` is chunk-granular; both fixtures emit ONE
  chunk, so the par_iter is width-1 and the whole minify pipeline runs on
  the tail thread. `EcmaCompiler::dce_or_minify`
  (`crates/rolldown_ecmascript/src/ecma_compiler.rs:111-140`) re-parses the
  rendered chunk string, rebuilds semantic data, compresses, and
  re-codegens with a fresh sourcemap.
- **Measured share**: 73.0 ms = **17.3%** of apps/10000 wall; 220.3 ms =
  **66.6%** of apps/three10x wall (the plan's "~70 ms" guess is confirmed
  at 73.0). The single largest measured serial block.
- **Expected win**: parallelizing compress+semantic (35.8 ms / 135.4 ms)
  across ~6 P-cores would save roughly 25–30 ms (≈7%) on apps/10000 and
  ~110 ms (≈33%) on apps/three10x.
- **Feasibility**: not implementable in rolldown. The chunk is a
  `string_wizard` concatenation — no combined AST exists to partition, and
  a per-module split is semantically invalid after scope hoisting
  (cross-module peephole, renaming). Function-level parallel compression,
  parallel semantic building, and parallel sourcemap emission are
  oxc-internal work (`oxc_minifier`, `oxc_semantic`, `oxc_codegen`);
  upstream oxc perf PRs are the vehicle.
- **Verdict**: top opportunity by measured share; fails the prototype
  rule's <150-LOC/in-repo half.

### 3. Sourcemap serial path — same upstream story

- **Where**: `oxc_codegen::sourcemap_builder::add_source_mapping` (render
  and minify codegen), oxc_sourcemap `lookup_token`/`generate_lookup_table`
  (inside `collapse_sourcemaps`), map→JSON serialization
  (`json_escape_simd`; the 13.7 MB `.map` with embedded sourcesContent on
  apps/10000); rolldown-side only `sugar_path` normalization (2.8 ms).
- **Measured share**: 25.1 + 10.7 = 35.8 ms = **8.5%** of apps/10000 wall;
  37.8 + 25.7 = 63.5 ms = **19.2%** of apps/three10x.
- **Feasibility**: the meat is oxc-side (largely the same bytes as
  candidate 2 — sourcemap cost is embedded in codegen). The only <150-LOC
  rolldown-side slice is worth ~2.8 ms (0.7%).
- **Verdict**: fold into the oxc upstream conversation; no in-repo
  prototype.

### 4. Parallel include/tree-shaking marking — REJECTED

- **Where**: `crates/rolldown/src/stages/link_stage/tree_shaking/include_statements.rs:213`
  — fixpoint loop over recursive `include_symbol`/`include_statement`/
  `include_module` marking with shared `&mut IncludeContext` bitsets and
  order-sensitive CJS-bailout set accumulation.
- **Measured share**: 15.1 ms tree-shaking-include CPU = **3.6%** of
  apps/10000 wall (three10x: 4.0 ms = 1.2%). The plan's ~38 ms guess was
  2.5× too high — most of the link window's 130 ms CPU is concurrent
  teardown of scan data (86 ms non-link, cf. Task-16 anomaly 4), not
  linking.
- **Rejected because**: below the 5% bar, and parallelizing it is exactly
  the "linker's correctness-critical DFS ordering" the decision rule
  excludes.

### 5. Deconflict/renamer parallelism — REJECTED

- **Where**: `crates/rolldown/src/utils/renamer.rs`
  (`add_symbol_in_root_scope` / `ConflictResolver`), driven per chunk by
  `deconflict_chunk_symbols` during generate.
- **Measured share**: 18.8 ms = **4.4%** of apps/10000 wall (three10x:
  6.8 ms = 2.1%). The plan's ~19 ms confirmed — but below the bar.
- **Rejected because**: below the 5% bar; name assignment is sequential by
  construction — each resolution depends on the taken-name set built by all
  previous ones, so any reordering changes emitted names (snapshot churn,
  cross-run nondeterminism).

### 6. Scan-frontier / dispatcher latency — REJECTED

- **Where**: `crates/rolldown/src/module_loader/module_loader.rs:398-684`
  (serial `rx.recv()` dispatcher; children spawn only after the parent's
  result message is dequeued); FIFO runnable queue with no LIFO slot
  (`crates/rolldown_utils/src/async_runtime.rs:492,559`).
- **Measured**: scan occupancy 0.93 (mean 16.7/18 active, 5 ms sampler) at
  15.5 busy cores — the pool is not starved waiting on the dispatcher.
  Upper bound even if ALL idle occupancy were dispatch latency:
  (1 − 0.93) × 220 ms ≈ 15 ms (3.6%), and much of that is IO wait.
- **Rejected because**: no starvation signal; below the bar. (But see the
  A/B conclusion — the wake path around this same queue is implicated as
  the gating cost for any IO-shaping win.)

### 7. Overlap `write()` with generate — REJECTED (was the plan's first pick)

- **Where**: `crates/rolldown/src/bundle/bundle.rs:199-211` — sequential
  `fs.write` per asset after generation completes.
- **Measured share**: write is 10 ms wall / 2.3 ms CPU = **2.4%** of
  apps/10000 wall (three10x: 7.5 ms = 2.3%).
- **Rejected because**: below the 5% bar. The guess that this was the
  cheapest win predates the profiles; at these sizes the OS absorbs 19 MB
  of buffered writes in ~10 ms. Streaming per-chunk writes only becomes
  interesting for many-chunk builds, which these fixtures do not exercise.

### 8. Scheduler micro-costs — REJECTED as ranked (but see A/B conclusion)

- **Where**: `crates/rolldown_utils/src/async_runtime.rs:490-497`
  (per-schedule lock + notify), `:196-208` (metric RMWs), `:769-776`
  (`CoopSignal::notify` → `notify_all`).
- **Precondition**: "only if Task 15 flagged a regression" — it did not;
  the shared runtime beat tokio on all four fixtures
  ([benchmarks.md](./benchmarks.md)).
- **Measured**: metric RMWs are `Relaxed` atomics, ~4 per poll × 40k polls
  — sub-ms. The visible cost is idle-worker wake churn: ~299 ms CPU across
  the build (Task 16), ~41 ms (10000) / ~50 ms (three10x) of it inside the
  generate window — but on cores the serial tail is not using, so
  wall-neutral on an idle machine.
- **Rejected as a standalone wall lever** — however, the fs-read-pool A/B
  below implicates this wake path as the reason IO-shaping experiments
  fail, which promotes it from "hygiene" to "prerequisite".

## Prototype decision rule, applied

Rule (from the plan): prototype ONLY if the top candidate is ≥5% of
apps/10000 wall AND implementable in <~150 LOC without touching the
linker's correctness-critical DFS ordering. Applications, in rank order:

1. **Candidate 1 (FS contention)**: estimated share 14–28% ≥ 5% ✓ (even a
   quarter of the ceiling clears the bar); implementation is a dedicated
   read pool behind an env flag — 107 LOC touching only `load_source`'s
   read venue, linker untouched ✓ → **rule fires; prototype built and
   measured** (below).
2. **Candidate 2 (intra-chunk minify)**: 17.3% ≥ 5% ✓; needs oxc-internal
   parallelism, not <150 LOC in this repo ✗ → no prototype. (Had it ranked
   first, the cascade lands on candidate 1 anyway.)
3. **Candidate 3 (sourcemap path)**: 8.5% ≥ 5% ✓; the in-repo slice is
   0.7% ✗, the meat is oxc-side ✗ → no prototype.

All remaining candidates measure below the 5% bar.

## A/B: `ROLLDOWN_EXPERIMENTAL_FS_READ_POOL` — the pool loses

Branch `experiment/fs-read-pool` (off `e5786dcd7`, local only, head
`04bfaf4db`): 107 LOC routing `load_source`'s three non-wasm read call
sites to a lazily-spawned dedicated OS-thread pool
(`crates/rolldown/src/utils/fs_read_pool.rs`; flag unset/`0` = off, `1` =
4 threads — the #6270 value, `N` = N threads). Flag-off is byte-identical
to baseline; flag-on output verified byte-identical
(`shasum` over `dist-rolldown/`). One binding binary
(`bench-fs-read-pool.node`, unstripped release, same profile as Task 16),
both sides selected by env var only.

**Design note / disclosure**: the measurement window was NOT clean — an
unrelated single-core test binary plus FileProvider/OneDrive/Spotlight sync
churn (~1–2 cores, load avg 10–23 on 18 cores) ran throughout. A blocked
hyperfine matrix (3 warmups + 12 runs per side) was recorded first but is
unreliable (flag-0 σ = 171 ms on apps/10000; its runs landed 515–1046 ms
vs the known-clean 361–430 ms band). The verdict therefore rests on a
**paired interleaved** design — 16 alternating off/on pairs per fixture,
first 2 pairs discarded as warmup, per-pair deltas — which cancels ambient
drift by construction. Raw data:
`/tmp/parallelism-profiles/ab-fs-read-pool/`.

| fixture (14 pairs) | off median | on (4 threads) median | paired delta median | pairs won by on |
| --- | ---: | ---: | ---: | ---: |
| apps/10000 | 434.1 ms | 478.3 ms | **+39.4 ms (+9.1% — loss)** | 2/14 |
| apps/three10x | 313.5 ms | 317.6 ms | +5.9 ms (+1.9% — loss) | 4/14 |

Per-process counters, apps/10000. user/sys are the hyperfine block means
(12 runs/side — robust); instructions and context switches come from
`/usr/bin/time -l` (medians of 3 samples/side; the noisiest block of the
session — its own 3-sample sys medians go the other way, 2.76 → 2.94 s,
which is why the 12-run means are quoted for time):

| metric | flag off | flag on (4 threads) |
| --- | ---: | ---: |
| sys time (12-run means) | 2.50 s | **1.90 s (−24%)** |
| user time (12-run means) | 1.11 s | 1.27 s (+15%) |
| instructions retired (median of 3) | 14.38e9 | 20.23e9 (**+41%**) |
| involuntary context switches (median of 3) | 19.8k | 201.8k (**10×**) |

A width probe (2 vs 8 reader threads, 6 runs each, blocked design —
indicative only) ordered 8 > 4 > 2, i.e. the closer the pool gets to the
old 16-wide behavior, the less it loses: there is no width sweet spot that
beats the baseline.

**Reading**: the mechanism half-works exactly as PR #6270 predicts — 4
concurrent openers instead of 16 cuts kernel sys-time by ~24% (the
contention is real and reducible). But under the shared runtime every read
completion is now a cross-thread wakeup: reader thread → oneshot →
`schedule()` → `CoopSignal::notify` (a `notify_all`,
`async_runtime.rs:769-776`) + drainer spawn — ~10,000 times per build. The
counter signature (10× involuntary context switches, +41% instructions,
+15% user time) matches wake amplification, and it costs
more than the reclaimed kernel time. Task 15's lane-cap and this dedicated
pool are two currencies of the same tax: **on this scheduler, funneling
10k reads through a narrow channel converts kernel contention into wake
churn.** Under tokio, #6270 pays a targeted-wake (`notify_one` + LIFO
slot) price instead, which is why it won there.

**Verdict: rejected as implemented.** The experiment branch stays in place
(unpushed) for re-measurement if the wake path gets cheaper.

## What to do with this

1. **The tail is an oxc conversation, not a rolldown one.** Minify
   (17.3% / 66.6% of wall) plus the sourcemap path (8.5% / 19.2%) are the
   measured serial blocks, and both live in `oxc_minifier` /
   `oxc_semantic` / `oxc_codegen` / oxc_sourcemap. Function-level parallel
   compression and parallel sourcemap emission upstream are the only moves
   that touch double-digit shares.
2. **Scan IO shaping is gated on a cheaper wake path.** The contention is
   confirmed (sys −24% with 4 openers) but uncapturable while each read
   completion costs a `notify_all` fan-out. Targeted wake / a LIFO slot in
   `MultiThreadExecutor` is the prerequisite experiment; re-run
   `experiment/fs-read-pool` after it.
3. **Everything else measured below the 5% bar**: include-marking 3.6%,
   renamer 4.4%, write overlap 2.4%, dispatcher ≤3.6%, metric RMWs sub-ms.
   The plan's prior first pick (write overlap) is dead on the numbers.
4. Dev/watch single-thread mode remains explicitly out of scope for this
   analysis (unchanged from the plan).

## Related

- [benchmarks.md](./benchmarks.md) — tokio-vs-shared A/B and the
  blocking-cap A/B this document builds on
- [implementation.md](./implementation.md) — scheduler structure
- [design.md](./design.md) — why one pool
