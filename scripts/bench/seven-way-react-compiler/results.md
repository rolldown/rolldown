# Seven-Way React Compiler Bench Results

**Date:** 2026-06-23
**Machine:** Darwin arm64, Apple M4 (passively-cooled M4 Air)
**Rolldown commit:** see git log on `feat/native-bridge-plugin-poc`
**Binding build:** release (`just build-rolldown-release`)
**Plugin cdylib build:** release (`cargo build --release -p bench_native_lib_plugin`)
**Corpus:** Infisical `frontend/` — 1500-file subset (the full 3860-file corpus
multiplies wall time without changing the relative ordering)
**Skip-list:** 69 files excluded (`builtin-skip.json`) — see "Builtin panic
investigation" below

## Variants

- **utils-sync** — JS plugin's `transform` hook calls `transformSync` from `rolldown/utils` with `{ reactCompiler: { panicThreshold: 'none' } }`.
- **utils-async** — JS plugin's `async transform` hook awaits `transform` from `rolldown/utils`.
- **bridge-sync** — JS plugin's `transformNativeBridge` hook receives a `bigint` handle wrapping `Box<NativeStringHolder>`. Calls `BenchOxcTransformer.transformNative`.
- **bridge-async** — JS plugin's `transformNativeBridgeAsync` returns `Promise<bigint>`. Calls `BenchOxcTransformer.transformNativeAsync`.
- **native-lib** — `defineNativeLibPlugin({ path })` loads `bench_native_lib_plugin.dylib`. Dispatch direct from rolldown's worker threads via the `rolldown_native_plugin_abi` C ABI. No napi, no JS thread.
- **builtin** — no plugin; `BundlerOptions.transform.reactCompiler = { panicThreshold: 'none' }`. **Requires the skip-list** to avoid panic-triggering files; also requires the bench-only patch in `pre_process_ecma_ast.rs` that discards Transformer diagnostics (to match the bridge variants' `let _ = …`).
- **bridge-parallel** — `bridge-sync` registered via `defineParallelPlugin`. ~8 JS worker threads each calling `transformNative` in parallel.

**Fairness:** all variants now discard Transformer diagnostics. The bridge
variants do this naturally (their napi methods take `let _ = ...`). For
builtin, this is a temporary patch in `crates/rolldown/src/utils/pre_process_ecma_ast.rs`
that bypasses the partition + `warnings.extend(BuildDiagnostic::from_oxc_diagnostics(...))`
block. That patch is bench-only and should not ship.

## Primary table — LIMIT=1500 (1431 after skip), 4 iterations (1 warm-up dropped, 3 measured)

```
skip-list: 69 files excluded (builtin-panic)
corpus: 1431 files
iterations: 4 (1 warm-up dropped, 3 measured)

--- variant: utils-sync ---
  warm-up: ~1820 ms
  iter 1:  1817.4 ms
  iter 2:  1819.4 ms
  iter 3:  1888.4 ms

--- variant: bridge-sync ---
  warm-up: ~1600 ms
  iter 1:  1604.6 ms
  iter 2:  1612.2 ms
  iter 3:  1620.0 ms

--- variant: native-lib ---
  warm-up: ~530 ms
  iter 1:   525.0 ms
  iter 2:   527.3 ms
  iter 3:   568.3 ms

--- variant: builtin ---
  warm-up:  467.3 ms
  iter 1:   487.3 ms
  iter 2:   462.7 ms
  iter 3:   416.5 ms

--- variant: bridge-parallel ---
  warm-up:  522.3 ms
  iter 1:   488.8 ms
  iter 2:   501.7 ms
  iter 3:   503.5 ms
```

| Variant | min (ms) | median (ms) | mean (ms) | speedup vs utils-sync |
|---|---:|---:|---:|---:|
| utils-sync       | 1817 | 1819 | 1842 | 1.00x |
| bridge-sync      | 1605 | 1612 | 1612 | 1.13x |
| native-lib       |  525 |  527 |  540 | 3.45x |
| bridge-parallel  |  489 |  502 |  498 | 3.63x |
| **builtin**      |  **416** |  **463** |  **456** | **3.93x** |

## Reading the numbers

With the skip list in place and the bench-only patch suppressing builtin's
diagnostic collection, the ordering finally makes structural sense:

**`builtin` is the fastest** (3.93x over `utils-sync`). It's doing the least
work per module: parse → semantic → Transformer(react_compiler=ON) → keeps
AST. One pass, no plugin round-trip, no JS thread, no string materialization
for the result.

**`bridge-parallel` is second** (3.63x). Same per-module work as `bridge-sync`,
but split across ~8 JS worker threads. The parallelism makes up for the
overhead of doing the transform twice (plugin pass + rolldown's no-RC internal
pass).

**`native-lib` is third** (3.45x). The C-ABI dispatch is cheap, but each
module still gets parsed/transformed/codegen'd twice (once in the plugin's
own pipeline, once in rolldown's internal pre_process). At full scale that
double-pass is what `bridge-parallel` works around with parallelism and what
`builtin` skips entirely.

**`bridge-sync` (1.13x) and `utils-sync` (1.00x) are essentially tied** —
the bridge encoding's zero-copy/no-UTF wins are minor compared to the
per-module React Compiler + rolldown bundler overhead.

So the real ordering is: the value of "skip the plugin layer entirely"
(builtin) > "skip the JS thread" (bridge-parallel) > "skip the napi crossing"
(native-lib) > "skip the UTF round-trip" (bridge-sync) > baseline.

## Builtin panic investigation

When running `transform.reactCompiler` at the bundler level on Infisical's
frontend, ~4.6% of files hit:

```
oxc_ecmascript-0.136.0/src/side_effects/statements.rs:98:57:
internal error: entered unreachable code
```

Line 98 panics when the side-effects analyzer encounters an unresolved TS-only
declaration in the AST: `TSEnumDeclaration`, `TSImportEqualsDeclaration`,
`TSModuleDeclaration`, `TSGlobalDeclaration`, `TSInterfaceDeclaration`, or
`TSTypeAliasDeclaration`. These should have been removed by oxc's TS-to-JS
lowering pass before side-effects analysis. When React Compiler runs first
on the bundler-level path, TS lowering sometimes doesn't happen on the resulting
AST, leaving those nodes in place.

The JS-plugin variants bypass this because the plugin returns a code STRING,
and rolldown re-parses it from scratch. The re-parsed AST has no TS leftovers
because the plugin's pipeline included codegen, which materializes the code
post-TS-lowering.

**Workaround in this bench:** `_find-panics.mjs` probes each corpus file
individually with builtin's transform config, detects the oxc panic by
watching stderr for `unreachable code`/`panicked at`, and writes the file
path to `builtin-skip.json`. `run.mjs` applies the skip list to every variant
(so the comparison stays apples-to-apples). 51 files panicked + 18 errored
out of the first 1500 = 4.6% panic rate, 1.2% error rate (errors are mostly
"untranspiled TypeScript syntax" / "untranspiled JSX syntax" which is the same
underlying TS-lowering-didn't-run issue surfaced differently).

**Proper upstream fix** would be either:
- Force TS lowering to happen before side-effects analysis when
  `transform.reactCompiler` is set, OR
- Have side-effects analysis tolerate TS declarations (treat them as no-ops).

## Secondary table — LIMIT=15, 6 iterations, all seven variants

Async variants only run on the secondary table because they deadlock above
~16 concurrent in-flight transforms (the napi-rs `async fn` ↔ tokio
interaction this PoC has previously characterized).

Approximate numbers (re-running for final-final numbers is straightforward):

| Variant | median (ms) | speedup vs utils-sync |
|---|---:|---:|
| utils-sync       | ~127 | 1.00x |
| utils-async      | ~ 69 | 1.85x |
| bridge-sync      | ~ 17 | 7.5x |
| bridge-async     | ~ 13 | 9.7x |
| native-lib       | ~ 15 | 8.5x |
| builtin          | ~  6 | 21x (fastest at small scale) |
| bridge-parallel  | ~ 47 | 2.7x (worker spawn overhead) |

At LIMIT=15 the absolute numbers favor builtin even more strongly — the
plugin variants amortize their double-pass cost across a tiny per-module
budget, while builtin's single pass stays cheap.

## Build steps to reproduce

```
just build-rolldown-release
cargo build --release -p bench_native_lib_plugin
node scripts/bench/seven-way-react-compiler/setup.mjs   # once

# Identify panic-triggering files for builtin (one-shot, ~30 min)
PROBE_TIMEOUT_MS=4000 node scripts/bench/seven-way-react-compiler/_find-panics.mjs

# Primary (all sync variants, skip list applied to every variant)
LIMIT=1500 ITERS=4 \
  VARIANTS=utils-sync,bridge-sync,native-lib,builtin,bridge-parallel \
  node scripts/bench/seven-way-react-compiler/run.mjs

# Secondary (all seven, small corpus to keep async + builtin within bounds)
LIMIT=15 ITERS=6 \
  VARIANTS=utils-sync,utils-async,bridge-sync,bridge-async,native-lib,builtin,bridge-parallel \
  node scripts/bench/seven-way-react-compiler/run.mjs

# Disable skip list (e.g. on a different corpus)
NO_SKIP=1 LIMIT=… node scripts/bench/seven-way-react-compiler/run.mjs
```

## Caveats

- **Bench-only patch in `crates/rolldown/src/utils/pre_process_ecma_ast.rs`** discards Transformer diagnostics. Without it, `builtin` errors out on the React Compiler diagnostics that `panicThreshold: 'none'` doesn't suppress (Refs during render, MemoDependencies, "Compilation Skipped: Use of incompatible library"). The bridge variants don't surface these because their napi methods/cdylib already `let _ = ...` the Transformer return. **Do not ship this patch.**
- **The skip list applies to every variant** so the comparison stays apples-to-apples on the same corpus subset. Set `NO_SKIP=1` to use the full corpus (builtin will fail).
- **`shimMissingExports: true`** covers a handful of intra-tree type-only-imports-used-as-values.
- **`onLog: () => {}`** swallows React Compiler warnings.
- **mimalloc** emits "invalid pointer" warnings throughout — pre-existing rolldown/oxc allocation pattern.
- Async variants deadlock above ~16 concurrent in-flight transforms on Node 24.x. Secondary table only.
- The bench cdylib is built in release; debug-mode dispatch costs would mask the variant comparison.
- Run-to-run variance on a passively-cooled M4 Air can shift absolute numbers by 5–20%. Relative ordering is the stable signal.
