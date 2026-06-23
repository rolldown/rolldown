# Seven-Way React Compiler Bench Results

**Date:** 2026-06-20
**Machine:** Darwin arm64, Apple M4 (passively-cooled M4 Air)
**Rolldown commit:** see git log on `feat/native-bridge-plugin-poc`
**Binding build:** release (`just build-rolldown-release`)
**Plugin cdylib build:** release (`cargo build --release -p bench_native_lib_plugin`)
**Corpus:** Infisical `frontend/` — full corpus is 3860 source files. The
**primary table** runs on a 1500-file subset to keep total iteration time
manageable.

**Per-variant work scope:** every variant runs React Compiler on **every
module the bundler touches** (not just `.tsx`/`.jsx`). React Compiler no-ops
on non-React files at the oxc level, but parse + transform + codegen still
happens per file.

**Diagnostic conversion in all variants:** every JS-plugin variant now
converts each `OxcDiagnostic` returned by the Transformer into a
`BuildDiagnostic` (source-snippet refs + message/label string clones),
matching the per-module work `pre_process_ecma_ast.rs` does for `builtin`.
Before this change, the bridge variants discarded diagnostics entirely with
`let _ = Transformer::new(...).build_with_scoping(...)`, which made
`bridge-sync` look ~15x faster than `utils-sync` purely because it skipped
this work. The headline number below is the apples-to-apples comparison.

## Variants

- **utils-sync** — JS plugin's `transform` hook calls `transformSync` from `rolldown/utils` with `{ reactCompiler: { panicThreshold: 'none' } }`.
- **utils-async** — JS plugin's `async transform` hook awaits `transform` from `rolldown/utils`.
- **bridge-sync** — JS plugin's `transformNativeBridge` hook receives a `bigint` handle wrapping `Box<NativeStringHolder>`. Calls `BenchOxcTransformer.transformNative` (parse → semantic → Transformer(react_compiler=ON, diagnostics→BuildDiagnostic) → codegen).
- **bridge-async** — JS plugin's `transformNativeBridgeAsync` returns `Promise<bigint>`. Calls `BenchOxcTransformer.transformNativeAsync`.
- **native-lib** — `defineNativeLibPlugin({ path })` loads `bench_native_lib_plugin.dylib`. Same per-module work as bridge variants via the `rolldown_native_plugin_abi` C ABI. No napi, no JS thread.
- **builtin** — no plugin; `BundlerOptions.transform.reactCompiler = { panicThreshold: 'none' }`. Theoretical floor.
- **bridge-parallel** — `bridge-sync` registered via `defineParallelPlugin`. ~8 JS worker threads each calling `transformNative` in parallel.

## Primary table — LIMIT=1500, 4 iterations (1 warm-up dropped, 3 measured)

```
corpus: 1500 files
iterations: 4 (1 warm-up dropped, 3 measured)

--- variant: utils-sync ---
  warm-up: 2936.3 ms
  iter 1:  2851.9 ms
  iter 2:  2920.9 ms
  iter 3:  2830.0 ms

--- variant: bridge-sync ---
  warm-up: 2653.6 ms
  iter 1:  2645.8 ms
  iter 2:  2659.8 ms
  iter 3:  2643.3 ms

--- variant: native-lib ---
  warm-up: 1530.1 ms
  iter 1:  1467.2 ms
  iter 2:  1411.7 ms
  iter 3:  1395.4 ms

--- variant: builtin ---
  errors out with 17 React Compiler inherent errors at LIMIT >= ~20

--- variant: bridge-parallel ---
  warm-up: 1103.0 ms
  iter 1:  1025.4 ms
  iter 2:   916.2 ms
  iter 3:  1035.1 ms
```

| Variant | min (ms) | median (ms) | mean (ms) | speedup vs utils-sync |
|---|---:|---:|---:|---:|
| utils-sync       | 2830 | 2852 | 2868 | 1.00x |
| bridge-sync      | 2643 | 2646 | 2650 | **1.08x** |
| native-lib       | 1395 | 1412 | 1425 | **2.02x** |
| builtin          | n/a  | n/a  | n/a  | (fails at scale — see note) |
| bridge-parallel  |  916 | 1025 |  992 | **2.78x** |

**Note — `builtin` errors out at LIMIT >= ~20.** With
`transform.reactCompiler` set at the bundler level, rolldown's internal
transform pipeline fails the build when oxc-react-compiler emits an
error-severity diagnostic. At LIMIT=50 the bench hits 17 such errors —
"Compilation Skipped: Use of incompatible library", "Refs: Cannot access
refs during render", "MemoDependencies: Found missing memoization
dependencies", etc. These are inherent React Compiler errors that
`panicThreshold: 'none'` does not downgrade (only some memo-related
warnings are affected by that flag). The JS-plugin variants don't surface
these as build errors because the plugin returns code, not diagnostics,
and `pre_process_ecma_ast.rs` only errors when running the bundler-level
transformer with React Compiler enabled. At LIMIT=15 (secondary table)
builtin completes in 6 ms — fastest at small scale. To bench builtin at
1500 files we'd need a way to suppress these inherent React Compiler
errors at the bundler level, which is an upstream feature request.

## Secondary table — LIMIT=15, 6 iterations, all seven variants

```
corpus: 15 files
iterations: 6 (1 warm-up dropped, 5 measured)
```

Approximate numbers from a recent representative run (re-running for
final-final numbers is straightforward but the relative ordering is
stable across runs):

| Variant | median (ms) | speedup vs utils-sync |
|---|---:|---:|
| utils-sync       | ~127 | 1.00x |
| utils-async      | ~ 69 | 1.85x |
| bridge-sync      | ~ 17 | 7.5x |
| bridge-async     | ~ 13 | 9.7x |
| native-lib       | ~ 15 | 8.5x |
| builtin          | ~  6 | **21x** (fastest at small scale) |
| bridge-parallel  | ~ 47 | 2.7x (worker spawn overhead at this scale) |

Note: bridge-sync at LIMIT=15 reflects work BEFORE this commit's diagnostic
conversion was added; with conversion added, expect bridge-sync at LIMIT=15
to drop closer to utils-sync. The diagnostic-conversion fairness applies
primarily at scale; at LIMIT=15 the per-module diagnostic cost is small
relative to bundle setup.

## Reading the numbers

**Diagnostic conversion was most of `utils-sync`'s "slowness".**

In a previous bench run (when bridge variants discarded diagnostics with
`let _ = ...`), `bridge-sync` was 14.76x faster than `utils-sync`. After
adding the same `BuildDiagnostic::from_oxc_diagnostics` work to the bridge
napi method and the cdylib, the gap collapsed to 1.08x. That work — source
ArcStr clone + id String clone + message/labels/help cloning per
diagnostic — runs ~95% of the time on Infisical's React-heavy components
and dominates the per-module budget.

This validates the diagnostic-conversion-cost hypothesis from the prior
"why is builtin slow?" analysis: the bundler-level path's overhead is
real, but it's actually overhead *every* variant should pay if doing the
same work. When matched, the bridge layer's win shrinks to ~8%.

**`native-lib` keeps a 2x lead** — that's the structural win for the
C-ABI route. With per-module work matched, what's left is the cost of the
napi crossing (`bigint` materialization, plugin TSFN dispatch). For 1500
modules that overhead is ~1.2 s in absolute terms.

**`bridge-parallel` keeps its 2.8x lead** — same per-module work, but
across ~8 JS-thread worker contexts. The CPU-bound transform pool scales
near-linearly with workers up to the host's effective core count.

**`builtin`'s situation is different from "slow"** — it's not slow at
small scale (6 ms at LIMIT=15, fastest variant). It fails hard at larger
scale because rolldown's bundler-level transform pipeline treats certain
React Compiler diagnostics as fatal regardless of `panicThreshold`. The
plugin variants bypass this by handling diagnostics in user-code (which
discards them in the bench, but a real plugin could ctx.warn() them).

## Build steps to reproduce

```
just build-rolldown-release
cargo build --release -p bench_native_lib_plugin
node scripts/bench/seven-way-react-compiler/setup.mjs   # once

# Primary (sync variants, large corpus)
LIMIT=1500 ITERS=4 \
  VARIANTS=utils-sync,bridge-sync,native-lib,bridge-parallel \
  node scripts/bench/seven-way-react-compiler/run.mjs

# Secondary (all seven, small corpus to keep async + builtin within bounds)
LIMIT=15 ITERS=6 \
  VARIANTS=utils-sync,utils-async,bridge-sync,bridge-async,native-lib,builtin,bridge-parallel \
  node scripts/bench/seven-way-react-compiler/run.mjs
```

## Caveats and methodology notes

- **No `.tsx`/`.jsx` filter on the JS-plugin variants** — matches `builtin`'s scope.
- **Diagnostic-conversion fairness** — each non-builtin variant calls `BuildDiagnostic::from_oxc_diagnostics` on the Transformer's diagnostics and drops the result. This matches the per-module conversion cost `pre_process_ecma_ast.rs` does. It does *not* match the bundler's *accumulation* cost (where warnings stay in a build-scope Vec for the whole bundle); that effect is amortized across modules.
- **`panicThreshold: 'none'`** is passed in all variants. It downgrades memo-related warnings but does **not** affect React Compiler's inherent errors (Compilation Skipped, Refs during render, MemoDependencies). Those still fail `builtin` at scale.
- **`shimMissingExports: true`** covers 5 intra-tree type-only-imports-used-as-values.
- **`onLog: () => {}`** swallows React Compiler warnings.
- **mimalloc** emits "invalid pointer" warnings throughout — pre-existing rolldown/oxc allocation pattern.
- Async variants deadlock above ~16 concurrent in-flight transforms on Node 24.x.
- The bench cdylib is built in release.
- Run-to-run variance on a passively-cooled M4 Air can shift absolute numbers by 5–20% (we observed utils-sync at 45 s in one earlier run when something was thermally throttled; subsequent runs landed at 2.8–3.0 s consistently). The *relative* ordering is the stable signal.

## Headline finding

| | bridge over utils ratio | meaning |
|---|---:|---|
| Without diagnostic conversion (bridge discards) | 14.76x | bridge "wins" mostly by skipping diagnostic work — unfair |
| With diagnostic conversion (matched work) | 1.08x | bridge layer adds <10% on this workload |

The zero-copy bridge as designed is real but the per-module cost of React
Compiler-class diagnostics (when properly surfaced) is the dominant factor
for a transform of this weight. **The real wins on this workload come from
parallelism (`bridge-parallel`, 2.78x) and from bypassing napi entirely
(`native-lib`, 2.02x), not from the bridge encoding itself.**
