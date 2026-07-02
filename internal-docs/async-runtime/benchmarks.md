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

| build | worker threads | blocking limit | where |
| --- | --- | --- | --- |
| tokio | 27 (`physical * 3 / 2`) | dedicated blocking pool, capped at **4** threads | `crates/rolldown_binding/src/lib.rs` `init` + `resolve_default_runtime_threads` (`crates/rolldown_binding/src/async_runtime.rs:200-209`) |
| shared | 18 (`num_cpus::get_physical()`) | `max_blocking_tasks` defaults to **worker_threads (18)**; blocking jobs run *on* the shared pool (no extra threads), clamped to pool size | `register_async_runtime` (`crates/rolldown_binding/src/async_runtime.rs:386-392`), clamp at `crates/rolldown_utils/src/async_runtime.rs:69` and `:485` |

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

| fixture | side | wall mean ± σ (ms) | wall median (ms) | instructions retired | max RSS (MiB) | invol. ctx switches | peak threads |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| apps/1000 | tokio | 118.3 ± 1.8 | 119.0 | 2.639e9 | 264.7 | 4,877 | 38¹ |
| | shared | **115.7 ± 2.1** | 116.0 | 2.634e9 | 252.4 | 4,304 | **25** |
| apps/5000 | tokio | 283.1 ± 33.9 | 267.4 | 7.050e9 | 662.7 | 12,080 | 56 |
| | shared | **276.1 ± 34.1** | 259.2 | 7.062e9 | 642.8 | 8,966 | **25** |
| apps/10000 | tokio | 491.9 ± 25.4 | 484.7 | 13.425e9 | 1,220.9 | 22,317 | 56 |
| | shared | **469.6 ± 18.4** | 461.3 | 13.345e9 | 1,178.9 | 17,508 | **25** |
| apps/three10x | tokio | 388.9 ± 16.1 | 383.9 | 8.779e9 | 798.2 | 7,034 | 56 |
| | shared | **382.4 ± 13.0** | 380.7 | 8.771e9 | 780.7 | 6,312 | **25** |

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
- **System time**: shared is *higher* (e.g. 2.27 s vs 1.24 s on apps/10000) —
  structural: up to 18 concurrent blocking file operations vs tokio's 4-thread
  blocking pool, plus worker park/unpark syscalls. Wall time is still lower;
  see the A/B below for why capping does not help.

## Blocking-cap A/B (`apps/10000`): keep `max_blocking_tasks = worker_threads`

PR #6270 capped the *tokio* blocking pool at 4 threads on macOS (sys-time
collapsed ~63%, wall 553→458 ms on an IO-heavy path). Decision rule for the
shared runtime: adopt a `min(4)` default if cap-4 wins wall by >2% or sys-time
by >20% on `apps/10000`. Measured (shared binding, cap via
`ROLLDOWN_MAX_BLOCKING_THREADS=4`):

Every value below is a **median with no runs excluded** (wall: all 12
hyperfine runs; counters: all 3 `/usr/bin/time -l` samples). The run-spread
row is descriptive only; nothing is dropped from the medians.

| metric (apps/10000) | cap-default (18) | cap-4 | cap-4 effect |
| --- | ---: | ---: | ---: |
| wall median (all 12 runs) | 464.7 ms | 557.8 ms | **+20.0% slower** |
| wall run spread | 9 of 12 in 450.7–482.9; 3 burst runs 605.6 / 816.5 / 1,099.0 | all 12 in 548.3–594.1 | see note below |
| sys time (median of 3) | 2.19 s | 2.60 s | +18.7% worse |
| user time (median of 3) | 1.12 s | 1.49 s | +33% worse |
| instructions retired (median of 3) | 13.235e9 | 17.595e9 | +32.9% worse |
| invol. ctx switches (median of 3) | 17,998 | 126,183 | 7× worse |

Run-spread note: every one of cap-default's nine non-burst runs (450.7–482.9)
is at least 65 ms faster than cap-4's fastest run (548.3); the three
cap-default burst runs are slower than every cap-4 run, and the medians —
which include them — still put cap-4 20.0% behind.

The rule fires in the *opposite* direction: cap-4 loses decisively on every
axis, so the default stays `max_blocking_tasks = worker_threads`. The #6270
result does not transfer because the mechanism differs: tokio's cap shrinks a
*dedicated* thread pool (fewer threads issuing concurrent kernel IO), while
the shared runtime's cap only limits how many of the existing pool threads may
occupy the blocking lane — the pool keeps all 18 threads, and the starved lane
turns into queueing plus park/unpark churn (the 7× involuntary-context-switch
and +4.4e9-instruction signature above). No Rust change was made
(`register_async_runtime` is untouched), so no crate tests were required.

## Anomalies observed (full disclosure)

- Ambient noise on this host arrives as multi-run bursts (~80–600 ms added,
  2–3 consecutive hyperfine runs). The first full matrix
  (`results-20260702-175337`) had one-sided bursts in the *tokio* wall blocks
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
