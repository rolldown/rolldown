# WASI Flavors — Why Two Artifacts, Not One

> Companion to [implementation.md](./implementation.md) (which documents how
> the flavors are built and regenerated) and `docs/guide/wasi.md` (the user
> guide). This file answers one recurring design question and records the
> evidence, so it is not re-derived: **can rolldown ship a single `.wasm`
> file and switch between threaded and single-threaded operation at
> runtime?** It cannot. The reasons are type-level, not preference.

## The decision

Rolldown ships two independent WASI binaries, built from two Rust targets:

| Rust target | npm artifact | Loader | Used by |
| --- | --- | --- | --- |
| `wasm32-wasip1-threads` | `@rolldown/binding-wasm32-wasi` | `rolldown-binding.wasi.cjs` | Node fallback, browsers with cross-origin isolation |
| `wasm32-wasip1` | `@rolldown/binding-wasm32-wasip1` | `rolldown-binding.wasip1.cjs`, `rolldown-binding.wasip1-deferred.js` | `@rolldown/browser`, workerd, non-isolated pages |

Build scripts: `packages/rolldown/package.json` — `build-binding:wasi`
passes `--target wasm32-wasip1-threads`, `build-binding:wasi-single` passes
`--target wasm32-wasip1`. Both compile the same `async-runtime` feature; the
target decides the threading model.

**Naming trap:** the short npm suffix `-wasm32-wasi` is the **threaded**
artifact. `-wasm32-wasip1` is the single-threaded one. This mirrors
napi-rs's artifact names and looks like a stale rename; it is not — both
packages are live, and the short suffix means threads.

## What the JS side does and does not control

Both binaries import their linear memory from JS (`env.memory`); the loader
constructs a `WebAssembly.Memory` and hands it in. That gives JS control
over **size** — `initial`, `maximum`, and growth at runtime (grown shared
memory is why JS-side typed-array views need refresh; that is the
emnapi memory-growth work this program upstreamed).

JS does **not** control the memory's kind. The `shared` flag is part of the
module's declared import type, fixed at compile time, and
`WebAssembly.instantiate` validates the supplied memory against it — the
same check that rejects a too-small memory (`LinkError: memory import is
smaller than initial …`) also rejects a sharedness mismatch, in both
directions:

- non-shared memory → threaded module: `LinkError`
- shared memory → threadless module: `LinkError`

## Evidence, from the built binaries

Import/export sections of the two artifacts, parsed from the debug builds
in `packages/rolldown/src/` (flags byte per the WebAssembly binary format:
bit 0 = has maximum, bit 1 = shared):

```
rolldown-binding.wasm32-wasi.wasm        (wasm32-wasip1-threads)
  memory import env.memory   flags=0x03  shared=true   min=1023  max=65536
  host import  wasi.thread-spawn
  export       wasi_thread_start

rolldown-binding.wasm32-wasip1.wasm      (wasm32-wasip1)
  memory import env.memory   flags=0x01  shared=false  min=1023  max=65536
  (no thread-spawn import, no wasi_thread_start export)
```

## Why a runtime switch cannot exist

Three compile-time facts, in increasing depth:

1. **The memory type.** Shown above. One binary declares `shared=true`, the
   other `shared=false`, and instantiation enforces the declaration. There
   is no instantiation that defers this decision to runtime.

2. **The thread ABI.** The threaded binary imports `wasi.thread-spawn`. The
   JS runtime (`@napi-rs/wasm-runtime`) answers it by spawning a Web Worker
   that re-instantiates the *same module against the same shared memory*
   and enters at `wasi_thread_start`. The threadless binary contains none
   of this plumbing — the import, the entry export, and the TLS setup are
   absent from the binary, not disabled.

3. **Rust std.** `wasm32-wasip1-threads` links a standard library where
   `thread::spawn` works through that ABI. On `wasm32-wasip1` it does not
   exist, and the shared scheduler is compiled in CurrentThread mode with
   the JS timer host driving it. This is a property of the target's std,
   selected when `rustc` runs.

**Atomics are not the barrier.** The spec permits atomic instructions on
non-shared memory (they behave as plain accesses; only `memory.atomic.wait`
traps there). Compiling the threadless target with `+atomics` would change
nothing above: the memory type, the thread ABI, and std are the barriers.

**"Ship only the threaded binary everywhere" also fails.** A `shared=true`
memory is backed by `SharedArrayBuffer`. The environments the threadless
flavor exists for — workerd, StackBlitz-class embedders, any page without
cross-origin isolation — have no `SharedArrayBuffer`, so the single binary
would fail at `LinkError` precisely where it is needed most. This is why
napi-rs#3353 made `wasm32-wasip1` a first-class second artifact rather
than a build flag on the existing one.

## Where the selection actually happens

At load time, in JS, per environment — never inside wasm:

- Node without a native binary: the generated `binding.cjs` fallback chain
  resolves the threaded package.
- `@rolldown/browser`: bundles the threadless flavor only
  (`build-browser-pkg` → `build-binding:wasi-single`).
- workerd: the deferred threadless loader
  (`rolldown-binding.wasip1-deferred.js`), asserted by
  `scripts/wasi/check-workerd-packed-consumer.mjs`.
- Tests: flavor predicates in `packages/rolldown/tests/src/runtime-flavor.ts`
  (`isWasiTest`, `isSingleThread`) skip what a flavor cannot run.

Each flavor also carries its own generated declaration file
(`rolldown-binding.wasi.d.cts`, `rolldown-binding.wasip1.d.cts`); the
regeneration order that keeps them consistent is documented in
[implementation.md](./implementation.md).
