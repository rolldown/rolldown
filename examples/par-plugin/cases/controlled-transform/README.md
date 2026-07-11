# Controlled Parallel Transform Fixture

This direct-Rolldown fixture separates transform task size, ready-call concurrency, worker-pool queueing, callback service, initialization, payload, CPU, and process RSS. It is research code, not a proposed public API.

## Setup and invocation

Build the optimized native binding before collecting performance data:

```sh
mise exec node@24.18.0 -- just build-rolldown-release
```

From this directory, run the committed matrices with the pinned Node.js binary:

```sh
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./wall-primary-matrix.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./instrumented-primary-matrix.json
```

The primary, crossover, secondary-axis, and targeted confirmation matrices are separate files so noisy points can be repeated without rerunning the full suite. `wall-heavy-confirm-matrix.json` is the 15-round confirmation for the heavy-work point.

Pass a third argument to write the raw JSON report to a file while printing only a one-line run summary, for example `./run-matrix.mjs ./wall-primary-matrix.json /tmp/wall-primary.json`.

The runner creates each corpus outside the measured child process in a unique temporary directory, performs one discarded fresh-process warmup for every variant, and rotates variant order across measured rounds. Every measured sample is a new Node.js process and, for parallel variants, a newly initialized worker pool.

## Primary fields

- `totalElapsedMs` starts before the ordinary or parallel plugin module is imported and its main-side factory is called, and ends after `build.close()`. This is the primary wall-time field.
- `pluginSetupElapsedMs` covers the main-process plugin import and factory. Worker implementation import and factory time occur later inside `rolldownApiElapsedMs` and are also decomposed by initialization metrics.
- `rolldownApiElapsedMs` covers `rolldown()`, `generate()`, and `close()` after the main-side plugin object exists.
- `peakRssBytes` comes from `/usr/bin/time -l` around the measured child. Corpus generation occurs in the parent and therefore does not inflate this child peak.
- `workIterations` is a fixed number of JavaScript integer and string operations. The checksum is returned in a fixed-width comment so V8 cannot discard the loop as unused. Observed handler duration, rather than an assumed time per iteration, is used in analysis.
- `minimumSourceBytes` is a per-file lower bound. Import statements can make the wide entry or chain modules larger; `totalSourceBytes` is the measured corpus total.

## Instrumentation semantics

Wall-time claims use only `instrumentation: false` runs. Instrumented runs add clocks, shared atomics, byte counts, and JSON reporting and are used only to explain the uninstrumented result.

- JavaScript `handler*` fields count only transform handler hits and measure time inside the controlled JavaScript handler.
- Rust `wrapper*` fields count every call that enters the ParallelJsPlugin transform wrapper, including native filter misses.
- `permitQueueWaitNs` starts when the wrapper requests a worker and ends when a worker permit is acquired. It does not include time before Rolldown invokes this plugin.
- `permitHeldNs` starts immediately after acquisition and ends after the permit is returned to the worker pool. It includes native filter evaluation, Node-API dispatch, worker event-loop scheduling, JavaScript execution, and return conversion.
- `permitInFlight` counts concurrently held worker permits. `wrapperOutstanding` counts wrapper calls either waiting for or holding a permit. Neither field by itself proves when a module became ready elsewhere in the module loader.
- Initialization metrics report main-observed pool startup, each worker's measured implementation import, factory, binding conversion and registration, and pool termination. The difference between `mainReadyMs` and `measuredBootstrapMs` includes worker creation, scheduling, and static worker-script imports that start before worker-local timing can run.

The current wrapper acquires a permit before evaluating the declarative transform filter. The fixture therefore expects Rust wrapper calls and input bytes to be at least the matching JavaScript handler values. A null result for Rolldown's internal runtime module is an observed example of that extra permit path, not a handler call-count mismatch.

The runner records a raw output hash and a second hash after replacing the unique temporary corpus path in Rolldown's region comments. It requires raw hashes to match across variants within one corpus and uses the normalized hash for reproducibility across separate matrix invocations. It also rejects missing or duplicate metrics, unbalanced counters, missing worker numbers, invalid durations, factory or worker-mask mismatches, handler byte mismatches, errors or cancellations, and permit concurrency above the configured worker count.

## Boundaries

The fixture models stateless synchronous JavaScript work with deterministic output. It does not model compiler caches, source maps, diagnostics, asynchronous native work, cross-module state, Vue or Svelte behavior, watch, rebuild, or HMR. A chain graph tests the absence of wide module-level concurrency; real plugin conclusions require the separately pinned Vue and Svelte cases.
