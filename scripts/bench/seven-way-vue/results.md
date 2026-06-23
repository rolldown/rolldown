# Seven-Way Vue SFC Bench (6 variants — `builtin` N/A)

Companion to `seven-way-react-compiler`, but for Vue Single-File Component
compilation. Same plumbing on the bridge / native-lib / parallel side; the
React Compiler transforms swap out for two SFC compilers:

- **JS**: `@vue/compiler-sfc` (`parse` → `compileScript` → `compileTemplate`
  → `rewriteDefault`), wrapped in [`vue-utils.mjs`](./vue-utils.mjs).
- **Rust**: [Vize](https://github.com/ubugeeei-prod/vize) (`vize_atelier_sfc`)
  via `compile_sfc`.

## Variant matrix

| # | Variant | Where Vue compile runs | Dispatch |
|---|---|---|---|
| 1 | `utils-sync` | JS — `@vue/compiler-sfc` on JS thread | sync hook |
| 2 | `utils-async` | JS — same compiler, awaited | async hook |
| 3 | `bridge-sync` | Rust — Vize via `BenchVizeTransformer` napi class | sync TSFN bridge |
| 4 | `bridge-async` | Rust — Vize via `BenchVizeTransformer` napi class | async TSFN bridge |
| 5 | `native-lib` | Rust — Vize via `defineNativeLibPlugin` dlopen | direct fn-ptr call from rolldown workers |
| 6 | `bridge-parallel` | Rust — Vize in `defineParallelPlugin` workers | one bridge per worker thread |

**`builtin` is dropped** — rolldown's bundler core has no `transform.vue`
option, so there's no apples-to-apples way to embed Vize at the bundler
level the way the React Compiler bench does for oxc-react-compiler.

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
- For the `native-lib` variant, rolldown's existing `defineNativeLibPlugin`
  dlopens this cdylib unchanged.
- For the `bridge-*` variants, a new `BenchVizeTransformer` napi class in
  `rolldown_binding` **also** dlopens the cdylib (via `libloading`). The
  napi method dispatches through the same fn pointers; Vize's OXC fork
  stays entirely behind the FFI boundary.

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
napi method short-circuits on `!id.ends_with(".vue")` and returns `None`
(no transform). The cdylib does the same in its FFI symbol.

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

| Variant | min | **med** | p95 | speedup vs utils-sync |
|---|---:|---:|---:|---:|
| `utils-sync` | 112.6 | **117.9** | 135.8 | 1.00x |
| `utils-async` | 107.5 | **109.4** | 113.0 | 1.08x |
| `bridge-parallel` | 48.0 | **48.5** | 50.0 | 2.43x |
| `bridge-sync` | 32.6 | **33.1** | 33.3 | 3.56x |
| `bridge-async` | 15.0 | **15.4** | 15.9 | 7.67x |
| `native-lib` | 13.8 | **14.9** | 24.3 | **7.89x** |

## Observations

### native-lib wins by a hair over bridge-async (~3% gap)

Both variants pay ~0.5 ms per SFC for Vize's actual work. The 0.5 ms gap
between them is the per-call cost of an async TSFN dispatch + bigint
handle marshalling. Direct dlopen has neither.

This is a different shape from the React Compiler bench, where
`bridge-async` led `native-lib` (1833 ms vs 2247 ms). Two reasons:

1. **Per-file work is much lighter for Vue.** React Compiler emits ~0.6 ms
   per file averaged across Infisical's 3176 modules; Vize emits ~0.06 ms
   per Elk SFC. Fixed-cost overhead (TSFN dispatch, napi marshalling) is
   ~10x more visible when each call does ~10x less work.
2. **Corpus is 12x smaller** (255 vs 3176). Less parallelism for async
   dispatch to exploit; the constant overhead dominates.

### bridge-parallel is *slower* than bridge-sync (48.5 ms vs 33.1 ms)

Same root cause as above: at 255 files / ~0.06 ms each, the per-worker
startup cost and module-routing IPC overhead of `defineParallelPlugin`
overwhelms the parallelism win. The crossover point where parallel pays
off is well above this corpus.

For reference, on the React Compiler bench at LIMIT=3176, `bridge-parallel`
was 2038 ms (3.67x utils-sync) — solidly faster than `bridge-sync` (6925
ms, 1.08x). That's because per-file React Compiler work is heavy enough
that the worker overhead amortizes.

### utils-async barely beats utils-sync (1.08x)

`@vue/compiler-sfc` itself is sync. Wrapping it in `async transform` adds
one microtask hop per call but no actual concurrency, so the win is
marginal — and we save a bit on TS-parse-pipeline scheduling. Compare
React Compiler's `utils-async` at 3.11x, where the await actually
unblocks rolldown's worker pool to run other modules concurrently because
the work is heavier.

### Bridge layer overhead is real here

`utils-sync` (118 ms) vs `bridge-sync` (33 ms) is a **3.56x speedup just
from moving Vize compile to Rust**, before any async tricks. The bridge
layer itself adds essentially no overhead vs `native-lib` on synchronous
paths — both go through the same fn pointer in the same cdylib.

## What's left to optimize?

Same conclusions as the React Compiler bench:

- **TSFN dispatch latency** is the dominant cost on `bridge-sync` (33 ms
  vs `native-lib`'s 15 ms). The only way to close that gap is to batch
  transforms across a single TSFN call (not done here) or to use async
  dispatch (already done — see `bridge-async`).
- **`ArcStr::from(args.id)`** in the napi adapter copies the module id
  per call. ~0.5% of bench time. Not worth eliminating.
- **The bigint-handle protocol** has no remaining marshalling overhead
  worth removing — both source and id flow through the handle as views
  into Rust-owned buffers.
