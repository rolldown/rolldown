# Seven-Way Vue SFC Bench (8 variants — `builtin` N/A)

Companion to `seven-way-react-compiler`, but for Vue Single-File Component
compilation. Same plumbing on the bridge / native-lib / parallel side; the
React Compiler transforms swap out for two SFC compilers:

- **JS**: `@vue/compiler-sfc` (`parse` → `compileScript` → `compileTemplate`
  → `rewriteDefault`), wrapped in [`vue-utils.mjs`](./vue-utils.mjs).
- **Rust**: [Vize](https://github.com/ubugeeei-prod/vize) (`vize_atelier_sfc`)
  via `compile_sfc`.

## Variant matrix

| # | Variant | Compiler | Dispatch |
|---|---|---|---|
| 1 | `utils-sync-js` | **JS** — `@vue/compiler-sfc` | sync transform hook |
| 2 | `utils-async-js` | **JS** — `@vue/compiler-sfc`, awaited | async transform hook |
| 3 | `utils-sync` | **Rust** — Vize via `transformer.transformStr(code, id)` (napi JS strings) | sync transform hook |
| 4 | `utils-async` | **Rust** — Vize via `transformer.transformStrAsync` | async transform hook |
| 5 | `bridge-sync` | **Rust** — Vize via `transformer.transformNative(handle)` (bigint handle bridge) | sync TSFN bridge |
| 6 | `bridge-async` | **Rust** — Vize via `transformer.transformNativeAsync(handle)` | async TSFN bridge |
| 7 | `native-lib` | **Rust** — Vize via `defineNativeLibPlugin` dlopen | direct fn-ptr call from rolldown workers |
| 8 | `bridge-parallel` | **Rust** — Vize via `transformer.transformNative` in `defineParallelPlugin` workers | one bridge per worker thread |

**`builtin` is dropped** — rolldown's bundler core has no `transform.vue`
option, so there's no apples-to-apples way to embed Vize at the bundler
level the way the React Compiler bench does for oxc-react-compiler.

The variant set is now richer than the React Compiler bench in two ways:
both a JS *and* a Rust `utils-sync`/`utils-async` exist (whereas the React
bench only has the Rust version, because rolldown/utils ships React
Compiler natively). The pair lets you cleanly separate:

- **JS impl vs Rust impl** of the same compile: `utils-sync-js` vs `utils-sync`.
- **napi-string overhead vs handle bridge** of the same compiler: `utils-sync` vs `bridge-sync` (and `utils-async` vs `bridge-async`).

## Architecture notes

### Why a standalone cdylib

Vize's workspace pins OXC `=0.127.0` at a forked git rev
(`oxc-project/oxc#8265ed94...`); rolldown is on stable OXC 0.136.0. Adding
Vize as a workspace member would force duplicate trait impls for two OXC
versions in the same link graph. Instead:

- `scripts/bench/seven-way-vue/native/` is a **standalone Cargo workspace**.
  It depends on `vize_atelier_sfc` and exports the same C ABI symbols as
  `rolldown_native_plugin_abi` (`rolldown_native_plugin_abi_version`,
  `rolldown_native_plugin_transform`, `rolldown_native_plugin_drop_output`).
- For `native-lib`, rolldown's existing `defineNativeLibPlugin` dlopens
  this cdylib unchanged.
- For `utils-*` and `bridge-*`, a new `BenchVizeTransformer` napi class in
  `rolldown_binding` **also** dlopens the cdylib (via `libloading`). All
  three napi methods (`transformStr`, `transformStrAsync`, `transformNative`,
  `transformNativeAsync`) dispatch through the same fn pointers; Vize's
  OXC fork stays entirely behind the FFI boundary.

This means our rolldown_binding crate **does not depend on Vize at compile
time**, and the only Vize symbols that reach rolldown's process come
through `#[repr(C)]` types.

### Module type

`@vue/compiler-sfc.compileScript` doesn't strip TypeScript — that's
normally Vite's downstream esbuild step. We tell rolldown to parse the
SFC's transform output as `ts` via `moduleTypes: { '.vue': 'ts' }`. TS is
a superset of JS so Vize's already-TS-stripped output still parses fine
under the TS parser.

### `.vue`-only filter (bridge variants)

The bridge plugin's transform hook fires for **every** module — including
rolldown's internal runtime module that defines `__esm`, `__toESM`, etc.
If we passed those through Vize they'd be replaced with `export default {}`
(SFC parse fails) and the bundle would lose its runtime helpers. So the
napi `transform_native` method short-circuits on `!id.ends_with(".vue")`
and returns `None` (no transform). The cdylib does the same in its FFI
symbol. The `utils-*` JS plugins filter at the JS layer.

### Stub on Vize failure

A handful of Elk's SFCs trip Vize and a few trip `@vue/compiler-sfc` too.
Returning the original `.vue` source on failure doesn't work because
rolldown's TS parser can't read `<template>...</template>`. So on failure
all variants emit `export default {};\n` — a valid TS stub. Bench fairness
is preserved (same "give up" policy across variants).

## Numbers

**Corpus**: [`elk-zone/elk`](https://github.com/elk-zone/elk) at commit
`0b92391b` — 255 `.vue` files under `app/`, all `<script setup lang="ts">`.

**Hardware**: M4 Max, mimalloc, release builds for both rolldown and the
Vize cdylib. Iterations: 6 (1 warm-up dropped, 5 measured). Times below
are medians in ms.

| Variant | min | **med** | p95 | speedup vs `utils-sync-js` |
|---|---:|---:|---:|---:|
| `utils-sync-js` | 111.7 | **123.5** | 143.4 | 1.00x |
| `utils-async-js` | 106.0 | **107.5** | 108.4 | 1.15x |
| `bridge-parallel` | 48.5 | **50.1** | 60.9 | 2.47x |
| `utils-sync` (Vize via napi string) | 36.0 | **37.0** | 37.6 | 3.34x |
| `bridge-sync` (Vize via handle) | 32.9 | **33.2** | 33.5 | 3.72x |
| `utils-async` (Vize via async napi string) | 15.9 | **16.6** | 23.0 | 7.45x |
| `bridge-async` (Vize via async handle) | 14.9 | **15.4** | 32.7 | 8.04x |
| **`native-lib`** (Vize via dlopen) | 13.9 | **14.4** | 14.6 | **8.59x** |

## Observations

### JS compiler vs Rust compiler is the dominant lever (~3.3x)

`utils-sync-js` (123.5 ms) vs `utils-sync` (37.0 ms) is the cleanest A/B —
same sync transform hook, same dispatch path, just `@vue/compiler-sfc` vs
Vize underneath. Vize alone wins **3.34x** before any napi or dispatch
tricks.

### Bridge layer saves ~10% over plain napi strings (sync) and ~7% (async)

Comparing same-compiler / different-marshalling pairs at the bench median:

- `utils-sync` 37.0 ms vs `bridge-sync` 33.2 ms → bridge saves **3.8 ms / 10%**
- `utils-async` 16.6 ms vs `bridge-async` 15.4 ms → bridge saves **1.2 ms / 7%**

The bridge protocol (bigint handle wrapping pre-allocated Rust-owned
buffers, zero JS-string marshalling per call) is a measurable but small
win on top of an already-fast Rust compiler. Each napi-string crossing
costs ~7–15 µs amortized over 255 modules; that's noise next to the few
ms the Vue compile itself spends. The win would scale up on heavier
corpora.

### native-lib beats bridge-async by ~3% (15.4 → 14.4 ms)

Both pay ~0.5 ms per SFC for Vize's actual work. The remaining gap is the
per-call cost of an async TSFN dispatch + bigint handle Box allocation.
Direct dlopen has neither.

Different shape from the React Compiler bench, where `bridge-async` led
`native-lib` (1833 ms vs 2247 ms). Reasons:

1. **Per-file work is much lighter for Vue.** React Compiler emits ~0.6 ms
   per file averaged across Infisical's 3176 modules; Vize emits ~0.06 ms
   per Elk SFC. Fixed-cost overhead (TSFN dispatch, napi marshalling) is
   ~10x more visible when each call does ~10x less work.
2. **Corpus is 12x smaller** (255 vs 3176). Less parallelism for async
   dispatch to exploit; the constant overhead dominates.

### bridge-parallel is *slower* than bridge-sync (50.1 vs 33.2 ms)

Same root cause: at 255 files / ~0.06 ms each, the per-worker startup
cost and module-routing IPC overhead of `defineParallelPlugin` overwhelms
the parallelism win. The crossover point where parallel pays off is
above this corpus.

For reference, on the React Compiler bench at 3176 files,
`bridge-parallel` was 2038 ms (3.67x utils-sync) — solidly faster than
`bridge-sync` (6925 ms, 1.08x). That's because per-file React Compiler
work is heavy enough that the worker overhead amortizes.

### utils-async-js barely beats utils-sync-js (1.15x), utils-async beats utils-sync by 2.2x

When the underlying compile is fast (Vize, ~0.15 ms/file), the async hook
unblocks rolldown's worker pool to run other modules concurrently and you
get a real 2x+ speedup. When the underlying compile is slow
(`@vue/compiler-sfc`, ~0.5 ms/file), the JS thread is the bottleneck so
async doesn't help much.

The same pattern repeats in the React Compiler bench: `utils-async` was
3.11x `utils-sync` there, because the workload was heavy enough that
async scheduling helped.

## What's left to optimize?

- **TSFN dispatch latency** is the dominant cost on sync bridge paths
  (`bridge-sync` 33.2 ms vs `native-lib` 14.4 ms). The only way to close
  that gap is to batch transforms across a single TSFN call (not done
  here) or to use async dispatch (already done — see `bridge-async`).
- **napi-string marshalling** costs ~3.8 ms across 255 modules in sync
  mode and ~1.2 ms in async mode (`utils-*` minus `bridge-*`). Real but
  proportionally small; would matter more on heavier corpora.
- **`ArcStr::from(args.id)`** in the napi adapter copies the module id
  per call. ~0.5% of bench time. Not worth eliminating.
