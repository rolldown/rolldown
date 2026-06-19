# Seven-Way React Compiler Bench — Design

**Date:** 2026-06-20
**Status:** Draft / awaiting user review
**Author:** Claude + sapphi-red
**Branch base:** `bea835c94` (clean checkout of main)
**Prior exploration:** kept on `feat/native-bridge-plugin-poc-prior` for reference, but **not** carried into the implementation. This spec is the canonical description; everything below is built from scratch.

## Motivation

We want to measure where the actual cost lives when a JS plugin in rolldown applies a Rust-backed transform (concretely: React Compiler via oxc-transformer). Earlier exploration on this branch's prior incarnation accumulated overlapping bench variants that conflated multiple axes (the calling convention vs the sync/async dispatch shape vs JS-vs-native-vs-builtin location), which made the story unreadable. This spec defines a clean seven-way comparison — one variant per axis combination — and a benchmark harness that exercises each on the same Infisical corpus.

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

## Implementation surface (built from scratch on top of main)

The branch is reset to `bea835c94` and contains only this design doc. Every piece below is new.

### Crates

1. **`crates/rolldown_native_plugin_abi`** — types-only crate. Defines the C ABI:
   - `#[repr(C)] struct NativeStr { *const u8, usize }`
   - `#[repr(C)] struct TransformOutput { code: NativeStr, error: NativeStr, plugin_data: *mut c_void }`
   - `pub const ABI_VERSION: u32 = 1`
   - Function-pointer typedefs and symbol-name string constants for `abi_version`, `transform`, `drop_output`.

2. **`crates/bench_native_lib_plugin`** — `cdylib` that exports the three required symbols and runs the same `oxc::Parser` → `semantic_builder_for_transform` → `Transformer{ react_compiler: Some(default_plugin_options()) }` → `Codegen` pipeline as the rolldown-binding bench transformer.

### Inside `crates/rolldown_binding`

3. **`src/native_bridge.rs`** — `NativeStrRef`, `NativeStringHolder` (enum-inner: `ArcStr` or `String`), `into_raw_handle` / `from_raw_handle` / `handle_as_str` API used by the bridge variants.

4. **`src/bench_oxc_transformer.rs`** — `#[napi] BenchOxcTransformer` class with four methods (`transformStr` sync, `transformStrAsync` async, `transformNative` sync handle, `transformNativeAsync` async handle returning `Promise<bigint>`).

5. **Two new fields on `BindingPluginOptions`** (split deliberately so the sync path doesn't pay the `MaybeAsyncJsCallback` cost):
   ```rust
   /// Sync-only zero-copy bridge.
   pub transform_native_bridge:
     Option<JsCallback<FnArgs<(i64, String)>, Option<i64>>>,

   /// Sync-Promise zero-copy bridge. JS callback MUST return Promise<bigint>;
   /// sync returns are rejected at napi validation time.
   pub transform_native_bridge_async:
     Option<JsCallback<FnArgs<(i64, String)>, Promise<Option<i64>>>>,
   ```

6. **`JsPlugin::transform` dispatch order**: if `transform_native_bridge` is set, run the sync bridge; else if `transform_native_bridge_async` is set, await its Promise; else fall through to the existing `transform` path. Both bridge paths construct a `NativeStringHolder::from_arcstr(args.code.clone())`, leak it to an i64, drop the holder after the call, and reconstitute the result holder's String into `HookTransformOutput::code`.

7. **`ParallelJsPlugin::transform`**: dispatch when any of `transform`, `transform_native_bridge`, or `transform_native_bridge_async` is set on the per-worker `JsPlugin`.

8. **`Plugin` variant chain** widens to `Either3<BindingPluginOptions, BindingNativeLibPlugin, BindingBuiltinPlugin>`. `BindingNativeLibPlugin` is a `#[napi_derive::napi(object)]` `{ name, path }` whose `TryInto<NativeLibPlugin>` opens the `.dylib` via `libloading`, version-checks, and stashes the three resolved fn pointers behind an `Arc<Library>`.

9. **`src/options/plugin/native_lib_plugin.rs`** — `NativeLibPlugin` implementing `Plugin`. The transform impl builds two `NativeStr` views (source from `args.code`, id from `args.id`), calls the plugin's `transform` fn pointer with a stack `TransformOutput`, copies the result code into a Rust `String` (one terminal copy across the binary boundary), then calls the plugin's `drop_output`.

### Inside `packages/rolldown`

10. **`src/plugin/native-lib-plugin.ts`** — `defineNativeLibPlugin({ name, path })` returning `{ _nativeLib: ... }` marker.

11. **`src/plugin/index.ts`** — `RolldownPlugin` union extends to include `NativeLibPlugin`.

12. **`src/experimental-index.ts`** — re-export `defineNativeLibPlugin`.

13. **`src/utils/bindingify-input-options.ts`** — recognize `_nativeLib` and emit a `BindingNativeLibPlugin` directly.

14. **`src/plugin/bindingify-plugin.ts`** — pass `transformNativeBridge` and `transformNativeBridgeAsync` through to the binding plugin options.

15. **`src/plugin/generated/hook-usage.ts`** — union `HookUsageKind.transform` when either bridge field is set on a plugin.

16. **`src/parallel-plugin-worker.ts`** — drop `parentPort.unref()` in the success path; add a long-interval `setInterval` so the worker's JS event loop stays alive after bootstrap. (Required on Node 24.11 to prevent `Status::Closing` on the first dispatch from the main thread; also reproduces in rolldown's own `parallel-noop-plugin` example without this patch.)

### Bench harness

17. **`scripts/bench/seven-way-react-compiler/`** — new directory.
    - `.gitignore` for `.fixture/`, `corpus.json`, `out-*/`, etc.
    - `setup.mjs` — sparse-clones `Infisical/infisical`'s `frontend/` and writes `corpus.json` (filters out `.d.ts`).
    - `parallel-impl.mjs` — `defineParallelPluginImplementation` returning a plugin with the sync `transformNativeBridge` hook for variant 7.
    - `run.mjs` — defines exactly the seven variants; primary table dispatches a sync set, secondary table at `LIMIT=15` covers all seven.
    - `results.md` — populated after the bench runs.

### Tests

18. **`packages/rolldown/tests/native-bridge-plugin.test.ts`** — round-trip integration test that exercises both `transformNativeBridge` and `transformNativeBridgeAsync` and asserts the output matches the `rolldown/utils.transformSync` baseline for the same input.

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
Variants: 1, 3, 5, 6, 7. (2 and 4 are async; the prior exploration documented they deadlock above ~16 concurrent in-flight transforms — a generic napi-rs 3.x `async fn` ↔ `MaybeAsyncJsCallback` ↔ tokio interaction, not specific to the bridge code.)

```
node scripts/bench/seven-way-react-compiler/run.mjs
```

Default `VARIANTS=utils-sync,bridge-sync,native-lib,builtin,bridge-parallel`.

**Secondary — `LIMIT=15`, 6 iterations.** All seven variants. The only fair head-to-head for the async variants.

```
LIMIT=15 ITERS=6 VARIANTS=utils-sync,utils-async,bridge-sync,bridge-async,native-lib,builtin,bridge-parallel \
  node scripts/bench/seven-way-react-compiler/run.mjs
```

Both runs go in `scripts/bench/seven-way-react-compiler/results.md`.

## Success criteria

The PoC ships successfully when:

1. `just build-rolldown` builds the binding with the new ABI types crate, the bench cdylib, the split bridge fields, and the `NativeLibPlugin` loader compiled in.
2. `cargo test -p rolldown_binding --lib` passes (unit tests for `NativeStringHolder` and `BenchOxcTransformer::run_transform`).
3. `just t-node-rolldown -- native-bridge` passes a fresh integration test that round-trips both `transformNativeBridge` (sync) and `transformNativeBridgeAsync` (async) and asserts both match `rolldown/utils.transformSync` for the same input.
4. `node scripts/bench/seven-way-react-compiler/run.mjs` runs the primary table (5 sync variants) to completion on the full Infisical corpus and writes `results.md`.
5. `LIMIT=15 … VARIANTS=…all seven…` runs the secondary table covering all seven variants without hanging.

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

1. Create `rolldown_native_plugin_abi` types crate (item 1 above).
2. Create `bench_native_lib_plugin` cdylib (item 2).
3. Create `native_bridge.rs` + unit tests, then `bench_oxc_transformer.rs` with the four `#[napi]` methods (items 3, 4).
4. Add the two split bridge fields on `BindingPluginOptions` and wire `JsPlugin::transform` to dispatch in priority order (items 5, 6).
5. Add `BindingNativeLibPlugin` napi-derive object and `NativeLibPlugin` loader; widen the plugin variant chain to `Either3` (items 8, 9).
6. Extend `ParallelJsPlugin::transform` to dispatch when either bridge field is set (item 7).
7. Patch `parallel-plugin-worker.ts` keep-alive (item 16).
8. Wire the JS surface: `defineNativeLibPlugin`, `RolldownPlugin` union, experimental re-export, `bindingify-input-options` recognition of `_nativeLib`, `bindingify-plugin` pass-through of both bridge fields, `hook-usage.ts` update (items 10-15).
9. Write the JS integration test that round-trips both bridge variants and asserts equivalence with `rolldown/utils.transformSync` (item 18).
10. Build out `scripts/bench/seven-way-react-compiler/` — fixture setup, parallel-plugin impl, runner, results template (item 17).
11. Run primary and secondary benches in release. Fill in `results.md`.

## Open questions deferred to implementation

- Whether `JsCallback<…, Promise<Option<i64>>>` is a valid napi-rs 3.x type — if `Promise<T>` here works only with `MaybeAsyncJsCallback`, fall back to wrapping the i64 in a `Promise<i64>` with a sentinel (e.g. negative-zero or i64::MIN) for "skip this transform" and document the choice in the plan.
- Whether the `builtin` variant produces byte-for-byte identical output to the plugin variants (both run `oxc_react_compiler::default_plugin_options()`). The integration test asserts equivalence between `bridge-sync` and `rolldown/utils.transformSync`; `builtin` is harder to assert against without a separate bundle-output diff. Acceptable for a measurement spec.
- Whether `setInterval(() => {}, 1 << 30)` in `parallel-plugin-worker.ts` ever fails to terminate cleanly. The prior exploration didn't see it, but the test suite should confirm; if it does fail, the right fix is upstream (have the TSFNs that wrap each plugin hook keep the worker's JS thread referenced).
