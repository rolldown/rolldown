# WASI and workerd support

Rolldown publishes two WASI flavors:

- `wasm32-wasip1-threads` uses the threaded napi-rs loader and the Tokio
  runtime.
- `wasm32-wasip1` uses an unshared-memory, threadless loader and the
  CurrentThread runtime. `@rolldown/browser` uses this flavor.

Query the loaded artifact instead of inferring support from environment
variables:

```js
import { getRuntimeCapabilities, getRuntimeSupport } from 'rolldown/experimental';

console.log(getRuntimeCapabilities());
console.log(getRuntimeSupport());
```

`getRuntimeSupport().threadlessWasi` reports that the loaded binding is
compatible with a threadless WASI host. `getRuntimeSupport().workerd` reports
the canonical managed browser-package workflow, so it is true only when the
threadless binding is loaded through `@rolldown/browser`. The separately
resolved `rolldown/workerd` and threadless optional-package facades follow the
same managed factory contract but are not inferred from the root binding's
runtime report.

## Support matrix

| Feature                                         | Native MultiThread | Native CurrentThread  | Threaded WASI         | Threadless WASI                     |
| ----------------------------------------------- | ------------------ | --------------------- | --------------------- | ----------------------------------- |
| One-shot `rolldown()` / `build()`               | Yes                | Yes                   | Yes                   | Yes                                 |
| `dev()`                                         | Yes                | No, fails immediately | Yes                   | No, fails immediately               |
| `watch()`                                       | Yes                | Yes                   | No, fails immediately | No, fails immediately               |
| Async built-in-plugin resolution                | Yes                | Yes                   | Yes                   | Yes                                 |
| Complete plugin error metadata and cause chains | Yes                | Yes                   | Yes                   | Yes                                 |
| Symbolic-link traversal                         | Yes                | Yes                   | No                    | No                                  |
| Managed deferred workerd loader                 | No                 | No                    | No                    | Through a public `./workerd` facade |

In both WASI flavors, plugin hook failures retain the original JavaScript
error's stack and custom properties, Rolldown's applicable `code`, `plugin`,
`hook`, and `id` metadata, and nested cause chains.

Unsupported public workflows throw
`ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE` before entering the binding. This
prevents configurations that cannot make progress from hanging.

## Browser async context

Callback-free `@rolldown/browser` builds work without additional host support
and do not lock async-context configuration. JavaScript callbacks, including
plugin hooks and functional input or output options, require host-backed async
context so Rolldown can reject reentrant build and close cycles without blocking
unrelated work.

Hosts that expose `globalThis.AsyncContext.Variable` are detected
automatically. Otherwise configure the host integration before the first
callback-bearing build:

```js
import { configureAsyncContext } from '@rolldown/browser/experimental';

configureAsyncContext({
  createStorage() {
    return hostAsyncContext.createStorage();
  },
});
```

Each storage must provide `getStore()` and `run(store, callback)`, and `run()`
must preserve the store through promises and `await`. A stack-only shim is not
sufficient. Without a valid provider, Rolldown fails before invoking user code
with an `AsyncContextUnavailableError` whose code is
`ERR_ROLLDOWN_ASYNC_CONTEXT_UNAVAILABLE`. Callback-bearing one-shot build
options are preflighted before Rolldown enters the N-API binding, preserving the
error's public `name` and `code` instead of relying on emnapi error translation.
Errors thrown later by callbacks may still be wrapped as build errors, so use
`getAsyncContextSupport()` for capability checks rather than relying only on
`instanceof`. A failed preflight does not lock configuration, so the host may
configure a provider and retry the same build.

## Managed workerd loader

Use the stable `@rolldown/browser/workerd` export instead of importing a
generated, binary-name-specific loader:

```js
import { createInstance } from '@rolldown/browser/workerd';
import rolldownWasm from '@rolldown/browser/workerd/wasm.wasm';

const instance = await createInstance(rolldownWasm);
try {
  const binding = instance.exports;
  // Use the low-level binding within this instance.
} finally {
  // Close every build created from binding before disposing the instance.
  instance.dispose();
}
```

`instantiate` remains an alias of `createInstance` for compatibility.
Published `rolldown/workerd` and
`@rolldown/binding-wasm32-wasip1/workerd` facades expose the same managed
factory. `@rolldown/browser/workerd` remains the canonical entry for workerd
applications.

Configure Wrangler to import the package's Wasm export as a precompiled
module:

```json
{
  "rules": [
    {
      "type": "CompiledWasm",
      "globs": ["**/*.wasm"],
      "fallthrough": true
    }
  ]
}
```

Each call creates an independent emnapi context, N-API environment, scheduler,
and unshared Wasm memory. Concurrent instances do not share runtime ownership.
`dispose()` is idempotent after successful cleanup and releases the managed
handle's references. If a cleanup hook throws, the instance remains
undisposed and a later `dispose()` call retries cleanup. Close every build
first, and do not retain aliases to `instance.exports` or `instance.memory`
after disposal.
Caller-provided `WebAssembly.Memory` objects are single-use per validated
initialization attempt. The loader claims memory before entering emnapi because
a failed instantiation may already have mutated it. Once initialization begins,
the memory cannot be reused, even if initialization fails or the resulting
instance is later disposed. Inputs rejected before memory validation do not
claim it.

The loader rejects byte buffers, URLs, and `Response` objects because workerd
requires a precompiled `WebAssembly.Module`.

## Memory validation

The current threadless loader declares 1,024 initial pages, a 64 MiB Wasm
address space. It can grow up to the memory32 limit when the host permits it.
`instance.memoryBytes` and `getWorkerdRuntimeStats()` report address-space and
instance-lifecycle data, not committed platform memory.

Cloudflare Workers limits the JavaScript heap and Wasm allocations in an
isolate to 128 MB. Before production use:

1. Run `node scripts/misc/check-workerd-memory.mjs` as a local lifecycle and
   RSS regression canary.
2. Exercise representative bundles with `wrangler dev`; open DevTools with
   `D` and take memory snapshots.
3. Repeat with production-like traffic and remote bindings.
4. Monitor the Workers memory-usage percentiles and `exceededMemory`
   invocation outcomes after deployment.

Miniflare and local address-space measurements are not substitutes for this
production gate.

See the Cloudflare documentation for
[memory profiling](https://developers.cloudflare.com/workers/observability/dev-tools/memory-usage/),
[memory metrics](https://developers.cloudflare.com/workers/observability/metrics-and-analytics/#memory-usage),
and [platform limits](https://developers.cloudflare.com/workers/platform/limits/#memory).
