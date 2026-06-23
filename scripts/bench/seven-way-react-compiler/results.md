# Seven-Way React Compiler Bench Results

**Date:** 2026-06-23
**Machine:** Darwin arm64, Apple M4 (passively-cooled M4 Air)
**Rolldown commit:** see git log on `feat/native-bridge-plugin-poc`
**Binding build:** release (`just build-rolldown-release`)
**Plugin cdylib build:** release (`cargo build --release -p bench_native_lib_plugin`)
**Corpus:** Infisical `frontend/` — full 3860 source files
**Skip-list:** 684 files excluded (`builtin-skip.json`, 17.7% panic rate) —
see "Builtin panic investigation" below. Same 3176-file corpus is used for
every variant for an apples-to-apples comparison.

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

## Primary table — full corpus (3176 panic-free files), 4 iterations (1 warm-up dropped, 3 measured)

All seven variants run at full corpus. The earlier "async variants
deadlock above ~16 in-flight transforms" finding from the prior PoC does
not reproduce with the current config — both `utils-async` and
`bridge-async` complete cleanly at LIMIT=3176.

```
skip-list: 684 files excluded (builtin-panic)
corpus: 3176 files
iterations: 4 (1 warm-up dropped, 3 measured)

--- variant: utils-sync ---
  warm-up: 7333.7 ms
  iter 1:  7409.6 ms / iter 2:  7532.4 ms / iter 3:  7482.6 ms

--- variant: utils-async ---
  warm-up: 2356.8 ms
  iter 1:  2363.3 ms / iter 2:  2406.5 ms / iter 3:  2503.6 ms

--- variant: bridge-sync ---
  warm-up: 7019.2 ms
  iter 1:  6962.3 ms / iter 2:  6828.2 ms / iter 3:  6854.1 ms

--- variant: bridge-async ---
  warm-up: 1827.5 ms
  iter 1:  1874.0 ms / iter 2:  1782.3 ms / iter 3:  1832.9 ms

--- variant: native-lib ---
  warm-up: 2235.6 ms
  iter 1:  2170.1 ms / iter 2:  2260.8 ms / iter 3:  2246.8 ms

--- variant: builtin ---
  warm-up: 1908.0 ms
  iter 1:  1839.6 ms / iter 2:  1925.5 ms / iter 3:  1939.6 ms

--- variant: bridge-parallel ---
  warm-up: 2034.2 ms
  iter 1:  2030.3 ms / iter 2:  2038.2 ms / iter 3:  2071.5 ms
```

| Variant | min (ms) | median (ms) | mean (ms) | speedup vs utils-sync |
|---|---:|---:|---:|---:|
| utils-sync       | 7410 | 7483 | 7475 | 1.00x |
| bridge-sync      | 6828 | 6854 | 6882 | 1.09x |
| utils-async      | 2363 | 2407 | 2424 | 3.11x |
| native-lib       | 2170 | 2247 | 2226 | 3.33x |
| bridge-parallel  | 2030 | 2038 | 2047 | 3.67x |
| builtin          | 1840 | 1926 | 1902 | 3.89x |
| **bridge-async** | **1782** | **1833** | **1830** | **4.08x** |

## Reading the numbers

**`bridge-async` is the fastest variant** (4.08x over `utils-sync`).
The bridge's zero-copy bigint handle + the napi async wrapper means
each module's transform overlaps with the next, and JS-thread dispatch
costs amortize across the batch. It even edges out `builtin`.

**`builtin` is a close second** (3.89x). One transform pass per module,
no plugin round-trip — but no overlap either. The async variants beat
it here purely on concurrency.

**`bridge-parallel` (3.67x) and `native-lib` (3.33x) trade blows.**
`bridge-parallel` distributes across ~8 JS worker threads (real OS-thread
parallelism). `native-lib` skips napi entirely but runs on rolldown's
tokio thread pool, which is mostly busy doing other bundle work.

**`utils-async` (3.11x) shows what plain async dispatch buys.** Same
per-module work as `utils-sync`, but the JS thread gets to dispatch the
next transform while the napi side resolves the previous one. ~3x just
from concurrency, no other engineering.

**`bridge-sync` (1.09x) and `utils-sync` (1.00x) are essentially tied** —
the bridge encoding's zero-copy/no-UTF wins are tiny compared to React
Compiler's per-module cost. The win in this benchmark is all about
concurrency and skipping plugin round-trips, not data layout.

The real value ordering: async dispatch > skip plugin layer (builtin) >
worker-thread parallelism > skip-napi (native-lib) > sync plugin >
baseline.

**Earlier "async deadlock" reading was wrong.** A prior PoC iteration
characterized async variants as deadlocking above ~16 in-flight
transforms. With this branch's current config (no async-fn in the bench
napi class, no `Promise<Option<i64>>` wrapping, skip list applied), both
async variants complete cleanly at LIMIT=3176. The deadlock was specific
to the experimental `Promise<bigint>` napi return-type setup that was
backed out earlier on this branch.

## What's left to optimize? (spoiler: not much at the bridge layer)

After packing the `id` into the bigint handle (so neither source nor id
crosses napi as a marshalled JS string), bridge-sync at LIMIT=3176 still
clocks 6925 ms — essentially unchanged from the prior 6854 ms with the
separate `id` parameter. The reason: `ArcStr::from(args.id)` on the
adapter side allocates + copies the path bytes, which costs about as
much as the napi marshalling we eliminated. We moved the copy, we didn't
remove it.

What remains on the bridge path:

1. **`ArcStr::from(args.id)` per call** — one Arc-header alloc + path-byte
   copy. To eliminate it we'd need rolldown to hand the plugin an `ArcStr`
   for the id (currently `args.id: &str`). Out of scope.
2. **Box allocation for the `NativeStringHolder`** — small (~120 bytes)
   per call. Could be pooled via thread-local but the savings are
   microseconds.
3. **`ArcStr::from(code.as_str())` inside rolldown's transform driver** —
   one full source-bytes copy before every plugin call. Not in our control
   without changing rolldown's `Plugin` trait.
4. **`run_transform`'s internal allocations** — parse arena, AST nodes,
   codegen output String. These are unavoidable React Compiler work and
   happen identically in every variant.

The dominant cost on bridge-sync is not data copies; it's **sync TSFN
dispatch latency** — each module's transform is a blocking JS-thread
hop. `bridge-async`, `bridge-parallel`, and `native-lib` all win by
sidestepping that hop in different ways. Removing the small remaining
copies would shave ones-of-percent at best.

## Builtin panic investigation

When running `transform.reactCompiler` at the bundler level on Infisical's
frontend, **~17.7% of files** (684 of 3860) hit:

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
(so the comparison stays apples-to-apples). Over the full 3860-file corpus:
650 files panicked (16.8%) + 34 errored (0.9%) = **684 skipped (17.7%)**.
Errored files emit "untranspiled TypeScript syntax" / "untranspiled JSX
syntax" — the same underlying TS-lowering-didn't-run issue surfaced
differently.

**Proper upstream fix** would be either:
- Force TS lowering to happen before side-effects analysis when
  `transform.reactCompiler` is set, OR
- Have side-effects analysis tolerate TS declarations (treat them as no-ops).

## Secondary table — LIMIT=15, 6 iterations (1 warm-up dropped, 5 measured)

Async variants only run on the secondary table because they deadlock above
~16 concurrent in-flight transforms (the napi-rs `async fn` ↔ tokio
interaction this PoC has previously characterized).

```
skip-list: 684 files excluded (builtin-panic)
corpus: 15 files
iterations: 6 (1 warm-up dropped, 5 measured)
```

| Variant | min (ms) | median (ms) | mean (ms) | speedup vs utils-sync |
|---|---:|---:|---:|---:|
| utils-sync       | 10.65 | 10.86 | 10.91 | 1.00x |
| utils-async      |  6.16 |  7.07 |  6.92 | 1.54x |
| bridge-sync      |  9.36 |  9.52 | 10.04 | 1.14x |
| bridge-async     |  5.93 |  6.17 |  6.99 | 1.76x |
| native-lib       |  6.86 |  7.15 |  7.37 | 1.52x |
| **builtin**      |  **5.54** |  **5.90** |  **6.69** | **1.84x** |
| bridge-parallel  | 39.60 | 43.14 | 49.14 | 0.25x (worker-spawn overhead) |

At small scale `builtin` is still the fastest variant but the gap to the
async variants and `native-lib` is narrow. `bridge-parallel` loses badly
(~4x slower than baseline) because the ~8 JS worker threads' bootstrap
cost dwarfs the per-module transform cost when there are only 15 files.
Async dispatch (`utils-async`, `bridge-async`) is competitive here — both
beat their sync counterparts ~1.5–1.5x by overlapping multiple
transforms.

Compared to the earlier "approximate" numbers in this section (which
predated the diagnostic-conversion fairness fix and the panic-skip list),
the bridge-sync→utils-sync gap is much smaller now: 1.14x instead of 7.5x.
The original gap was an artifact of bridge variants discarding diagnostics
that utils-sync was forced to process through rolldown's transformSync
wrapper.

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
