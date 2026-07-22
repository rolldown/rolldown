# Experimental Async Runtime

Every current Rolldown binding runs a shared scheduler that executes async
polling, CPU work, and bounded blocking work in one scheduling domain on the
default `MultiThread` flavor (see [CurrentThread](#currentthread) for that
flavor's narrower scope). Check the loaded artifact before configuring it —
older published artifacts predate this runtime:

```ts
import { configureAsyncRuntime, getRuntimeCapabilities } from 'rolldown/experimental';

const capabilities = getRuntimeCapabilities();
if (capabilities.asyncRuntimeBuild && !capabilities.wasi) {
  configureAsyncRuntime({
    flavor: 'MultiThread',
    workerThreads: 12,
    maxBlockingTasks: 8,
  });
}
```

`configureAsyncRuntime` must run before the binding's first async operation.
Configuration is process-wide for that loaded binding and remains immutable
after the first runtime generation starts.

## Artifacts

| Artifact                                    | Backend | Supported flavor                             |
| ------------------------------------------- | ------- | -------------------------------------------- |
| Standard native npm binding                 | Shared  | `MultiThread` or `CurrentThread`             |
| Any WebAssembly binding (both WASI targets) | Shared  | `CurrentThread`                              |
| Legacy Tokio-era published binding          | Tokio   | reports its own flavor; configuration throws |

Use `getRuntimeCapabilities()` instead of inferring the backend or target from
environment variables. Legacy artifacts without a capability report are
recognized by the package's compatibility shim, which synthesizes
`backend: 'tokio'` for them.

## Environment

The binding reads these variables once during module initialization:

- `ROLLDOWN_RUNTIME=single|current-thread|multi|multi-thread`
- `ROLLDOWN_WORKER_THREADS`
- `ROLLDOWN_MAX_BLOCKING_THREADS`
- `ROLLDOWN_PARK_DEADLINE_MS`
- `ROLLDOWN_DRAIN_LINGER_US`

`ROLLDOWN_RUNTIME` selects the flavor and the two thread-count variables size
the topology. `ROLLDOWN_PARK_DEADLINE_MS` is not a topology knob: it opts into
deadline-based `block_on` deadlock detection, which is disabled by default.
`ROLLDOWN_DRAIN_LINGER_US` sets the MultiThread drainer's idle-linger budget
in microseconds: `0` disables lingering, unset or unparsable values keep the
built-in default (500µs), and oversized values are clamped. It has no
`configureAsyncRuntime` option; the effective value is reported by
`getAsyncRuntimeConfig()` as `drainLingerUs`.

Native `ROLLDOWN_*` worker counts are capped at 256. Explicit
`configureAsyncRuntime()` thread values above 256 throw
instead of being silently clamped. Valid values still undergo
topology normalization: CurrentThread becomes `(1, 1)`, MultiThread promotes
one worker to two, applies the platform worker cap, and limits blocking
admission to one less than the effective worker count. On WebAssembly, the
shared backend ignores the multi-thread request and reports one `CurrentThread`
execution lane. Later environment changes have no effect.

Without thread-count overrides, the native runtime starts from
`min(physical CPUs, process-available CPUs)`, promotes MultiThread to at least
two workers, and admits at most `workerThreads - 1` blocking tasks. Both
defaults remain subject to the production and platform caps above.

The published Node threaded-WASI loader additionally sizes emnapi's async-work
pool from `NAPI_RS_ASYNC_WORK_POOL_SIZE`, falling back to `UV_THREADPOOL_SIZE`
and then 4. The generated loader normalizes the value to a positive integer
capped at 1024 before creating emnapi workers and before exposing the
environment to the WASI guest. That pool serves napi-rs async work on the
host side; it is not part of the shared scheduler's topology.

## Metrics

`getAsyncRuntimeMetrics()` returns cumulative event counters, live gauges, and
lifetime high-water marks. `resetAsyncRuntimeMetrics()` clears cumulative
events only. It preserves live gauges and high-water marks while work can still
publish retirement updates.

Legacy Tokio-era bindings exposed the same query functions for a stable API
shape, but their scheduler counters are zero.

## CurrentThread

`CurrentThread` is cooperative and does not make arbitrary blocking calls on
the JavaScript host thread safe. Query `blockOnJsThreadSafe`,
`watchSupported`, `devSupported`, and `timers` from
`getRuntimeCapabilities()` before enabling features that depend on them.

On the native artifact, `ROLLDOWN_RUNTIME=single` (or a `CurrentThread`
override) governs the shared scheduler only: async polling moves to the
JavaScript host thread, blocking admission narrows to one task, and the
reported `workerThreads: 1` / `threads: false` describe exactly that
scheduler topology. Rolldown's data-parallel compute is not part of the
shared scheduler's topology: it keeps using Rayon's process-global pool,
which is sized from the CPU count and spins up on first use — the same
compute pool every Tokio-era binding used on every flavor. Native
`CurrentThread` is therefore a scheduler-flavor knob, not a process-wide
single-thread mode. Only the WebAssembly artifacts, which compile without
Rayon, execute on a single lane.

Every WebAssembly artifact remains `CurrentThread`, including the published
threaded build for `wasm32-wasip1-threads`: the shared scheduler has no
WebAssembly multi-thread executor, so that target gains threads for napi-rs
host work but not a parallel executor. Consequently `wasm32-wasip1-threads`
reports `devSupported: false` — `dev()` is unavailable there even though the
legacy Tokio-era artifact for the same target reported it as supported. Watch
mode remains unsupported on every WASI artifact.
