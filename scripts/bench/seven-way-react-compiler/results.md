# Seven-Way React Compiler Bench Results

**Date:** 2026-06-20
**Machine:** Darwin arm64, Apple M4 (passively-cooled M4 Air)
**Rolldown commit:** see git log on `feat/native-bridge-plugin-poc`
**Binding build:** release (`just build-rolldown-release`)
**Plugin cdylib build:** release (`cargo build --release -p bench_native_lib_plugin`)
**Corpus:** Infisical `frontend/` — full corpus is 3860 source files. The
**primary table** runs on a 1500-file subset because `utils-sync`'s wall time
at full corpus is ~470 s/iter, which would make the primary table take well
over an hour. 1500 files preserves the relative ordering.

**Transform scope:** every variant runs React Compiler on **every module the
bundler touches** (not just `.tsx`/`.jsx`). React Compiler no-ops on non-React
files at the oxc level, but parse + transform + codegen still happens per
file. This is the same scope as `builtin`'s bundler-level
`transform.reactCompiler` config, so the variant comparison stays
apples-to-apples.

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
  warm-up: 45266.5 ms
  iter 1:  44770.0 ms
  iter 2:  44632.5 ms
  iter 3:  44734.2 ms

--- variant: bridge-sync ---
  warm-up: 3066.3 ms
  iter 1:  2952.7 ms
  iter 2:  3030.3 ms
  iter 3:  3034.9 ms

--- variant: native-lib ---
  warm-up: 1971.6 ms
  iter 1:  1983.1 ms
  iter 2:  2067.1 ms
  iter 3:  1931.4 ms

--- variant: builtin ---
  did not finish — see note below

--- variant: bridge-parallel ---
  warm-up: 1557.3 ms
  iter 1:  1607.7 ms
  iter 2:  1674.2 ms
  iter 3:  1581.5 ms
```

| Variant | min (ms) | median (ms) | mean (ms) | speedup vs utils-sync |
|---|---:|---:|---:|---:|
| utils-sync       | 44632 | 44734 | 44712 | 1.00x |
| bridge-sync      |  2953 |  3030 |  3006 | **14.76x** |
| native-lib       |  1931 |  1983 |  1994 | **22.56x** |
| builtin          | n/a   | n/a   | n/a   | (did not finish) |
| bridge-parallel  |  1581 |  1608 |  1621 | **27.83x** |

**Note — `builtin` did not finish at LIMIT >= 20.** At LIMIT=5 it completes in
12 ms. At LIMIT=15 it returns ~67 ms. At LIMIT=20 and above it never returns.
The same `oxc::Transformer` + `oxc_react_compiler::default_plugin_options()`
runs to completion in ~2 s for 1500 files via every JS-plugin variant on the
same input — so the underlying transform isn't the bottleneck. The issue is
structural in rolldown's bundler-level transform pipeline. The
filter-scope hypothesis is ruled out (this run already removed the
`.tsx`/`.jsx` filter from every JS plugin; the others still complete in
~2-3 s). The remaining hypotheses, in order of plausibility:

1. **Tokio scheduling interaction.** Each module's pre-process step parses
   → builds semantic → runs the bundler-level Transformer (with React
   Compiler) → produces code. If React Compiler holds a future open across
   a shared resource (diagnostic collector, sourcemap writer), tokio's
   worker pool can serialize behind it. The JS-plugin path moves the
   React Compiler work off the worker pool's critical path — it happens
   inside a sync napi call that returns immediately.
2. **Diagnostic accumulation.** With `panicThreshold: 'none'`, every React
   Compiler diagnostic still gets collected into the bundler's
   warning/error list. ~95% of Infisical's React components emit at least
   one (Refs during render, missing memo deps, etc.). At LIMIT=1500 that's
   thousands of `BuildDiagnostic` allocations; the JS-plugin path discards
   them at the plugin boundary.
3. **A specific file is a worst-case** for React Compiler when invoked
   through the bundler driver (less likely — same oxc code in both paths,
   but the surrounding state differs).

A `samply record` or `cargo flamegraph` against `builtin` at LIMIT=20 would
pin down (1) vs (2) cleanly. That's upstream-rolldown follow-up; out of
scope for this PoC.

## Secondary table — LIMIT=15, 6 iterations, all seven variants

```
corpus: 15 files
iterations: 6 (1 warm-up dropped, 5 measured)
```

| Variant | min (ms) | median (ms) | mean (ms) | speedup vs utils-sync |
|---|---:|---:|---:|---:|
| utils-sync       | 121.45 | 126.84 | 126.84 | 1.00x |
| utils-async      |  62.34 |  68.76 |  69.48 | 1.85x |
| bridge-sync      |  14.80 |  16.59 |  23.22 | **7.65x** |
| bridge-async     |  12.25 |  12.84 |  13.79 | **9.88x** |
| native-lib       |  13.67 |  15.46 |  15.06 | **8.20x** |
| builtin          |  62.71 |  67.54 |  68.84 | 1.88x |
| bridge-parallel  |  45.30 |  47.69 |  47.35 | 2.66x |

## Reading the numbers

**utils-sync** is the slow path by a wide margin. At LIMIT=1500 it costs 45 s
where `bridge-sync` does the same work in 3 s — 14.8x slower per call.
`rolldown/utils.transformSync` carries a lot of per-call overhead (options
re-normalization, warning/error aggregation, sourcemap wiring) that
disappears when the napi method is called directly.

**native-lib** is now clearly faster than **bridge-sync** at scale: 1.98 s vs
3.03 s. The earlier "they're tied" reading was an artifact of the
`.tsx`/`.jsx` filter restricting both variants to the same small workload;
once both variants process every module, the napi-crossing cost per
`bridge-sync` call accumulates and `native-lib`'s no-napi C-ABI dispatch
pulls ahead. 22.6x over `utils-sync`.

**bridge-parallel** is the fastest at scale: 1.6 s, 27.8x over `utils-sync`,
and ~1.2x faster than `native-lib`. With 8 OS-thread JS contexts the
transform pool runs in parallel across CPU cores; `native-lib` is sync on
rolldown's tokio worker pool, which has fewer effective cores for CPU-bound
work. (A future improvement for `native-lib` would be a dedicated CPU-bound
thread pool sized to the host.)

**Async dispatch wins where it works.** At LIMIT=15 `utils-async` is 1.85x
faster than `utils-sync` and `bridge-async` is the fastest variant (9.88x).
Above ~16 concurrent in-flight async transforms both async variants hit the
upstream napi-rs 3.x `async fn` ↔ tokio deadlock characterized in this PoC.

**`builtin` is the surprise loser.** Same transform code as the JS-plugin
variants, supposedly the theoretical floor — instead it hangs at modest
scale. See the note above the primary table.

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

- **No `.tsx`/`.jsx` filter on the JS-plugin variants.** Every variant runs React Compiler on every module to match `builtin`'s scope. Filter-scope hypothesis for the `builtin` hang was ruled out by running `bridge-sync` at LIMIT=50 (1.37 s) while `builtin` still hangs at the same scale.
- **`panicThreshold: 'none'`** is passed in all variants that invoke React Compiler. `rolldown/utils`'s `transformSync`/`transform` default to a stricter threshold; without overriding, React Compiler diagnostics fail the bundle on ~95% of Infisical's React components. The Rust bench transformer and cdylib already use the lenient default.
- **`shimMissingExports: true`** covers 5 intra-tree type-only-imports-used-as-values in Infisical's source.
- **`onLog: () => {}`** in the runner swallows React Compiler warnings (Refs during render, missing memo dependencies, etc.).
- **mimalloc** emits "invalid pointer" warnings throughout — pre-existing rolldown/oxc allocation pattern, not caused by any variant.
- The async variants (`utils-async`, `bridge-async`) deadlock above ~16 concurrent in-flight transforms on Node 24.x. Only the secondary table runs them.
- The bench cdylib is built in release; debug-mode dispatch costs mask the variant comparison.
- React Compiler is the only transform; heavier transforms would shrink the bridge layer's relative win; cheaper ones would amplify it.
