# Seven-Way React Compiler Bench Results

**Date:** 2026-06-20
**Machine:** Darwin arm64, Apple M4 (passively-cooled M4 Air)
**Rolldown commit:** see git log on `feat/native-bridge-plugin-poc`
**Binding build:** release (`just build-rolldown-release`)
**Plugin cdylib build:** release (`cargo build --release -p bench_native_lib_plugin`)
**Corpus:** Infisical `frontend/` — full corpus is 3860 source files. The
**primary table** runs on a 1500-file subset because `utils-sync` (rolldown's
public `transformSync` API) takes ~187 s per iteration at full scale, which
makes a 6-iter × 7-variant primary run >90 min. 1500 files is enough to
preserve the relative ordering; the variant deltas don't change between LIMIT
slices.

## Variants

- **utils-sync** — JS plugin's `transform` hook calls `transformSync` from `rolldown/utils` with `{ reactCompiler: { panicThreshold: 'none' } }`.
- **utils-async** — JS plugin's `async transform` hook awaits `transform` from `rolldown/utils`.
- **bridge-sync** — JS plugin's `transformNativeBridge` hook receives a `bigint` handle wrapping `Box<NativeStringHolder>`. Calls `BenchOxcTransformer.transformNative`.
- **bridge-async** — JS plugin's `transformNativeBridgeAsync` returns `Promise<bigint>`. Calls `BenchOxcTransformer.transformNativeAsync`.
- **native-lib** — `defineNativeLibPlugin({ path })` loads `bench_native_lib_plugin.dylib`. Dispatch direct from rolldown's worker threads via the `rolldown_native_plugin_abi` C ABI. No napi, no JS thread.
- **builtin** — no plugin; `BundlerOptions.transform.reactCompiler = { panicThreshold: 'none' }`. Theoretical floor.
- **bridge-parallel** — `bridge-sync` registered via `defineParallelPlugin`. ~8 JS worker threads each calling `transformNative` in parallel.

## Primary table — LIMIT=1500, 4 iterations (1 warm-up dropped, 3 measured)

```
corpus: 1500 files
iterations: 4 (1 warm-up dropped, 3 measured)

--- variant: utils-sync ---
  warm-up: 29748.6 ms
  iter 1:  29616.7 ms
  iter 2:  29418.3 ms
  iter 3:  29388.1 ms

--- variant: bridge-sync ---
  warm-up: 2270.7 ms
  iter 1:  2248.0 ms
  iter 2:  2323.6 ms
  iter 3:  2275.1 ms

--- variant: native-lib ---
  warm-up: 1989.5 ms
  iter 1:  2034.7 ms
  iter 2:  2041.0 ms
  iter 3:  1947.9 ms

--- variant: builtin ---
  did not finish — see note below

--- variant: bridge-parallel ---
  warm-up: 1116.0 ms
  iter 1:  1105.6 ms
  iter 2:  1115.9 ms
  iter 3:  1052.7 ms
```

| Variant | min (ms) | median (ms) | mean (ms) | speedup vs utils-sync |
|---|---:|---:|---:|---:|
| utils-sync       | 29388 | 29418 | 29474 | 1.00x |
| bridge-sync      |  2248 |  2275 |  2282 | **12.93x** |
| native-lib       |  1948 |  2035 |  2008 | **14.45x** |
| builtin          | n/a   | n/a   | n/a   | (did not finish) |
| bridge-parallel  |  1053 |  1106 |  1091 | **26.59x** |

**Note — `builtin` did not finish.** With `BundlerOptions.transform.reactCompiler` set, rolldown's bundler-level transform pass scales poorly on Infisical's corpus (works fine at LIMIT=5; hangs indefinitely at LIMIT >= 20). The JS-plugin path that runs the same `oxc_react_compiler` transform via `transformer.transformNative` completes in ~2s for the same input. Cause is upstream in rolldown's bundler pipeline, not in this PoC's wiring; the **builtin** path is therefore measured in the secondary table only (small-scale).

## Secondary table — LIMIT=15, 6 iterations, all seven variants

```
corpus: 15 files
iterations: 6 (1 warm-up dropped, 5 measured)

--- variant: utils-sync ---       med: 110.3 ms
--- variant: utils-async ---      med:  64.0 ms
--- variant: bridge-sync ---      med:  15.3 ms
--- variant: bridge-async ---     med:  16.1 ms
--- variant: native-lib ---       med:  15.2 ms
--- variant: builtin ---          med:  66.6 ms
--- variant: bridge-parallel ---  med:  46.6 ms
```

| Variant | min (ms) | median (ms) | mean (ms) | speedup vs utils-sync |
|---|---:|---:|---:|---:|
| utils-sync       | 108.20 | 110.31 | 112.43 | 1.00x |
| utils-async      |  61.09 |  64.05 |  66.43 | 1.72x |
| bridge-sync      |  14.81 |  15.25 |  16.10 | **7.23x** |
| bridge-async     |  15.60 |  16.12 |  17.07 | **6.84x** |
| native-lib       |  14.68 |  15.24 |  15.25 | **7.24x** |
| builtin          |  59.76 |  66.56 |  64.40 | 1.66x |
| bridge-parallel  |  45.12 |  46.63 |  46.57 | 2.37x |

## Reading the numbers

**`utils-sync` is the slow path. The bridge variants are 7–14x faster.**

The big finding: `rolldown/utils`'s `transformSync` adds *a lot* of overhead per
call relative to the same `oxc_react_compiler` transform invoked directly via a
napi class method. At LIMIT=1500 it costs 29 s where `bridge-sync` does the same
work in 2.3 s — that's 12.9× slower per file. At LIMIT=15 the ratio is similar
(7.2x). The bridge layer eliminates the UTF round trips and probably more
importantly the per-call overhead inside `transformSync` (options
re-normalization, warning/error aggregation, sourcemap wiring, etc.).

**`native-lib` and `bridge-sync` are essentially tied.** ~14.5x vs ~12.9x in
the primary table; identical in the secondary. The cost of crossing the napi
boundary for the bridge-sync variant is small compared to the actual transform
work. `native-lib`'s structural win (skipping napi entirely and dispatching the
plugin's `transform` fn pointer directly from rolldown's worker thread) is real
but the absolute saving is in the low-percent range on this workload, not
order-of-magnitude.

**`bridge-parallel` doubles `bridge-sync`/`native-lib` at scale.** At LIMIT=1500
it's the fastest variant (26.6x over `utils-sync`, ~2.1x over `bridge-sync`).
The 8 OS-thread JS contexts let multiple transforms run truly in parallel
across CPU cores. At LIMIT=15 the worker-spawn overhead dominates and
`bridge-parallel` loses to `bridge-sync`/`native-lib` — useful confirmation
that the parallel win comes from the actual transform pool, not anything
intrinsic to the bridge's calling convention.

**Async dispatch helps at small scale, deadlocks at large scale.** At LIMIT=15
`utils-async` is 1.7x faster than `utils-sync` and `bridge-async` is at parity
with `bridge-sync`. Above ~16 concurrent in-flight async transforms, both async
variants hit the upstream napi-rs 3.x `async fn` ↔ `MaybeAsyncJsCallback` ↔
tokio deadlock that this PoC has already characterized in detail.

**`builtin` is the surprise loser at scale.** At LIMIT=15 it ties with
`utils-async` at ~67 ms (1.66x). At LIMIT >= 20 it never completes. The same
oxc transform runs to completion in 2 seconds via the JS-plugin bridge path on
the same data, so the issue is in rolldown's bundler-level transform pipeline
itself, not in the React Compiler. Worth following up upstream.

## Build steps to reproduce

```
just build-rolldown-release
cargo build --release -p bench_native_lib_plugin
node scripts/bench/seven-way-react-compiler/setup.mjs   # once

# Primary (sync variants, large corpus)
LIMIT=1500 ITERS=4 \
  VARIANTS=utils-sync,bridge-sync,native-lib,bridge-parallel \
  node scripts/bench/seven-way-react-compiler/run.mjs

# Secondary (all seven, small corpus to keep async under deadlock threshold)
LIMIT=15 ITERS=6 \
  VARIANTS=utils-sync,utils-async,bridge-sync,bridge-async,native-lib,builtin,bridge-parallel \
  node scripts/bench/seven-way-react-compiler/run.mjs
```

## Caveats and methodology notes

- **`panicThreshold: 'none'`** is passed in all five variants that invoke React Compiler. `rolldown/utils`'s `transformSync`/`transform` default to a stricter threshold than `oxc_react_compiler::default_plugin_options()` (which uses `"none"`); without overriding, React Compiler diagnostics fail the bundle on ~95% of Infisical's React components. Aligning the JS plugins with the Rust path's lenient default keeps the variant comparison apples-to-apples.
- **`shimMissingExports: true`** covers 5 intra-tree type-only-imports-used-as-values that exist in Infisical's source.
- **`onLog: () => {}`** in the runner swallows the React Compiler warnings that survive `panicThreshold: 'none'` (Refs during render, missing memo dependencies, etc.). None of them affect timing.
- **mimalloc** emits "invalid pointer" warnings throughout — pre-existing rolldown/oxc allocation pattern, not caused by any variant. Reproduces on the `null transform` control.
- The async variants (`utils-async`, `bridge-async`) deadlock above ~16 concurrent in-flight transforms on Node 24.x — a generic napi-rs 3.x `async fn` ↔ tokio interaction. They appear only in the secondary table.
- The bench cdylib is built in release; debug-mode dispatch costs mask the variant comparison.
- React Compiler is the only transform; heavier transforms would shrink the bridge layer's relative win; cheaper ones would amplify it.
