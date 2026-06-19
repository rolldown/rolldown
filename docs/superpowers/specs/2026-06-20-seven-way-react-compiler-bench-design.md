# Seven-Way React Compiler Bench — Design

**Date:** 2026-06-20
**Status:** Draft / awaiting user review
**Author:** Claude + sapphi-red
**Supersedes (in part):** `docs/superpowers/specs/2026-06-18-native-bridge-plugin-design.md`

## Motivation

The earlier native-bridge PoC accumulated nine bench variants over time, several of which conflated more than one axis (the bridge's calling convention, the sync/async dispatch shape, JS-vs-native-vs-builtin location). The most recent runs landed with `string` and `native` nearly tied because the `MaybeAsyncJsCallback` union match cost — added to support `Promise<bigint>` returns — happened to be the same order of magnitude as the bridge's UTF-conversion savings on this workload. The story stopped being readable.

This spec defines a clean seven-way comparison, one variant per axis combination the user wants measured, and documents what we remove from the bench to keep the lineup honest.

## Goal

Bench seven implementations of the same React Compiler transform on the same Infisical corpus, varying only how the code crosses between rolldown and the transformer. Produce one primary table over the full corpus and one secondary table at a smaller corpus where the async variants don't hit the upstream deadlock.

Non-goals:
- Re-investigating the napi-rs async-fn ↔ tokio deadlock (documented; out of scope).
- Production hardening of the `transform_native_bridge*` fields.
- Removing the terminal `String` copy in `HookTransformOutput::code` (would need a `Plugin` trait change).
- A unified bridge field. We deliberately split into sync-only and async-only fields to isolate overhead; merging them back is right for a real ship but not for this measurement.

## Variant matrix

| # | Label | Implementation summary |
|---|---|---|
| 1 | `utils-sync` | JS plugin's `transform(code, id)` calls `transformSync(id, code, { reactCompiler: true })` from `rolldown/utils`. Two UTF round trips per module. |
| 2 | `utils-async` | JS plugin's `async transform(code, id)` awaits `transform(id, code, { reactCompiler: true })` from `rolldown/utils`. JS thread freed on dispatch. |
| 3 | `bridge-sync` | JS plugin's `transformNativeBridge(handle, id)` calls `BenchOxcTransformer.transformNative(handle, id)`. Sync bigint handle to `Box<NativeStringHolder>`. No UTF, no copy at bridge. Hook field is sync-only (`JsCallback<…, Option<i64>>`). |
| 4 | `bridge-async` | Same shape as (3) but uses a new sync-Promise hook field `transformNativeBridgeAsync(handle, id) => Promise<bigint>`. Calls `BenchOxcTransformer.transformNativeAsync`. |
| 5 | `native-lib` | dlopen'd cdylib (`bench_native_lib_plugin`) implementing `rolldown_native_plugin_abi`. `NativeStr { ptr, len }` view on the wire. Dispatched directly from rolldown's tokio worker threads via `NativeLibPlugin`. No napi, no JS thread. |
| 6 | `builtin` | No plugin. `BundlerOptions.transform.reactCompiler = true`. Rolldown runs React Compiler as part of its internal transform pipeline. |
| 7 | `bridge-parallel` | Variant (3)'s hook, but the JS plugin is registered via `defineParallelPlugin`. One `BenchOxcTransformer` per worker. Sync TSFN dispatch, parallelized across ~8 JS worker threads. |

## What changes from the current branch

**Removed from `run.mjs`** (variants that conflated axes or were superseded):
- `string` (calls `BenchOxcTransformer.transformStr` directly — not what a real user writes)
- `string-async` (same problem; only existed to corroborate the async deadlock)
- `native` (same hook as `bridge-sync` but currently shares the `MaybeAsyncJsCallback` field with `bridge-async`; the splitting work below makes (3) the clean replacement)
- `native-async` (superseded by `bridge-async`)
- `native-parallel` (replaced by `bridge-parallel` — same setup, clearer name)

**Kept** (no changes):
- `crates/rolldown_native_plugin_abi` — types crate
- `crates/bench_native_lib_plugin` — cdylib
- `crates/rolldown_binding/src/options/plugin/native_lib_plugin.rs` — dlopen loader
- The parallel-plugin worker `setInterval` keep-alive patch

**Added or modified:**

1. **Split the bridge hook into two fields** in `BindingPluginOptions`:
   ```rust
   /// Sync-only zero-copy bridge.
   pub transform_native_bridge:
     Option<JsCallback<FnArgs<(i64, String)>, Option<i64>>>,
   
   /// Sync-Promise zero-copy bridge. The JS callback MUST return a
   /// Promise<bigint>; sync returns are rejected at validation time.
   pub transform_native_bridge_async:
     Option<JsCallback<FnArgs<(i64, String)>, Promise<Option<i64>>>>,
   ```
   The async path is opt-in via the dedicated field rather than auto-detected via `Either<Promise, T>`. This removes the per-call `Either::A(Either::A(promise)) | Either::A(Either::B(ret))` match that has been costing us on the sync path.

2. **`JsPlugin::transform` dispatch order**: if `transform_native_bridge` is set, use it; else if `transform_native_bridge_async` is set, use it (await the Promise); else fall through to the existing `transform` path. Both bridge paths still drop the source `NativeStringHolder` after the call and own the result holder on return.

3. **`ParallelJsPlugin::transform`**: extend the existing OR to cover the new field too — `transform.is_some() || transform_native_bridge.is_some() || transform_native_bridge_async.is_some()`.

4. **JS-side wiring**:
   - `bindingify-plugin.ts`: pass `transformNativeBridgeAsync` through alongside `transformNativeBridge`.
   - `generated/hook-usage.ts`: union `HookUsageKind.transform` when either bridge field is set.
   - Bench and test plugin definitions cast through `as unknown as Plugin` to attach the experimental hooks — same pattern the current `transformNativeBridge` test uses. We don't widen the public `Plugin` type; the new fields stay experimental.

5. **`BenchOxcTransformer`**: keep `transformStr`, `transformStrAsync`, `transformNative`, `transformNativeAsync`. The bench just stops importing `transformStr`/`transformStrAsync` directly (variants 1 and 2 use `rolldown/utils` instead). They stay in the binding for future micro-benches.

6. **`bench_native_lib_plugin`**: no change.

7. **`scripts/bench/native-bridge-plugin/run.mjs`**: rewritten to define exactly the seven variants above. The `parallel-impl.mjs` worker file gets updated to match (still uses `transformNativeBridge` since that's the sync field the parallel workers want).

## Architecture (data flow per variant)

For a module with source `code` (an `ArcStr` in rolldown's module store):

- **Variant 1 (`utils-sync`)**:
  ```
  ArcStr → JsString (UTF-8 → UTF-16) → transformSync napi call → 
  Rust String (UTF-16 → UTF-8) → oxc parse+transform+codegen → 
  Rust String → JsString (UTF-8 → UTF-16) → return to plugin → 
  rolldown JsString (UTF-16 → UTF-8) → ArcStr
  ```
  ~4 UTF conversions per module.

- **Variant 2 (`utils-async`)**: same as 1 but the napi call is async; JS thread is freed during the await. Same ~4 conversions.

- **Variant 3 (`bridge-sync`)**:
  ```
  ArcStr → NativeStringHolder::from_arcstr (Arc clone) → i64 handle → 
  JS sees bigint → BenchOxcTransformer.transformNative(handle, id) → 
  read &str from Holder (zero copy) → oxc → new String → 
  NativeStringHolder::from_string → i64 handle → 
  JS returns bigint → reclaim Holder → into_string (move) → 
  Some(String) into HookTransformOutput
  ```
  Zero UTF conversions; one terminal copy into `HookTransformOutput::code` (unavoidable).

- **Variant 4 (`bridge-async`)**: same as 3 but the JS callback returns `Promise<bigint>` and the adapter awaits it. Implementation detail: the napi `async fn transform_native_async` already has a `yield_now().await` so it functions as a promise the napi side actually resolves asynchronously.

- **Variant 5 (`native-lib`)**:
  ```
  ArcStr → NativeStr { ptr, len } view → 
  extern "C" call into bench_native_lib_plugin.dylib → 
  oxc → new String → TransformOutput (plugin-owned buffer) → 
  host reads bytes → drop_output → String into HookTransformOutput
  ```
  No napi, no JS thread, zero UTF conversions. One terminal copy (the host's `to_owned()` into `HookTransformOutput::code` — different bytes from the plugin's owned String due to allocator boundary).

- **Variant 6 (`builtin`)**: rolldown's internal transform pipeline parses each module once and applies React Compiler in-line. No plugin call at all.

- **Variant 7 (`bridge-parallel`)**: variant 3's path, but `cb` lives on a worker-thread JS context picked by `WorkerManager`. Multiple variant-3 calls run in parallel across the 8-worker pool.

## Benchmark methodology

Two tables.

**Primary — full corpus (3847 files), 6 iterations (1 warm-up dropped).**
Variants: 1, 3, 5, 6, 7. (2 and 4 are async and deadlock at scale.)

```
node scripts/bench/native-bridge-plugin/run.mjs
```

Default `VARIANTS=utils-sync,bridge-sync,native-lib,builtin,bridge-parallel`.

**Secondary — `LIMIT=15`, 6 iterations.** All seven variants. The only fair head-to-head for the async variants.

```
LIMIT=15 ITERS=6 VARIANTS=utils-sync,utils-async,bridge-sync,bridge-async,native-lib,builtin,bridge-parallel \
  node scripts/bench/native-bridge-plugin/run.mjs
```

Both runs go in `scripts/bench/native-bridge-plugin/results.md`. The historical results section in that file is preserved as a "prior iterations" appendix.

## Success criteria

The PoC ships successfully when:

1. `pnpm build` (i.e. `just build-rolldown`) builds the binding with the new split fields compiled in.
2. `cargo test -p rolldown_binding --lib` continues to pass.
3. `just t-node-rolldown -- native-bridge` integration test continues to pass (with the test rewritten to exercise both `transformNativeBridge` and `transformNativeBridgeAsync`).
4. `node scripts/bench/native-bridge-plugin/run.mjs` runs the primary table to completion and writes `results.md`.
5. `LIMIT=15 … VARIANTS=…all seven…` runs the secondary table without hanging.

The PoC is **informative**, not pass/fail, on the measured ordering. We have predictions (see "Expected ordering" below) but the goal is publishable numbers, not a target.

## Expected ordering

Stated for posterity so we can compare against what comes back:

- **Primary table** (sync, full corpus): `builtin < bridge-parallel < native-lib ≲ bridge-sync ≤ utils-sync`. `builtin` wins because it skips one parse cycle (no plugin round-trip). `bridge-parallel` wins among plugin variants because it actually uses multiple OS threads. `native-lib` should beat `bridge-sync` because it skips napi entirely; `bridge-sync` should narrowly beat `utils-sync` due to the UTF conversions.
- **Secondary table** (LIMIT=15): mostly the same ordering except `utils-async` and `bridge-async` slot in close to `bridge-parallel` because async dispatch at small scale unlocks similar concurrency without the parallel-plugin spawn overhead.

If `bridge-sync` and `utils-sync` end up indistinguishable in the primary table the way they did before, that's a useful finding — the bridge layer's win is dwarfed by the per-module React Compiler cost on this workload, and a lighter transform would be needed to see it.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| The new `transform_native_bridge_async` field needs napi-rs to validate `Promise<T>` returns explicitly | Use `JsCallback<…, Promise<…>>` rather than `MaybeAsyncJsCallback`; the validation comes from `Promise<T>: FromNapiValue`. If napi-rs rejects sync returns at runtime with a confusing error, document it. |
| The `LIMIT=15` secondary table still triggers the async deadlock | Drop to `LIMIT=10` if needed. Document the threshold in results. |
| `transform_native_bridge_async`'s async hook ends up unused after we've measured the deadlock | The field stays; it's the only way to actually measure (4). Future work can collapse fields once the deadlock is fixed upstream. |
| `rolldown/utils`'s `transformSync` may include extra work compared to `BenchOxcTransformer.transformStr` (warnings, sourcemap collection, etc.) | Acceptable — variants 1 and 2 are *supposed* to measure the path real users hit. Both use the same `rolldown/utils` function so the (1)↔(2) comparison stays clean. |
| `builtin` is too fast to compare against because it skips parse | That's the point. Note it as "theoretical floor" in results, not as a target plugins should hit. |

## Implementation scope (what writing-plans will turn into tasks)

Roughly the following work, in dependency order:

1. Split `transform_native_bridge` into two fields on the Rust side; adjust `JsPlugin::transform` and `ParallelJsPlugin::transform` to dispatch in priority order.
2. Update `bindingify-plugin.ts` and `generated/hook-usage.ts` to surface the new field.
3. Rewrite the JS integration test to round-trip both fields.
4. Rewrite `run.mjs` to define exactly the seven variants. Add the `builtin` plugin (no plugin, just bundler option). Update `parallel-impl.mjs` to use the new sync field name explicitly.
5. Run primary and secondary benches in release. Capture into `results.md`, preserving the prior-iteration history as an appendix.

## Open questions deferred to implementation

- Whether `JsCallback<…, Promise<Option<i64>>>` is a valid napi-rs type — if not, we'll fall back to a custom wrapper that wraps `Promise<i64>` with a sentinel. Plan will note both shapes.
- Whether the `builtin` variant produces the same byte-for-byte output as the plugin variants. If not, that's a separate finding — both are running the same `oxc_react_compiler::default_plugin_options()`.
- Whether `setInterval(() => {}, 1 << 30)` in `parallel-plugin-worker.ts` ever fails to terminate cleanly. We haven't seen it but the test suite should confirm.
