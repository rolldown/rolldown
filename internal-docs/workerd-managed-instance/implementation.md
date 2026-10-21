# Workerd Managed Instance — Implementation

`@rolldown/browser/workerd` runs rolldown in workerd on the threadless
`wasm32-wasip1` binding. Each `createInstance()` owns its N-API state, emnapi
context and unshared memory.

```
  src/workerd.ts, src/workerd-build.ts     public entry: createInstance, build()
        │  build(): enterInstance → rolldown build → exitInstance
  src/workerd-managed-instance.ts          instance handle: build count, dispose gate
        │
  rolldown-binding.wasip1-deferred.js      cli loader: module check, fresh claimed Memory,
                                           per-instance hosts, settlement barrier, dispose()
```

- The deferred loader is `@napi-rs/cli` output, committed verbatim. The
  rolldown layer adds only whether a build uses the instance.
- `build.ts` `bundleManagedWorkerdLoaders()` bundles the entries self-contained
  (emnapi, wasm runtime, `buffer` polyfill) and aliases `src/binding.cjs` to
  `src/binding-workerd-proxy.ts`, which forwards each use to the entered
  instance: one `lazyExport` per name in the `napi-rs-artifact-metadata`
  header of the plain threadless loader `src/rolldown-binding.wasip1.cjs` (a
  missing header fails the build), minus the seven host exports and
  `getRuntimeCapabilities` (static while no instance is active).
- Consumers import the wasm under a `CompiledWasm` rule (`docs/guide/wasi.md`).

## Invariants

1. **Fresh memory.** Every instance runs on a Memory no other instance used.
   The loader allocates and claims it; the entry forwards only the page counts,
   never a caller `memory`, because a second bundled copy of the entry has its
   own loader that would not know the Memory is in use.
2. **One active instance per bundled copy.** The binding proxy routes the
   pipeline to one instance's exports at a time; entering another instance
   throws "Another workerd Rolldown instance is currently active".
3. **Build count.** Each `build()` holds one count from `enterInstance` to
   `exitInstance` (a `finally`), so the count covers `closeBundle` and a failed
   build.
4. **Dispose gate.** `dispose()` rejects with "Cannot dispose this workerd
   Rolldown instance with N active binding operation(s) ..." while the count is
   above zero. Otherwise it sets the disposal-started flag, then runs the
   loader's `dispose()`; if that rejects, the handle stays undisposed and a
   later call retries.
5. **No use after disposal starts.** `enterInstance`, `build()` and the
   internal `instanceExports()` throw once the flag is set; `.memory` throws
   once disposal completed, and the handle drops the loader instance so it
   retains no memory.
6. **Private instance disposal.** `build({ module })` retries a rejected
   dispose of its private instance (the loader's retryable
   `ERR_NAPI_WASI_CLEANUP_PENDING`) a few macrotasks apart; if it still fails,
   the error is reported and the handle is dropped. When the build also
   failed, the build error is always what `build()` rejects with; the disposal
   error becomes its `cause`, or, if it already has one (a release error, or
   its own), an `AggregateError` of that cause and then the disposal error. A
   `cause` that refuses the assignment (read-only, getter-only, or a throwing
   setter) is redefined as a data property. A thrown value that is not an
   object, or whose `cause` cannot be read or can be neither assigned nor
   redefined (a frozen error, a non-extensible one with no own `cause`, or a
   non-configurable `cause` that refuses the assignment), drops the disposal
   error; nothing is thrown in its place (`attachDisposeError` in
   `workerd-build.ts`). A completed `dispose()` destroys the environment and
   drops the handle's loader reference; the Memory itself is reclaimed only
   once nothing references it and the host garbage-collects, which workerd
   rarely does. An instance dropped without a completed `dispose()` is
   reclaimed the same way (nothing in the loader or emnapi pins it), so
   keeping a stuck handle would not free it sooner; the entry holds no
   registry of them.

The public handle has no raw binding exports: only `build()` reaches them, and
it counts every use. Objects taken through the internal `instanceExports()`
before disposal keep the dead instance's own Memory alive until dropped; emnapi
refuses every `napi_*` call into the destroyed context, so they cannot reach
any live instance. `build()` releases the result's raw binding wrappers after
materializing it, so a retained result keeps no instance alive.

What a plugin keeps follows the same rule, because a binding wrapper's methods
close over its instance's emnapi env and `dropInner()` frees only the Rust
payload. `PluginContextData` and the output hooks detach what they release:

- the `buildStart` / `buildEnd` contexts and the context passed to
  `output.codeSplitting.groups[].name` drop their wrapper at the build's
  terminal release (`releaseBinding` on `PluginContextImpl` and
  `ChunkingContextImpl`); later binding calls throw "already released", while
  data the context holds or handed out stays readable. A `this.load()` /
  `this.resolve()` still in flight reads the wrapper once before its first
  await, so the deferred box drop (`releaseOrDefer`) still runs at its
  `endNativeCall`;
- the normalized input and output options drop theirs when their boxes are
  released (the invalidate callback before `writeBundle`, or the terminal
  release), after caching every box-backed field, so kept options stay
  readable;
- the chunks and assets of a `generateBundle` / `writeBundle` bundle drop
  theirs when the hook settles (`releaseOutputBundleItems` in
  `utils/transform-to-rollup-output.ts`), after caching every getter-backed
  field, so kept items stay readable and hold plain data only.

The first two are both needed: a kept plugin context reaches the options
through its `data`. Contexts kept from other hooks (`resolveId`, `load`,
`transform`, `generateBundle`, ...) still hold their wrapper, so each keeps its
instance's Memory alive until the plugin drops it; so does a kept
`generateBundle` / `writeBundle` bundle object, which reaches its hook's
context.

A plugin may `Object.freeze` what it keeps, so each of these objects holds
its wrapper in a small cell object (`BindingCell` in `utils/threadless-free.ts`,
`state` in `ChunkingContextImpl`, which also holds its module-info cache; a
bundle item's getters close over their cell) rather than a plain field:
freezing is shallow, so the release can still clear the cell. The cell and the
accessor that reads it are TypeScript-private, never `#private`: a getter
reached through a Proxy or a derived object runs with that object as `this`,
which fails a `#private` brand check.
`PluginContextData`'s release runs every item even when one throws, then
rethrows the first error. On a failed build, `RolldownBuild#build` keeps the build error and
attaches a release error as its `cause`; a later private-instance disposal
error joins it (invariant 6).

## Tests

`packages/workerd-tests/suite.mjs` (real workerd via Miniflare) and `memory.mjs`
(concurrent instances, RSS budget); `packages/rolldown/tests/workerd-loader.test.ts`,
`workerd-build.test.ts` and `workerd-output-ownership.test.ts`.

Related: [async-runtime/implementation.md](../async-runtime/implementation.md)
(WASI artifacts and loaders), [async-runtime/design.md](../async-runtime/design.md)
(why the threadless artifact exists).
