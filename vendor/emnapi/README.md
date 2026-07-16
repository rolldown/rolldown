# TEMPORARY: vendored emnapi v2 WASI archives

This directory works around gaps in the published `emnapi@2.0.0-alpha.2`
package and must be deleted once a fixed emnapi v2 prerelease is published.

A sibling workaround lives in
`.yarn/patches/@emnapi-core-npm-2.0.0-alpha.2-*.patch` (wired through the
root `resolutions`): the published `@emnapi/core` captures typed-array /
DataView views over `wasmMemory.buffer` and reuses them across operations
that can grow the memory. Growing a **non-shared** wasm memory (the
single-threaded WASI builds) detaches the old buffer, so the stale views
crash (`Cannot perform Atomics.store/DataView.prototype.setUint32 on a
detached ArrayBuffer`). The patch re-creates the views at each use in
`dist/plugins/threadsafe-function.js` (`dispatch`, `enqueue`) and in
`dist/emnapi-core.js` (`napi_get_typedarray_info`, `napi_get_dataview_info`,
`napi_get_arraybuffer_info`, which may `malloc` through
`getViewPointer`/`getArrayBufferPointer` between view creation and use).
Upstream must apply the same fixes; drop the patch and the resolutions
entries together with this directory.

## What is vendored

Two static archives, built from the C sources shipped **inside the published
npm package itself** (`node_modules/emnapi/src`, source list of the
`emnapi_basic` target in `node_modules/emnapi/emnapi.gyp`) via
`vendor/emnapi/build.mjs`:

| Archive                                        | Why                                                                    |
| ---------------------------------------------- | ---------------------------------------------------------------------- |
| `wasm32-wasip1/libemnapi.a`                    | Missing from the published package (non-threaded WASI is unsupported). |
| `wasm32-wasip1-threads/libemnapi-napi-rs-mt.a` | Published build references the env cleanup hooks via the wrong module. |

**Both** archives deliberately omit `async_work.c` and
`threadsafe_function.c` (the extra sources of the full `emnapi` gyp target),
so `napi_call_threadsafe_function` & co. stay wasm **imports** that the
generated loaders resolve with the JavaScript implementations from
`@emnapi/core/plugins` — the emnapi v1 `libemnapi-basic(-mt).a` model that
upstream napi-rs `main` links (`emnapi-basic-mt`):

- without threads the C implementations are unconditional
  `napi_generic_failure` stubs;
- with threads the linker would extract the C implementations (the Rust side
  references `napi_create_threadsafe_function` etc.), silently shadowing the
  `@emnapi/core` v2 threaded TSFN protocol (`plugins/threadsafe-function.js`,
  its `NapiTSFNOffset32` struct layout and tsfn-send worker messaging) that
  the loaders and the browser TSFN instrumentation
  (`examples/napi/wasi-worker-browser.mjs`) are written against.

The threaded archive additionally contains
`src/thread/async_worker_create.c` + `src/thread/async_worker_init.S` (the
`emnapi_basic` gyp target's `wasm_threads` condition): the bootstrap for the
async-worker pthreads the JS `asyncWork` plugin spawns, exported by
`crates/build/src/wasi.rs` via `--export-if-defined`.

`vendor/emnapi/install.mjs` verifies and copies the archives into
`node_modules/emnapi/lib`. It runs from the repository `postinstall` hook and
from the CI steps that build WASI targets (CI installs with
`--mode=skip-build`, which skips `postinstall`).

## Integrity verification

`build.mjs` records `manifest.json` at generation time; `install.mjs`
re-verifies it on every machine (including every CI lane that consumes the
archives) before copying anything:

- the installed `emnapi` package version equals the pinned
  `2.0.0-alpha.2`,
- every npm-shipped file the archives can be built from (`emnapi.gyp`,
  `src/**`, `include/**`) still matches its recorded sha512 — catches source
  drift or a republished tarball,
- each vendored archive matches its recorded sha512 and `ar` member list —
  catches stale or locally modified blobs relative to the recorded
  generation run.

The **semantic** property the archives exist for (the `env`/`napi`
import-module split of the cleanup hooks) is verified functionally in CI:
the `Check minimal cleanup-hook imports` step links a fresh wasm against the
installed archive and asserts its import section, and the native-lane CLI
test `native builds preserve target-specific WASI exports and declarations`
performs a real non-threaded build+link.

CI does **not** rebuild the archives byte-for-byte: wasi-sdk is not
provisioned on the consuming lanes (the whole native matrix, including
Windows and macOS), and clang output is only reproducible against the exact
wasi-sdk recorded in `manifest.json` (`32.0`, llvm 22.1.0 — a local rebuild
with that SDK reproduces the committed archives bit-for-bit). Residual risk:
a commit that regenerates the archives _and_ the manifest together is only
caught by code review of that commit, like any other committed binary — the
manifest makes such a change loud (source hashes, member lists and archive
hashes all move together) and `build.mjs` documents the exact reproduction
command.

## Import-module conventions (why the published archive is wrong)

- `crates/sys/src/lib.rs` declares every `napi_*` function in plain
  `extern "C"` blocks on wasm, i.e. the **default `env` import module**.
- The single exception: `napi_add_env_cleanup_hook` /
  `napi_remove_env_cleanup_hook` are imported through the **`napi` module**
  (`#[link(wasm_import_module = "napi")]` in `crates/napi/src/lib.rs`, since
  #2399).
- The emnapi C archive must follow the same convention, otherwise the final
  wasm either fails to link (`import module mismatch`, when the archive uses
  the `napi` module for symbols Rust imports via `env` — this is what the
  plain `libemnapi-mt.a` does) or ends up with duplicate
  `env.napi_*_env_cleanup_hook` **and** `napi.napi_*_env_cleanup_hook`
  imports (what the published `libemnapi-napi-rs-mt.a` produces, rejected by
  `examples/napi/wasi-cleanup-hook-link/check-imports.mjs`).

## What upstream emnapi must publish to remove this directory

A prerelease > `2.0.0-alpha.2` whose package ships:

1. `lib/wasm32-wasip1/libemnapi.a` — the `emnapi_basic` gyp target compiled
   with `--target=wasm32-wasip1` (no threads), `napi_*` references through
   the default `env` import module **except** `napi_add_env_cleanup_hook`
   and `napi_remove_env_cleanup_hook`, which must use
   `__attribute__((__import_module__("napi")))`. It must NOT contain
   `async_work.c` / `threadsafe_function.c` — the JavaScript plugin
   implementations from `@emnapi/core/plugins` must be resolvable as
   imports.
2. `lib/wasm32-wasip1-threads/libemnapi-napi-rs-mt.a` — same convention,
   compiled with `--target=wasm32-wasip1-threads -pthread`, plus the
   `emnapi_basic` thread sources `src/thread/async_worker_create.c` and
   `src/thread/async_worker_init.S` (like the published v1
   `libemnapi-basic-napi-rs-mt.a`). It must also NOT contain `async_work.c`
   / `threadsafe_function.c`: the C implementations would be extracted by
   the linker and silently replace the `@emnapi/core` v2 plugin TSFN /
   async-work protocol the loaders provide.

Then:

- delete `vendor/emnapi`,
- drop the `node vendor/emnapi/install.mjs` calls from `package.json`
  (`postinstall`) and `.github/workflows/test-release.yaml`,
- bump the `emnapi`, `@emnapi/core` and `@emnapi/runtime` versions together
  (the CLI enforces that the three versions match, see `setWasiEnv` in
  `cli/src/api/build.ts`).

`vendor/emnapi/install.mjs` and `vendor/emnapi/build.mjs` hard-fail when the
installed emnapi version is not `2.0.0-alpha.2`, so a version bump without
this cleanup breaks loudly instead of silently shipping stale archives.
