# WASI and workerd support

Rolldown publishes two WASI builds:

- `wasm32-wasip1-threads` (`@rolldown/binding-wasm32-wasi`) uses threads and
  shared memory.
- `wasm32-wasip1` (`@rolldown/binding-wasm32-wasip1`) has no threads and uses
  unshared memory. `@rolldown/browser` uses this build.

In Node.js, set `NAPI_RS_WASI_FLAVOR=wasm32-wasip1` (or `wasm32-wasi` for the
threaded build) to load exactly that build. Rolldown then never falls back to
another build or to the native binary.

## Runtime flavors

The threaded build defaults to the `MultiThread` flavor. Set
`ROLLDOWN_RUNTIME=single` to run it on `CurrentThread`. On every host, the
threaded build's heap stays below 2 GiB.

The threadless build always runs `CurrentThread`, and the native binary always
runs `MultiThread`. Both ignore `ROLLDOWN_RUNTIME`.

### Environment variables

The binding reads these once, when it loads. Later changes have no effect.

- `ROLLDOWN_RUNTIME` (threaded build only): `current`, `current-thread`,
  `single` or `single-thread` selects `CurrentThread`; `multi` or
  `multi-thread` selects `MultiThread`. Other values keep the default.
- `ROLLDOWN_WORKER_THREADS`: the number of scheduler workers.
- `ROLLDOWN_MAX_BLOCKING_THREADS`: how many workers may run blocking work at
  once. It is a limit on the same workers, not a separate pool.

| Setting                    | Value                                                                                    |
| -------------------------- | ---------------------------------------------------------------------------------------- |
| Native default workers     | The smaller of the physical and the available CPU count                                  |
| Threaded WASI workers      | 2 by default; `ROLLDOWN_WORKER_THREADS` is clamped to 2 to 4                             |
| `MultiThread`              | At least 2 workers; blocking tasks are limited to `workerThreads - 1` (also the default) |
| `CurrentThread`            | 1 worker and 1 blocking task                                                             |
| `ROLLDOWN_*` thread counts | Capped at 256; `0` or a non-number keeps the default                                     |

## Support matrix

Call `getRuntimeSupport()` from `rolldown/experimental` to check the loaded
build instead of guessing from environment variables. Its field names are in
parentheses below.

| Feature (field)                                 | Native | Threaded WASI                  | Threadless WASI             |
| ----------------------------------------------- | ------ | ------------------------------ | --------------------------- |
| One-shot `rolldown()` / `build()`               | Yes    | Yes                            | Yes                         |
| `dev()` (`dev`)                                 | Yes    | Yes on `MultiThread` (default) | No                          |
| `watch()` (`watch`)                             | Yes    | No                             | No                          |
| Parallel JavaScript plugins (`parallelPlugins`) | Yes    | No                             | No                          |
| `@rolldown/browser/workerd` (`workerd`)         | No     | No                             | Through `@rolldown/browser` |

`threadlessWasi` is true when the loaded binding is the threadless build.
Unsupported features throw `ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE` before
any work starts, so they fail at once instead of hanging.

`viteImportGlobPlugin()` does not follow directory symlinks on WASI.

## Plugins on threadless WASI

Threadless hosts, workerd above all, rarely run garbage-collection finalizers.
So on this build only, Rolldown frees native data early, and reading freed data
throws:

| What a plugin keeps                                                                                                   | Readable until                          |
| --------------------------------------------------------------------------------------------------------------------- | --------------------------------------- |
| `this` in any hook except `buildStart` / `buildEnd`, and native objects a hook receives, such as `meta.magicString`   | That hook call settles                  |
| `this` in `buildStart` / `buildEnd`, and the context passed to `output.codeSplitting.groups[].name`                   | `generate()` / `write()` settles        |
| Normalized options, the `moduleParsed` module info, the chunk passed to `renderChunk`, `this.getModuleInfo()` results | Always: they are copied into JavaScript |

So a `buildStart`-bound `this.addWatchFile` still works in `writeBundle`, but
throws in `closeBundle`, which runs at `bundle.close()`. Copy what you need
while its hook is running.

## Cloudflare Workers (workerd)

`@rolldown/browser/workerd` bundles in memory with the usual `input`, `plugins`
and `output` options:

```js
import { build, createInstance } from '@rolldown/browser/workerd';
import wasmModule from '@rolldown/browser/workerd/wasm';

const instance = await createInstance(wasmModule);

export default {
  async fetch() {
    const { output } = await build({
      instance,
      input: 'virtual:entry',
      plugins: [myVirtualFilesPlugin],
      output: { format: 'esm' },
    });
    return new Response(output[0].code);
  },
};
```

- Pass `module: wasmModule` instead of `instance` to create a private instance
  for one build; `build()` disposes it before returning.
- Each `createInstance()` call has its own Wasm memory and runtime, but only one
  instance can run builds at a time: builds on the same instance may overlap,
  and a build on another instance rejects. Share one module-scope instance
  across requests.
- To release an instance, wait for its builds to settle, then
  `await instance.dispose()`. It rejects while a build is running; if cleanup
  itself rejects, call it again.

Configure Wrangler to import the Wasm file as a precompiled module. The loader
rejects byte buffers, URLs and `Response` objects, because workerd does not
allow compiling Wasm at runtime.

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

### Memory

An instance starts with about 64 MiB of Wasm memory and can grow;
`createInstance(module, { initialMemoryPages, maximumMemoryPages })` sizes it
in 64 KiB pages. `instance.memoryBytes` and
`getWorkerdRuntimeStats()` report address space, not the memory the platform
counts.

Cloudflare Workers limits the JavaScript heap and Wasm memory of an isolate to
128 MB. Test representative bundles with `wrangler dev` before production, and
watch the memory metrics after you deploy. See the Cloudflare documentation for
[memory profiling](https://developers.cloudflare.com/workers/observability/dev-tools/memory-usage/),
[memory metrics](https://developers.cloudflare.com/workers/observability/metrics-and-analytics/#memory-usage),
and [platform limits](https://developers.cloudflare.com/workers/platform/limits/#memory).
