# Controlled `resolveId` and `load` release results

## Outcome

On this host and fixture, Parallel JS Plugin has clear value for `resolveId` and `load` when hundreds of independent hooks are ready together and each hook performs roughly millisecond-scale synchronous CPU work. It is a regression for cheap hooks, a dependency chain with only one ready module, cached synchronous file probes that remain short, large returned code without enough CPU work, and already-asynchronous `load` work.

The strongest non-wall-time value is main-thread isolation. One worker made both CPU-heavy hooks slightly slower than an ordinary plugin, but reduced event-loop p99 from approximately the whole 0.5 second build to 1.2–1.3 ms. Four workers both accelerated the build and kept p99 below 1.7 ms. An async timer-based `load` was already responsive as an ordinary plugin and became about 126 times slower with one worker because every pending Promise held the only worker permit.

## Environment and data gates

- Host: Apple M3 Pro, arm64 macOS 25.5.0, 12 logical CPUs, 36 GiB memory.
- Node: v24.18.0 at `/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node`, SHA-256 `ee6fb0e015284d83a91e8ec5213f43a157f8a392b58555301682892ba928c04a`.
- Rolldown release binding: built with `just build-rolldown-release` from `c9a41b1b93bdceab0572edb91c8d68bf630f3c4b`, SHA-256 `72376e7c924a33f459b3b1a2641bdab94c620fbcb5f810c3c053501e20e8766d`, 16,311,152 bytes.
- The primary wall and instrumented reports use the binding-source commit directly. The confirmation and isolation commits add only fixture runners, matrices, and documentation; the measured plugin and native implementation are unchanged and the raw reports pin the same binding hash and source commit.
- Every recorded matrix run used a fresh Node process, a clean worktree, a corpus created by the parent outside the timed child, rotated variant order, exact cross-variant raw and normalized output hashes, and identical output bytes. Successful instrumented runs also require exact handler/result byte accounting, balanced counters, zero errors and cancellations, and concurrency no greater than the worker count.
- Recorded samples: 6 smoke, 345 primary wall, 108 instrumented decomposition, 150 wall confirmation, and 40 event-loop isolation samples. The separate correctness probe covers native-filter misses, state partitioning, reentrancy, and four error paths.

`totalElapsedMs`, used below, excludes Node startup and the top-level `rolldown` import. It includes measured plugin import and factory, worker initialization, `rolldown()`, `generate()`, and `close()`. `parentObservedProcessElapsedMs` additionally covers process launch, top-level imports, and output hashing. Peak RSS comes from macOS `/usr/bin/time -l` and therefore also covers output hashing.

## CPU-heavy wide graph

The 15-round confirmation used 512 independently ready hooks and 500,000 fixed checksum operations per hook. Speedup is the median of each round's ordinary time divided by that round's worker time, not a ratio of unrelated medians.

| Hook        |  Variant | Median wall |      MAD | Paired speedup | Median user CPU |  Peak RSS |
| ----------- | -------: | ----------: | -------: | -------------: | --------------: | --------: |
| `resolveId` | ordinary |    548.8 ms |  48.7 ms |          1.00x |        534.9 ms | 111.6 MiB |
| `resolveId` | worker-1 |    670.0 ms | 112.6 ms |          0.85x |        605.3 ms | 125.0 MiB |
| `resolveId` | worker-2 |    406.1 ms |  70.1 ms |          1.50x |        720.7 ms | 139.7 MiB |
| `resolveId` | worker-4 |    264.0 ms |  40.5 ms |          2.32x |        869.3 ms | 162.2 MiB |
| `resolveId` | worker-8 |    195.2 ms |  31.7 ms |          2.91x |      1,105.7 ms | 209.9 MiB |
| `load`      | ordinary |    601.8 ms | 103.5 ms |          1.00x |        561.2 ms | 110.5 MiB |
| `load`      | worker-1 |    691.2 ms | 102.3 ms |          0.86x |        619.0 ms | 124.6 MiB |
| `load`      | worker-2 |    446.0 ms |  94.3 ms |          1.45x |        740.6 ms | 139.0 MiB |
| `load`      | worker-4 |    294.2 ms |  65.1 ms |          2.18x |        901.5 ms | 161.7 MiB |
| `load`      | worker-8 |    249.5 ms |  69.7 ms |          2.74x |      1,223.7 ms | 209.4 MiB |

The direction is robust in this batch: every paired worker-2/4/8 round beat ordinary for both hooks. Exact wall times are not stable enough to present as general constants. Fixed operations still pass through V8 tiering, per-isolate optimization, CPU frequency changes, and contention; for example, the primary `load` batch drifted substantially in its later rounds, which is why this independent 15-round confirmation exists.

Worker-1 is useful here for isolation, not throughput. Additional workers trade more total CPU and memory for lower wall time. Peak RSS rises by roughly 12–14 MiB per worker in these cases.

## Negative and boundary cases

| Case                                                | Ordinary median |                                  Best reported worker result | Conclusion                                                                                   |
| --------------------------------------------------- | --------------: | -----------------------------------------------------------: | -------------------------------------------------------------------------------------------- |
| Cheap `resolveId`, 512 calls                        |         20.7 ms |                               worker-1 83.0 ms, paired 0.26x | Worker startup and dispatch dominate.                                                        |
| Cheap `load`, 512 calls                             |         16.0 ms |                               worker-2 66.0 ms, paired 0.24x | Worker startup and dispatch dominate.                                                        |
| `resolveId`, 16 cached `existsSync` probes per call |         28.2 ms |                               worker-2 69.7 ms, paired 0.41x | Synchronous work is not automatically heavy enough to benefit.                               |
| CPU `resolveId`, 512-module chain                   |        539.2 ms |                                all workers 0.78–0.84x paired | Only one resolve is ready at a time.                                                         |
| CPU `load`, 512-module chain                        |        578.8 ms |                              worker-1 638.1 ms, paired 0.94x | Only one load is ready at a time.                                                            |
| Async `load`, 512 independent 5 ms timers           |         22.1 ms | worker-8 440.5 ms, paired 0.05x; worker-1 3,003.1 ms, 0.007x | Ordinary JS overlaps all pending Promises; Parallel JS Plugin caps them at the permit count. |
| `load`, 64 KiB returned padding per call            |         54.4 ms |                               worker-4 78.8 ms, paired 0.64x | Crossing about 33.6 MB of results is overhead, not useful parallel work.                     |

The chain evidence is direct: instrumented chain runs have maximum JavaScript handler activity, held permits, and outstanding wrappers all equal to one for every worker count. More plugin instances cannot create graph readiness.

The payload padding is returned from every `load` handler and counted identically by JavaScript and Rust at 65,620.8 bytes per call on average. Rolldown later removes the padding comment during code generation, so final hash equality proves final output equivalence while the instrumented counters prove that the payload crossed the hook boundary.

## Hook mechanics and overhead

Instrumentation is explanatory only; Atomics and timing calls perturb the path, so wall-time claims use the uninstrumented reports.

- Each wide case records 512 matching handlers and 513 Rust wrapper calls. The extra call is the physical entry, which misses the native filter only after acquiring a worker permit. This is D018, now observed for both `resolveId` and `load`, not just inferred from source.
- Cheap ordinary handlers average about 2.4 microseconds for `resolveId` and 1.0 microsecond for `load`. With worker-1, permit-held time averages about 51.2 microseconds and 15.8 microseconds respectively, before amortizing the roughly 50+ ms build-level pool cost.
- For wide CPU work, wrapper outstanding reaches 512 and permit in-flight reaches exactly 1/4/8. For cheap work, permits can be occupied while JavaScript handler activity fails to reach all workers, showing that dispatch and bridge work consume much of the task.
- For async `load`, ordinary JavaScript handler activity reaches 512. Parallel variants reach exactly 1/4/8 because a pending Promise retains its permit for the full timer. Median handler duration remains around 5.4–5.6 ms, but worker-1 serializes 512 timers.
- Queue-wait totals are sums across concurrently waiting wrappers and are not build wall time. They show pressure and readiness, not additive latency.
- Pool initialization in the instrumented batches varies materially by batch and worker count. Near-empty wall cases provide the safer practical statement: one to four workers add roughly 50–75 ms on this host, with larger counts and cold batches costing more.

## Main-thread responsiveness

The event-loop monitor uses 1 ms resolution. After `chdir` and GC, the child enables it, waits 25 ms, and resets the histogram. The monitor then covers plugin import/factory, worker initialization, `rolldown()`, `generate()`, and `close()`, followed by a trailing 25 ms sampling window. `totalElapsedMs` excludes both sampling windows. Output hashing is outside the monitor and total-time boundaries, and every variant passes the usual hash and byte gates.

| Case            |  Variant | Median wall | Paired speedup | Median event-loop mean | Median p99 | Median max |
| --------------- | -------: | ----------: | -------------: | ---------------------: | ---------: | ---------: |
| CPU `resolveId` | ordinary |    495.9 ms |          1.00x |               18.85 ms |  484.44 ms |  484.44 ms |
| CPU `resolveId` | worker-1 |    556.9 ms |          0.91x |                1.13 ms |    1.31 ms |    3.89 ms |
| CPU `resolveId` | worker-4 |    197.3 ms |          2.52x |                1.14 ms |    1.66 ms |    4.20 ms |
| CPU `load`      | ordinary |    513.8 ms |          1.00x |               18.44 ms |  502.01 ms |  502.01 ms |
| CPU `load`      | worker-1 |    562.2 ms |          0.92x |                1.13 ms |    1.22 ms |    3.34 ms |
| CPU `load`      | worker-4 |    222.4 ms |          2.23x |                1.14 ms |    1.66 ms |    3.33 ms |
| Async `load`    | ordinary |     23.9 ms |          1.00x |                1.30 ms |    3.89 ms |    3.89 ms |
| Async `load`    | worker-1 |  3,024.7 ms |         0.008x |                1.14 ms |    1.38 ms |    4.69 ms |

This separates two values that should not be conflated. Synchronous CPU hooks can justify one worker purely to keep the host Node process responsive even when wall time regresses. Already-asynchronous hooks are already responsive and can become drastically slower under the current one-permit-per-pending-call model.

## Correctness and semantics defects

### D018: native filters run after permit acquisition

The dedicated miss probe has one wrapper call, one acquired permit, one null result, and zero JavaScript handler calls. Formal 512-call builds likewise have 513 acquired wrappers and one null result. Moving filter interpretation before `WorkerManager::acquire()` would avoid queueing and worker dispatch for misses. Real plugins with broad hook registration and narrow filters can have far more misses than this controlled entry-only case.

### D011: same-plugin reentrancy can deadlock

An outer `resolveId` calls `this.resolve('controlled-reentrant:inner', importer, { skipSelf: false })`. Ordinary and worker-2 builds complete with the same output hash. Worker-1 holds its only permit while awaiting the nested resolve, and the nested wrapper waits for that same permit; the probe deterministically times out after 2,000 ms. Reentrancy needs an explicit policy: detect and reject this cycle with context, reserve/reuse the current instance safely, or document that `skipSelf: false` requires spare capacity and remains depth-bounded. Merely increasing the default worker count does not remove the defect.

### Per-instance mutable state changes semantics

The state probe creates the same closure counter in each factory. Ordinary produces one local sequence of 32 unique values. Worker-4 distributes calls across four factories, each with its own sequence; the returned IDs and final output hash differ. Plugin authors must decide whether state is immutable, per-instance/sharded, or explicitly shared through a safe mechanism such as a `SharedArrayBuffer` or main-thread coordination. Existing singleton caches, counters, and ordering assumptions cannot be moved into workers unchanged.

### D019: errors propagate, but worker attribution is lost

Synchronous `resolveId` throws and asynchronous `load` rejections exit with status 1 and no signal for ordinary and worker-1; the earlier PendingException/SIGABRT failure is not reproduced. Ordinary stderr includes the plugin label and the handler frame in `probe-impl.js`. Worker stderr retains only the message: both the plugin label and original handler frame are absent, and no hook, module ID, or worker number is added. The probe establishes visible message propagation and non-abort, not structured error-attribution parity. Failed builds also exit before the drop-time hook metrics line is emitted, limiting diagnostics. The worker bridge should serialize structured error context and preserve or reconstruct the worker stack.

## Where optimization effort should go

1. Evaluate native filters before acquiring a worker permit. This directly removes D018 miss overhead from `resolveId`, `load`, and `transform`.
2. Separate plugin-instance ownership from pending asynchronous operations. A worker isolate can keep multiple Promises outstanding, but the current permit stays held until settlement. Any change needs a documented state and reentrancy contract so concurrent callbacks do not silently break mutable plugins.
3. Make worker count adaptive to ready work and measured hook cost. One instance is enough for isolation, while throughput needs at least two and a wide graph; extra workers consume about 12–14 MiB each and more total CPU.
4. Amortize or lazily create the pool when builds/plugins can reuse it. Cheap builds cannot repay the approximately 50+ ms fixed cost.
5. Reduce bridge conversions and returned payloads. A 64 KiB result per module moves the case closer to parity only because ordinary also pays more work; it does not become a parallel win.
6. Detect same-plugin reentrancy and preserve worker error context before treating parallel plugins as a general authoring model.

## Scope and limitations

- These are direct Rolldown builds. Vite, watch, rebuild, and HMR are intentionally excluded.
- The timer case is deterministic already-asynchronous I/O-like behavior, not a disk or network benchmark. The synchronous file probe hits an existing cached file and should not be generalized to slow storage.
- `workIterations` fixes executed operations, not elapsed handler cost. V8 optimization and contention make operation-to-time mapping non-linear and different between hook shapes and isolates.
- The fixture uses 512 homogeneous modules to expose concurrency. Real projects need their ready-task distribution and plugin cost profile measured before choosing workers.
- The controlled evidence establishes mechanisms and boundaries; it does not claim one universal crossover threshold for `resolveId` or `load`.

## Artifacts

- [`smoke.raw.json`](./smoke.raw.json) and [`smoke.summary.json`](./smoke.summary.json)
- [`wall-primary.raw.json`](./wall-primary.raw.json) and [`wall-primary.summary.json`](./wall-primary.summary.json)
- [`instrumented-primary.raw.json`](./instrumented-primary.raw.json) and [`instrumented-primary.summary.json`](./instrumented-primary.summary.json)
- [`wall-confirmation.raw.json`](./wall-confirmation.raw.json) and [`wall-confirmation.summary.json`](./wall-confirmation.summary.json)
- [`isolation.raw.json`](./isolation.raw.json) and [`isolation.summary.json`](./isolation.summary.json)
- [`correctness.json`](./correctness.json)
