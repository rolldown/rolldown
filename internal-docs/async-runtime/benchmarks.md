# Benchmarks: tokio runtime vs shared async runtime

Committed, reproducible A/B results for the two native binding builds:

- **tokio** — default build (`pnpm --filter rolldown build-binding --release`)
- **shared** — `--no-default-features --features async-runtime`

Both bindings and the JS glue were built from commit `d6622e8f0`
(sizes 16,344,368 B tokio / 16,095,680 B shared). Harness, prerequisites and
methodology: [`scripts/misc/bench-async-runtime/README.md`](../../scripts/misc/bench-async-runtime/README.md)
(hyperfine 3 warmups + 12 runs per side; `/usr/bin/time -l` counters as
medians of 3 samples; peak threads via a 50 ms `ps -M` sampler).

- **Date**: 2026-07-02
- **Host**: Apple M5 Max, 18 physical / 18 logical cores, 128 GB RAM,
  macOS 26.5.2, Node v24.12.0, hyperfine 1.20.0
- **Fixtures**: [`rolldown-benchmark`](https://github.com/rolldown/rolldown-benchmark)
  `apps/1000`, `apps/5000`, `apps/10000`, `apps/three10x`

## Effective runtime defaults (what was measured)

| build  | worker threads                  | blocking limit                                                                                                                            | where                                                                                                                                                  |
| ------ | ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| tokio  | 27 (`physical * 3 / 2`)         | dedicated blocking pool, capped at **4** threads                                                                                          | `crates/rolldown_binding/src/lib.rs` `init` + `resolve_default_runtime_threads` (`crates/rolldown_binding/src/async_runtime.rs:200-209`)               |
| shared | 18 (`num_cpus::get_physical()`) | `max_blocking_tasks` defaults to **worker_threads (18)**; blocking jobs run _on_ the shared pool (no extra threads), clamped to pool size | `register_async_runtime` (`crates/rolldown_binding/src/async_runtime.rs:386-392`), clamp at `crates/rolldown_utils/src/async_runtime.rs:69` and `:485` |

Both limits honor `ROLLDOWN_MAX_BLOCKING_THREADS`; worker counts honor
`ROLLDOWN_WORKER_THREADS`. Verified end-to-end: with the env var set to 1/4/
unset, `getAsyncRuntimeMetrics().maxActiveBlockingTasks` peaked at exactly
1/4/18 during an `apps/1000` build (2,511 blocking tasks each run).

## Results

Wall time is hyperfine `mean ± σ` over 12 runs, with the median alongside
(robust against the ambient-noise bursts described under Anomalies). Counters
are medians of 3 `/usr/bin/time -l` samples. Voluntary context switches
reported ~0 for both sides on every fixture on this macOS version and are
omitted. Instructions and involuntary context switches are per whole `node`
process (single in-process build, JIT warmup included on both sides equally).

| fixture       | side   | wall mean ± σ (ms) | wall median (ms) | instructions retired | max RSS (MiB) | invol. ctx switches | peak threads |
| ------------- | ------ | -----------------: | ---------------: | -------------------: | ------------: | ------------------: | -----------: |
| apps/1000     | tokio  |        118.3 ± 1.8 |            119.0 |              2.639e9 |         264.7 |               4,877 |          38¹ |
|               | shared |    **115.7 ± 2.1** |            116.0 |              2.634e9 |         252.4 |               4,304 |       **25** |
| apps/5000     | tokio  |       283.1 ± 33.9 |            267.4 |              7.050e9 |         662.7 |              12,080 |           56 |
|               | shared |   **276.1 ± 34.1** |            259.2 |              7.062e9 |         642.8 |               8,966 |       **25** |
| apps/10000    | tokio  |       491.9 ± 25.4 |            484.7 |             13.425e9 |       1,220.9 |              22,317 |           56 |
|               | shared |   **469.6 ± 18.4** |            461.3 |             13.345e9 |       1,178.9 |              17,508 |       **25** |
| apps/three10x | tokio  |       388.9 ± 16.1 |            383.9 |              8.779e9 |         798.2 |               7,034 |           56 |
|               | shared |   **382.4 ± 13.0** |            380.7 |              8.771e9 |         780.7 |               6,312 |       **25** |

¹ tokio's peak is bimodal on `apps/1000` (38 or 56 across sessions): the extra
18 threads are the lazily-created **global** rayon pool, which the tokio build
spawns in addition to its 27 tokio workers + up-to-4 blocking threads. The
shared build never creates it — rayon work initiated from the executor's own
rayon-backed pool reuses that pool. Thread inventory at peak (from `sample`):
shared = 18 `rolldown-runtime-*` + 4 V8 workers + main + inspector + task
scheduler = 25.

Summary vs the acceptance bar:

- **Wall**: shared faster on every fixture — mean +2.2% / +2.5% / +4.5% /
  +1.7% (median +2.5% / +3.0% / +4.8% / +0.8%). No fixture regresses.
- **Threads**: 56 (38) → 25 stable; the Rolldown-owned share collapses
  27 tokio + 4 blocking + 18 global-rayon → one 18-thread pool.
- **Instructions retired**: parity to slightly lower (−0.6%…+0.2%; the +0.2%
  on apps/5000 is within sample noise). The wall win comes from scheduling,
  not from executing fewer instructions.
- **Max RSS**: shared lower on every fixture (−2% to −5%).
- **Involuntary context switches**: shared lower on every fixture (−10% to −26%).
- **System time**: shared is _higher_ (e.g. 2.27 s vs 1.24 s on apps/10000) —
  structural: up to 18 concurrent blocking file operations vs tokio's 4-thread
  blocking pool, plus worker park/unpark syscalls. Wall time is still lower;
  see the A/B below for why capping does not help.

## Blocking-cap A/B (`apps/10000`): keep `max_blocking_tasks = worker_threads`

PR #6270 capped the _tokio_ blocking pool at 4 threads on macOS (sys-time
collapsed ~63%, wall 553→458 ms on an IO-heavy path). Decision rule for the
shared runtime: adopt a `min(4)` default if cap-4 wins wall by >2% or sys-time
by >20% on `apps/10000`. Measured (shared binding, cap via
`ROLLDOWN_MAX_BLOCKING_THREADS=4`):

Every value below is a **median with no runs excluded** (wall: all 12
hyperfine runs; counters: all 3 `/usr/bin/time -l` samples). The run-spread
row is descriptive only; nothing is dropped from the medians.

| metric (apps/10000)                |                                             cap-default (18) |                 cap-4 |      cap-4 effect |
| ---------------------------------- | -----------------------------------------------------------: | --------------------: | ----------------: |
| wall median (all 12 runs)          |                                                     464.7 ms |              557.8 ms | **+20.0% slower** |
| wall run spread                    | 9 of 12 in 450.7–482.9; 3 burst runs 605.6 / 816.5 / 1,099.0 | all 12 in 548.3–594.1 |    see note below |
| sys time (median of 3)             |                                                       2.19 s |                2.60 s |      +18.7% worse |
| user time (median of 3)            |                                                       1.12 s |                1.49 s |        +33% worse |
| instructions retired (median of 3) |                                                     13.235e9 |              17.595e9 |      +32.9% worse |
| invol. ctx switches (median of 3)  |                                                       17,998 |               126,183 |          7× worse |

Run-spread note: every one of cap-default's nine non-burst runs (450.7–482.9)
is at least 65 ms faster than cap-4's fastest run (548.3); the three
cap-default burst runs are slower than every cap-4 run, and the medians —
which include them — still put cap-4 20.0% behind.

The rule fires in the _opposite_ direction: cap-4 loses decisively on every
axis, so the default stays `max_blocking_tasks = worker_threads`. The #6270
result does not transfer because the mechanism differs: tokio's cap shrinks a
_dedicated_ thread pool (fewer threads issuing concurrent kernel IO), while
the shared runtime's cap only limits how many of the existing pool threads may
occupy the blocking lane — the pool keeps all 18 threads, and the starved lane
turns into queueing plus park/unpark churn (the 7× involuntary-context-switch
and +4.4e9-instruction signature above). No Rust change was made
(`register_async_runtime` is untouched), so no crate tests were required.

## Anomalies observed (full disclosure)

- Ambient noise on this host arrives as multi-run bursts (~80–600 ms added,
  2–3 consecutive hyperfine runs). The first full matrix
  (`results-20260702-175337`) had one-sided bursts in the _tokio_ wall blocks
  of apps/5000 and apps/three10x (e.g. 736 ms outliers); those two fixtures'
  wall/threads rows were re-measured (`results-20260702-175551`), where bursts
  hit both sides equally. Medians are stable across both runs. apps/three10x
  counters come from the first run because the re-run's shared counter block
  was itself hit (0.55–0.65 s real vs 0.36 s baseline).
- The cap-default hyperfine block of the A/B (`results-ab-20260702-175713`)
  contains three burst runs (605.6 / 816.5 / 1,099.0 ms); no run is excluded
  from the reported medians, and the verdict is unaffected — cap-default's
  nine non-burst runs all beat cap-4's fastest run by 65+ ms, and its burst
  runs only push its own median up.
- In one probe session, the shared binding with the non-default
  `ROLLDOWN_MAX_BLOCKING_THREADS=1` peaked at 49 threads (4/4 runs) instead of
  25; it never reproduced across 16 subsequent runs and never occurred at the
  default settings used for every number in this document.
- Results directories are intentionally not committed
  (`scripts/misc/bench-async-runtime/.gitignore`); the tables above are the
  committed record.

## Wake-path certification: pre-Task-3 vs head (2026-07-03)

The formal re-certification Task 3 deferred: its round-0 A/B certified the
wake-path rewrite (SeqCst no-waiter fast path + targeted per-driver wakes +
LIFO slot) on all fixtures, but the round-1 LIFO-slot fixes (Ready-exit
flush, coop streak cap, blocking-closure slot bypass) only got an
ambient-contaminated spot re-gate. This round certifies the shipped code.

- **Date**: 2026-07-03, same host and fixtures as above, Node v24.12.0
- **A (pre)**: shared release binding + dist glue built from `aa58a088a`
  (the branch point before Task 3), sha256 `8d0c72b3…`, 16,095,680 B
- **B (head)**: shared release binding + dist glue built from `c6baf0beb`
  (Tasks 3–7 + the wasi-threads cfg fix), sha256 `26e1f474…`, 16,079,136 B
- Both sides rebuilt fresh for this round; the stale `/tmp/bench-shared.node`
  left by an earlier session (sha `05760d58…`) was verified different and not
  used.

**Method.** Paired interleaved A/B per fixture: 16 alternating A/B pairs
per window, runs 1–2 (pair 1–2) discarded as warmup, 14 kept; per-run wall
is `direct.mjs`'s own in-process build ms (excludes node boot — hence lower
absolute numbers than the hyperfine whole-process walls above); counters
come from `/usr/bin/time -l` wrapped around **every** run.

**Decision rule (as actually applied — every step checkable from the raw
logs).** The unit of inference is the interleaved pair, and the estimator
is the **median of per-pair relative deltas, pooled over every kept pair
of every window run for that fixture**. Nothing is excluded: no run, no
pair, no window. This host's ambient noise arrives as +150–450 ms bursts
hitting 1–3 runs of a 32-run window (see the Anomalies section above —
same machine, same behavior); the paired design absorbs them two ways: a
burst spanning both runs of a pair cancels inside that pair's delta, and a
one-sided burst corrupts only that single pair's delta, which the median
tolerates (worst case below: 2 corrupted pairs of 14). A kept-run wall σ

> 5% of the side median was the pre-registered trigger to run an
> **additional** window (it fired for apps/5000 and apps/10000; both got 3
> windows, all published below). As an _acceptance_ criterion that σ bar is
> unsatisfiable on this machine — three windows at different hours,
> including one at the calmest ambient observed (load ~2.0), each contain at
> least one burst, and only apps/1000 meets it — so **the PASS below is NOT
> claimed on σ**. It is claimed on: (1) the pooled pair-delta median vs the
> 1% bar, (2) agreement of the per-window medians (window-selection
> insensitivity), (3) pair-sign counts.

**Scope disclosure.** The two sides load their own checkout's dist glue —
head glue cannot drive the old binding (`registerTimerHost`, added in
Task 5, is called at import). The delta therefore spans everything Tasks
3–7 landed (wake path, deadlock detection, timer facility, wasi naming,
config pipeline), of which the wake path is the only change aimed at this
hot path. A/B ran with default env: MultiThread, 18 workers.

**Ambient + burst disclosure (from the logs; run numbers are the raw 1–16
indices, kept = 3–16).** Time Machine idle all windows (`tmutil` Running=0);
Spotlight churn at session start (load 6.7 → ~2.0); apps/10000 window 3 ran
at the calmest ambient of the session (load ~2.06, no processes above 40%
CPU). Burst inventory per window:

- apps/1000: none — the only window meeting the 5% σ bar (A/B σ 1.9%/1.8%).
- apps/5000 w1: bursts hit BOTH sides at runs 13–14 (A 347.9/371.2 vs
  B 362.6/364.8) — paired, so they largely cancel; σ 35.2%/34.6%.
- apps/5000 w2: one-sided B burst at run 12 (A 187.9 vs B 327.6), both
  sides at run 13 (358.7/391.4), A-heavier at run 14 (345.4/226.4);
  σ 33.8%/36.5%.
- apps/5000 w3: single one-sided B burst at run 16 (183.8 vs 362.6);
  σ 1.6%/27.2%.
- apps/10000 w1: one-sided bursts on each side at different runs — A run 7
  (402.1), B runs 3 (405.7) and 16 (645.4); σ 3.3%/21.3%.
- apps/10000 w2: both sides at run 14 (483.1/819.3), one-sided A run 15
  (613.0), mild B run 4 (410.5); σ 20.5%/34.8%.
- apps/10000 w3: both sides at run 13 (683.4/809.7), one-sided A run 14
  (441.9), mild B run 16 (362.6); σ 26.4%/35.8%.
- apps/three10x: both sides at run 16 (428.5/472.5); σ 11.7%/15.7%.

(An earlier revision of this section misdescribed apps/5000 w1 as
"one-sided A bursts, rejected" and w2 as purely paired; the inventory above
is recomputed from the raw logs, and no window is rejected — all pool.)
For apps/10000 window 3 the A-side dist glue was rebuilt from `aa58a088a`
(deterministic; binding bytes sha-identical to window 1–2's). Raw logs:
`.superpowers/sdd/arch-task-8-artifacts/cert/<fixture>[-windowN]/`
(gitignored scratch; the tables here are the committed record).

Pooled results (side values are pooled medians; deltas are pair-delta
medians; "won" = pairs where B is lower):

| fixture       | windows × pairs | wall A → B (ms) |        pair-Δ wall | instructions A → B |       pair-Δ instr | invol. ctx A → B |        pair-Δ ictx |
| ------------- | --------------- | --------------: | -----------------: | -----------------: | -----------------: | ---------------: | -----------------: |
| apps/1000     | 1 × 14          |     60.4 → 59.8 |  **−0.50%** (8/14) |  2.630e9 → 2.614e9 | **−0.63%** (12/14) |    3,904 → 3,372 | **−10.9%** (12/14) |
| apps/5000     | 3 × 14          |   181.9 → 181.1 | **−0.33%** (22/42) |  7.008e9 → 6.976e9 | **−0.36%** (31/42) |    9,420 → 8,286 | **−11.7%** (34/42) |
| apps/10000    | 3 × 14          |   356.8 → 354.6 | **−0.09%** (23/42) |  13.27e9 → 13.19e9 | **−0.39%** (34/42) |  15,450 → 13,385 | **−11.8%** (36/42) |
| apps/three10x | 1 × 14          |   298.4 → 298.5 |      +0.37% (5/14) |  8.745e9 → 8.741e9 |      −0.03% (7/14) |    4,654 → 4,734 |      +2.78% (6/14) |

Per-window pair-delta medians (window-selection insensitivity — criterion
2 of the decision rule):

| window | apps/5000 wall / instr / ictx   | apps/10000 wall / instr / ictx  |
| ------ | ------------------------------- | ------------------------------- |
| w1     | +0.94% (5/14) / −0.18% / −6.8%  | −0.46% (8/14) / −0.63% / −17.3% |
| w2     | −0.61% (9/14) / −0.54% / −16.2% | +0.57% (6/14) / −0.38% / −11.0% |
| w3     | −0.72% (8/14) / −0.45% / −11.7% | −0.09% (9/14) / −0.39% / −10.3% |

**Verdict under the stated criteria: PASS** (bar: no fixture's pooled
pair-delta wall median regresses > 1%).

- **Wall**: parity on every fixture — pooled medians −0.50% / −0.33% /
  −0.09% / +0.37%, all within ±0.5%; per-window wall medians stay inside a
  ±1.0 pp band and straddle zero on both multi-window fixtures, i.e. the
  wall delta is indistinguishable from zero and far from the −1% bar.
- **Instructions**: the round-0 instruction win **survives at head** on
  the apps fixtures (pooled −0.36…−0.63%; sign consistent in all six
  windows; 34/42 pairs on apps/10000) — the round-1 blocking-closure slot
  bypass did not permanently give it back, answering the question the
  contaminated fix-round re-gate left open.
- **Involuntary context switches**: pooled −10.9…−11.8% on the apps
  fixtures, sign consistent in all six windows (36/42 pairs on
  apps/10000). Round-0 measured −26…−33% for the wake-path change alone;
  this round measures a different span (through Task 7, in-process window,
  per-pair medians), so the magnitudes are not directly comparable — the
  direction and pair-sign significance are.
- three10x is parity on all three axes, consistent with round-0's smallest
  win (−0.42% wall) sitting below this host's noise.
