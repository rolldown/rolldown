# Controlled `resolveId` and `load` cases

This fixture measures direct Rolldown builds with ordinary JavaScript plugins and Parallel JS Plugin instances. It does not use Vite.

Each measured child is a fresh Node.js 24.18.0 process. The parent creates one corpus outside the timed child, rotates variants by repeat, requires a clean Rolldown worktree, validates final output hashes and byte counts, and records the exact Rolldown commit, host load averages, Node binary and SHA-256, and native binding SHA-256. `parentObservedProcessElapsedMs` covers process launch through output hashing; `totalElapsedMs` excludes Node startup and the top-level Rolldown import but includes plugin import/factory, worker initialization, `rolldown()`, `generate()`, and `close()`. Formal wall-time data is collected only after `just build-rolldown-release` and with instrumentation disabled.

The `resolveId` cases create 512 controlled specifiers. A separate ordinary support loader turns the resolved virtual IDs into code. The `load` cases use a separate ordinary resolver and make the measured plugin return the module code. A wide entry makes all 512 tasks independently ready; the chain variant exposes only one next module at a time.

`workIterations` is fixed synchronous checksum work whose result changes the resolved ID or returned code. `syncFsProbes` performs a fixed number of synchronous `existsSync` calls against an existing file. `asyncDelayMs` models already-asynchronous work: an ordinary plugin can have many timers outstanding on one event loop, while Parallel JS Plugin holds one worker permit for each pending Promise. `resultPaddingBytes` changes the exact load result bytes crossing the worker boundary.

Instrumented cases report two boundaries:

- JavaScript handler time, input/returned bytes, calls, per-worker distribution, and active handler concurrency through a shared buffer.
- Rust wrapper queue wait, permit-held time, input/returned bytes, results, and concurrency. The wrapper begins before native hook filtering, so `nullResults` directly counts native-filter misses that nevertheless acquired a worker permit.

Instrumentation changes timings and exists only to explain uninstrumented wall results. Summed queue wait across concurrently queued calls is not build wall time.

Run a smoke after building the release binding:

```sh
NODE=/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node
just build-rolldown-release
$NODE examples/par-plugin/cases/controlled-hooks/run-matrix.mjs examples/par-plugin/cases/controlled-hooks/smoke-matrix.json /tmp/controlled-hooks-smoke.json
```

Generate a summary with:

```sh
$NODE examples/par-plugin/cases/controlled-hooks/summarize-matrix.mjs /tmp/controlled-hooks-smoke.json /tmp/controlled-hooks-smoke-summary.json
```

Run the small correctness and semantics probes with:

```sh
$NODE examples/par-plugin/cases/controlled-hooks/probe-correctness.mjs
```

The probes verify four non-performance properties: a native-filter miss currently acquires a worker permit before the filter runs; closure state is partitioned across plugin instances and can change observable results; a same-plugin `this.resolve(..., { skipSelf: false })` call deadlocks with one worker but completes with two; and synchronous/rejected `resolveId` and `load` errors propagate without a process abort.
