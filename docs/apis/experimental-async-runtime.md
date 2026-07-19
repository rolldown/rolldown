# Experimental Async Runtime

Rolldown can be built with a shared scheduler that runs async polling, CPU
work, and bounded blocking work in one scheduling domain. The standard native
npm binding still uses Tokio. Check the loaded artifact before configuring it:

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

| Artifact                                              | Backend | Supported flavor                     |
| ----------------------------------------------------- | ------- | ------------------------------------ |
| Standard native npm binding                           | Tokio   | `MultiThread`; configuration throws  |
| Published threaded WASI (`wasm32-wasip1-threads`)     | Tokio   | `MultiThread`; configuration throws  |
| Custom native binding built with `async-runtime`      | Shared  | `MultiThread` or `CurrentThread`     |
| Custom WebAssembly binding built with `async-runtime` | Shared  | `CurrentThread` on both WASI targets |

Use `getRuntimeCapabilities()` instead of inferring the backend or target from
environment variables. Threadless `wasm32-wasip1` must use `async-runtime`;
the binding rejects the unusable Tokio/threadless feature combination at
compile time because napi-rs cannot execute built-in async tasks there.

## Environment

The binding reads these variables once during module initialization:

- `ROLLDOWN_RUNTIME=single|current-thread|multi|multi-thread`
- `ROLLDOWN_WORKER_THREADS`
- `ROLLDOWN_MAX_BLOCKING_THREADS`
- `ROLLDOWN_PARK_DEADLINE_MS`

`ROLLDOWN_RUNTIME` and `ROLLDOWN_PARK_DEADLINE_MS` configure only the shared
backend. The thread-count variables also configure native Tokio. Native
`ROLLDOWN_*` worker counts are capped at 256; native Tokio blocking threads are
capped at 512. Explicit `configureAsyncRuntime()` thread values above 256 throw
instead of being silently clamped. Valid shared-runtime values still undergo
topology normalization: CurrentThread becomes `(1, 1)`, MultiThread promotes
one worker to two, applies the platform worker cap, and limits blocking
admission to one less than the effective worker count. On WebAssembly, the
shared backend ignores the multi-thread request and reports one `CurrentThread`
execution lane. Later environment changes have no effect.

Without thread-count overrides, native Tokio uses
`floor(min(physical CPUs, process-available CPUs) * 3 / 2)` workers and four
blocking threads. The native shared backend starts from
`min(physical CPUs, process-available CPUs)`, promotes MultiThread to at least
two workers, and admits at most `workerThreads - 1` blocking tasks. Both
defaults remain subject to the production and platform caps above.

The published Node threaded-WASI loader instead sizes Tokio's emnapi work pool
from `NAPI_RS_ASYNC_WORK_POOL_SIZE`, falling back to `UV_THREADPOOL_SIZE` and
then 4. The generated loader normalizes the value to a positive integer capped
at 1024 before creating emnapi workers and before exposing the environment to
the WASI guest. `getAsyncRuntimeConfig()` reports that effective value.

## Metrics

`getAsyncRuntimeMetrics()` returns cumulative event counters, live gauges, and
lifetime high-water marks. `resetAsyncRuntimeMetrics()` clears cumulative
events only. It preserves live gauges and high-water marks while work can still
publish retirement updates.

Tokio bindings expose the same query functions for a stable API shape, but
their scheduler counters are zero.

## CurrentThread

`CurrentThread` is cooperative and does not make arbitrary blocking calls on
the JavaScript host thread safe. Query `blockOnJsThreadSafe`,
`watchSupported`, `devSupported`, and `timers` from
`getRuntimeCapabilities()` before enabling features that depend on them.

A custom shared-runtime WebAssembly artifact remains `CurrentThread` even when
compiled for `wasm32-wasip1-threads`; that target does not add a shared
multi-thread executor. Watch mode remains unsupported on WASI.
