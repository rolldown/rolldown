# rolldown on s390x / Big-Endian — Compatibility Analysis

**Status:** ⚠️ Compatible **after a one-line-class fix** to a third-party dependency (`json-escape-simd`). Unpatched, rolldown miscompiles JSON string escaping (sourcemaps, ESM import paths, asset/data-URLs) on big-endian.
**rolldown version evaluated:** `1.1.3`
**Key dependencies:** `oxc_sourcemap 8.0.2`, `json-escape-simd 3.0.2`, `oxc 0.137.0`
**Date:** 2026-06-26
**Platform under test:** RHEL on native IBM Z (`s390x-unknown-linux-gnu`), big-endian, 64-bit (`uname -m` → `s390x`)
**Toolchain:** Rust `1.96.0` (pinned via `rust-toolchain.toml`)

---

## 1. Executive Summary

rolldown's Rust core **compiles cleanly and natively on s390x**, and rolldown publishes a
prebuilt s390x native binding (`@rolldown/binding-linux-s390x-gnu`). However, running the Rust
test suite on big-endian hardware reveals a **real correctness bug**: JSON string escaping is
broken on big-endian.

The bug is **not in rolldown's own code**. It lives in the third-party crate
[`json-escape-simd` 3.0.2](https://github.com/napi-rs/json-escape-simd), whose **portable SIMD
fallback** (the code path used on targets without SSE2/AVX/NEON, i.e. s390x) constructs its lane
bitmask with a bit ordering that is inconsistent with how the first-escape offset is read back.
On big-endian this produces integer underflow and corrupted escaping.

It reaches rolldown through **two independent paths**, so it must be fixed once, centrally:

1. **Directly** — rolldown calls `json_escape_simd::escape` in
   [`esm.rs`](crates/rolldown/src/ecmascript/format/esm.rs) and
   [`parse_to_ecma_ast.rs`](crates/rolldown/src/utils/parse_to_ecma_ast.rs).
2. **Transitively** — `oxc_sourcemap 8.0.2` (used by `rolldown_sourcemap`'s `to_json_string`)
   **also depends on `json-escape-simd`** and uses it to serialize sourcemap JSON. This is what
   makes the `render_ecma_module` sourcemap tests panic.

### Recommendation for downstream / consuming repos

| If your rolldown usage involves… | s390x guidance |
|---|---|
| Bundling **without** sourcemaps, and import paths with only plain ASCII | Largely unaffected, but **apply the fix** — escaping of any `"`, `\`, or control byte is wrong. |
| **Sourcemaps** (`output.sourcemap`) | ⚠️ **Broken until patched** — `sourcesContent`/`sources` JSON is corrupted. |
| Import paths / data-URLs / assets containing **non-ASCII, quotes, backslashes, control chars** | ⚠️ **Broken until patched.** |
| Any of the above, **with the vendored fix in this repo applied** | ✅ Correct on s390x. |

> **Severity note:** the failing unit tests panic only because of `debug_assert!`/overflow checks
> in debug builds. In **release** builds (e.g. the published s390x binding) the same code path
> wraps the subtraction instead of panicking, yielding **silent output corruption** — which is
> worse, because there is no crash to signal it.

---

## 2. Why s390x Is a Meaningful Test (Big-Endian)

`s390x` (IBM Z) is the canonical **big-endian, 64-bit** target. The classic things that break a
high-performance bundler on big-endian are byte-order assumptions in code that reinterprets bytes
as integers — bit-packing, SIMD-style byte scanning, hashing. rolldown itself is careful here;
the failure is in a vendored SIMD JSON-escaper whose big-endian branch was written but never
actually tested on big-endian hardware.

rolldown ships s390x as a release target ([`packages/rolldown/package.json`](packages/rolldown/package.json),
[`.github/workflows/reusable-release-build.yml`](.github/workflows/reusable-release-build.yml))
but its CI **only cross-builds** s390x — it never **runs the test suite** on big-endian. That is
exactly why this bug shipped undetected: a successful cross-compile does not exercise runtime
byte-order behavior.

---

## 3. Methodology & Reproduction

All steps were run **natively on s390x** (no QEMU / `cross` emulation).

### 3.1 Confirm the platform

```bash
uname -m                 # → s390x
rustc -vV | grep host    # → host: s390x-unknown-linux-gnu
```

### 3.2 Build (Rust core — no Node toolchain required)

```bash
export CARGO_HOME=/data/.cargo            # keep cargo off a full root fs, if needed
cargo build --target s390x-unknown-linux-gnu -p rolldown
```

Result: **clean native build**, no errors. Every `target_endian`-relevant path compiles.

### 3.3 Run the Rust test suite — the definitive big-endian check

```bash
cargo test --workspace --exclude rolldown_binding
```

This needs **none** of the JS toolchain and is the high-signal compatibility verdict.

### 3.4 (Optional) JS / binding integration tests

These require building the native binding and a working Node toolchain. On s390x note that some
**dev/test tooling has no s390x build** and is unrelated to rolldown:

- `workerd` → `Unsupported platform: linux s390x BE` (Cloudflare Workers runtime; Vite/worker tests)
- `playwright-chromium` → no s390x browser build (browser tests)

Install deps skipping those postinstall scripts, then build + test:

```bash
pnpm install --frozen-lockfile --ignore-scripts
export npm_config_verify_deps_before_run=false
pnpm --filter rolldown run build-native:debug
pnpm --filter rolldown-tests run test:main
```

---

## 4. Results

### 4.1 Build

Clean native build of the `rolldown` crate and all `rolldown_*` crates. No errors.

### 4.2 Rust unit/integration tests — 3 failures, all the same root cause

```
test utils::render_ecma_module::tests::empty_chain_returns_codegen_map_unchanged ... FAILED
test utils::render_ecma_module::tests::null_only_with_codegen_map_swaps_source_content ... FAILED
test utils::render_ecma_module::tests::real_map_is_not_replaced_by_placeholder ... FAILED

test result: FAILED. 66 passed; 3 failed; 0 ignored
```

Panics (debug build):

```
panicked at json-escape-simd-3.0.2/src/simd/v128.rs:187:
attempt to subtract with overflow

panicked at json-escape-simd-3.0.2/src/simd/util.rs:9:
char is s, cnt is 0,  NEED_ESCAPED is 0
```

All three tests assert `result.to_json_string()` (sourcemap JSON), which routes through
`rolldown_sourcemap` → `oxc_sourcemap` → `json_escape_simd::escape`.

---

## 5. Root Cause (pinned)

On s390x there is no SSE2/AVX/NEON, so `json-escape-simd` uses its **portable `v128`** path
([`lib.rs:317-319`](vendor/json-escape-simd/src/lib.rs)). Its big-endian support is internally
inconsistent:

- `Mask128::bitmask()` ([`v128.rs`](vendor/json-escape-simd/src/simd/v128.rs)) had a big-endian
  branch placing lane `i` at bit `15 - i`:
  ```rust
  #[cfg(target_endian = "big")]
  { self.0.iter().enumerate().fold(0, |acc, (i, &b)| acc | ((b as u16) << (15 - i))) }
  ```
- `BitMask::first_offset()` ([`bits.rs`](vendor/json-escape-simd/src/simd/bits.rs)) reads the
  **lowest** set bit via `self.as_little_endian().trailing_zeros()`, where `as_little_endian()`
  on big-endian did `swap_bytes()`.

These two do not compose. Trace "first byte needs escaping":

| step | value |
|---|---|
| big-endian `bitmask()` (lane 0 → bit 15) | `0x8000` |
| `as_little_endian()` = `swap_bytes(0x8000)` | `0x0080` |
| `trailing_zeros(0x0080)` → `first_offset` | **7** (should be **0**) |

The bogus offset `7` then drives `nb -= cn` to **underflow** (`v128.rs:187`,
"attempt to subtract with overflow"), and advances the source pointer onto a non-escape byte,
tripping the `cnt != 0` assert in `escape_unchecked` (`util.rs:9`). The partial-tail mask
`mask &= (1u16 << nb) - 1` (`v128.rs:180`) is wrong on big-endian for the same reason (it keeps
low bits while the big-endian layout put valid lanes in high bits).

The portable `Mask128` is a `[u8; 16]` array indexed by lane — it is a **logical** bitmask, with
no actual dependence on hardware byte order. The endian-specific branches were a misguided
attempt at big-endian support and are simply incorrect.

### Where it surfaces in rolldown

`json_escape_simd::escape` (wraps a string in `"…"` and JSON-escapes it) is used for:

- **Sourcemap JSON** — via `oxc_sourcemap` (`rolldown_sourcemap::to_json_string`); the failing tests.
- **ESM import-path emission** — [`esm.rs:341,346,394`](crates/rolldown/src/ecmascript/format/esm.rs#L341).
- **Asset / data-URL escaping** — [`parse_to_ecma_ast.rs:149,160,166`](crates/rolldown/src/utils/parse_to_ecma_ast.rs#L149).

Any input containing escape-worthy bytes (`"`, `\`, `0x00–0x1f`, or sitting near a 16-byte chunk
boundary with such bytes) is mis-escaped on big-endian.

---

## 6. The Fix (applied in this repo)

Because the crate is reached both directly and through `oxc_sourcemap`, the fix is applied **once**
via `[patch.crates-io]` so it covers the entire dependency graph.

- **Vendored** `json-escape-simd 3.0.2` at [`vendor/json-escape-simd/`](vendor/json-escape-simd/)
  with the portable big-endian path corrected:
  - `Mask128::bitmask()` now builds `lane i → bit i` **unconditionally** (no endian branch).
  - `BitMask::as_little_endian()` is now the identity (no `swap_bytes`) — the masks are logical
    values built either by `Mask128::bitmask` or by x86-only movemask intrinsics (always
    little-endian), so swapping was wrong on the portable path and a no-op on x86.
- **Wired in** via the workspace root [`Cargo.toml`](Cargo.toml):
  ```toml
  [workspace]
  exclude = ["vendor"]

  [patch.crates-io]
  json-escape-simd = { path = "vendor/json-escape-simd" }
  ```

The change is a **no-op on little-endian** (that branch was already `<< i` with an identity
`as_little_endian`), so x86/arm behavior and SIMD performance are unchanged. On big-endian the
path now uses the same logic that is proven correct on little-endian.

### Verification

- **Little-endian (developer machine, `aarch64-apple-darwin`):** with the patch applied,
  `cargo test -p rolldown --lib utils::render_ecma_module` → **5 passed, 0 failed** (no regression).
- **Big-endian (s390x):** rerun the suite that previously failed:
  ```bash
  cargo test --workspace --exclude rolldown_binding
  # expect: the three utils::render_ecma_module::tests::* now pass
  ```

---

## 7. Recommended Next Steps

1. **File an upstream issue/PR on `json-escape-simd`** (napi-rs/json-escape-simd): the portable
   `v128` big-endian path is incorrect; the consistent-and-correct fix is to drop the endian
   branch in `Mask128::bitmask` and the `swap_bytes` in `BitMask::as_little_endian`. (`NeonBits`,
   aarch64-only, is a genuinely different case and should **not** be changed.)
2. **Mirror the patch into `oxc_sourcemap`'s** dependency once the upstream release lands, then
   drop the local `[patch.crates-io]`.
3. **Add an s390x test job to rolldown CI** (via `cross`/QEMU or native Z) so big-endian
   regressions are caught — today CI only cross-**builds** s390x, which is why this shipped.

---

## 8. Appendix — Environment & Notes

- **Hardware:** IBM Z, big-endian, 64-bit (`uname -m` → `s390x`).
- **Rust:** `1.96.0` (pinned, `rust-toolchain.toml`).
- **Filesystem note:** on the test host the root fs was full; cargo/node/pnpm caches were
  redirected to a data volume (`CARGO_HOME`, `npm_config_cache`, `PNPM_HOME`, `HOME`, `TMPDIR`).
- **Node toolchain note:** `@oxc-node/core` **does** ship `linux-s390x-gnu` and loads fine; the
  only Node-side blockers are `workerd` and `playwright-chromium`, which have no s390x builds and
  are unrelated to rolldown's runtime.

### Endian-relevant code locations (reference)

| Area | Location |
|---|---|
| Portable SIMD bitmask (patched) | [`vendor/json-escape-simd/src/simd/v128.rs`](vendor/json-escape-simd/src/simd/v128.rs) |
| Bitmask `first_offset`/`as_little_endian` (patched) | [`vendor/json-escape-simd/src/simd/bits.rs`](vendor/json-escape-simd/src/simd/bits.rs) |
| rolldown direct escape use | [`esm.rs`](crates/rolldown/src/ecmascript/format/esm.rs#L341), [`parse_to_ecma_ast.rs`](crates/rolldown/src/utils/parse_to_ecma_ast.rs#L149) |
| Transitive use (sourcemap JSON) | `rolldown_sourcemap::to_json_string` → `oxc_sourcemap 8.0.2` → `json-escape-simd` |
| Failing tests | [`render_ecma_module.rs`](crates/rolldown/src/utils/render_ecma_module.rs) |
| `[patch.crates-io]` wiring | [`Cargo.toml`](Cargo.toml) |

---

## 9. s390x Build & Test Playbook (reproducible)

Everything below was learned building and testing **natively on RHEL/IBM Z**. None of it is
needed to *use* a correctly-built rolldown; it's the recipe for building/verifying from source on
s390x, where the dev toolchain is unpaved (rolldown CI only **cross-builds** s390x).

> **TL;DR of what actually matters:** the Rust suite is the authoritative compatibility check and
> needs only Rust (Section 9.3). Everything Node-related (9.5–9.7) is optional and fights
> dev-tooling, not rolldown.

### 9.0 Quick verdict matrix (what works vs what's blocked)

| Layer | Status on native s390x | Notes |
|---|---|---|
| `cargo build` (core crate + workspace) | ✅ works | native, no special flags |
| `cargo test --workspace --exclude rolldown_binding` | ✅ **1767 pass** with the fix | the authoritative check |
| `cargo build -p rolldown_binding` (napi cdylib) | ✅ works | native gcc; **no** `--target` |
| napi build via `oxnode`/`@oxc-node/core` executing `build-binding.ts` | ⚠️ flaky/crashes | use the cross-linker symlink **or** the cargo-direct route (9.5) |
| `dist` build (`build.ts` js-glue) | ❌ blocked | `@oxc-node/core` crashes when *executing* TS on s390x (loads fine, runs not) |
| `pnpm install` postinstall for `workerd`, `playwright-chromium` | ❌ no s390x build | unrelated to rolldown; `--ignore-scripts` |
| vitest e2e (`test:main`) | ❌ blocked | vite 8 / vitest 4 config loader; esbuild itself is fine |
| esbuild on s390x | ✅ works | `@esbuild/linux-s390x` present and functional |

### 9.1 Prerequisites

```bash
uname -m                 # → s390x
rustc -vV | grep host    # → host: s390x-unknown-linux-gnu  (toolchain 1.96.0, pinned)
# RHEL build deps:
dnf groupinstall -y "Development Tools"
dnf install -y gcc gcc-c++ make git pkgconf-pkg-config
```

### 9.2 Environment setup — keep everything off a full root fs

On the reference host `/` was 100% full while `/data` had space. **All** caches/temp must be
redirected, or installs fail with `ENOSPC` (corepack, npm, cargo, pnpm all default to `$HOME` /
`/tmp` on root). Re-export these in **every** shell (they don't persist):

```bash
export HOME=/data/roothome        # moves ~/.cache, ~/.local, ~/.npmrc off root
export TMPDIR=/data/tmp
export CARGO_HOME=/data/.cargo
export npm_config_cache=/data/.npm
mkdir -p "$HOME" "$TMPDIR"
```

### 9.3 Rust verification — the real compatibility check (no Node needed)

```bash
cd /data/rolldown
export CARGO_HOME=/data/.cargo
cargo build -p rolldown                                 # native build sanity
cargo test --workspace --exclude rolldown_binding       # expect: 1767 passed (with the fix)
# targeted (the originally-failing escaping/sourcemap tests):
cargo test -p rolldown --lib utils::render_ecma_module  # 5 passed
```

Confirm the `[patch.crates-io]` is active before trusting results:
```bash
cargo tree -i json-escape-simd     # must show: (/data/rolldown/vendor/json-escape-simd)
# if it shows the crates.io copy instead:
cargo update -p json-escape-simd   # forces the lockfile onto the patch
```

### 9.4 Node + pnpm (only needed for JS-side tests)

`corepack` (bundled with Node) has a stale-key signature bug; install pnpm with `npm` instead.
`pnpm@11.4.0` requires Node ≥ 22.13, so install Node 22 to a writable volume:

```bash
cd /data
curl -fLO https://nodejs.org/dist/v22.13.0/node-v22.13.0-linux-s390x.tar.xz
mkdir -p /data/node22 && tar -xJf node-v22.13.0-linux-s390x.tar.xz -C /data/node22 --strip-components=1
export PATH=/data/node22/bin:$PATH
# remove any corepack pnpm shims, then install real pnpm:
rm -f /data/node22/bin/pnpm /data/node22/bin/pnpx
npm install -g pnpm@11.4.0 && hash -r
node -v && pnpm -v        # v22.13.0 / 11.4.0
```

Install JS deps, skipping postinstall scripts (`workerd`/`playwright-chromium` have no s390x
build), and disable the pre-run dep check so it doesn't re-trigger the failing install:

```bash
cd /data/rolldown
pnpm config set store-dir /data/.pnpm-store
pnpm install --frozen-lockfile --ignore-scripts
export npm_config_verify_deps_before_run=false
# nested test-fixture workspace deps (needed by some integration fixtures):
ls node_modules/cjs-module-lexer >/dev/null && echo "cjs-module-lexer OK"
ls crates/rolldown/tests/rolldown/topics/npm_packages/node_modules >/dev/null && echo "npm-packages OK"
# integration fixtures that need the test262 submodule:
git submodule update --init test262
```

### 9.5 Build the native binding (two routes)

The napi build invokes cargo with an explicit `--target s390x-unknown-linux-gnu`, which makes
cargo/cc look for the **Debian cross-compiler name** `s390x-linux-gnu-gcc` even though you are
native. Provide it (native `gcc` *is* the s390x compiler):

```bash
mkdir -p /data/bin
ln -sf "$(command -v gcc)" /data/bin/s390x-linux-gnu-gcc
ln -sf "$(command -v g++)" /data/bin/s390x-linux-gnu-g++
export PATH=/data/bin:$PATH
export CARGO_TARGET_S390X_UNKNOWN_LINUX_GNU_LINKER=gcc
export CC_s390x_unknown_linux_gnu=gcc CXX_s390x_unknown_linux_gnu=g++
```

**Route A — cargo-direct (most reliable; avoids the broken TS runner).** `binding.cjs` already
exists in the tree and loads `./rolldown-binding.linux-s390x-gnu.node` *first*, so just build the
cdylib natively and drop it in:

```bash
cd /data/rolldown
cargo build -p rolldown_binding                          # native cdylib, default gcc (no --target)
cp target/debug/librolldown_binding.so \
   packages/rolldown/src/rolldown-binding.linux-s390x-gnu.node
```

**Route B — full napi build** (`pnpm --filter rolldown run build-binding`) also works once the
cross-linker symlink above is in place, but the subsequent `build-js-glue`/`dist` step
(`@oxc-node/core` executing `build.ts`) crashes on s390x — so Route A is preferred when you only
need the loadable binding.

### 9.6 Run the JS test suites

```bash
export PATH=/data/node22/bin:/data/bin:$PATH
export CARGO_HOME=/data/.cargo npm_config_verify_deps_before_run=false
pnpm --filter rolldown-tests run test:main    # ⚠️ currently blocked: vitest config loader
pnpm --filter rollup-tests run test           # needs rolldown `dist` (blocked, see 9.7)
```

**Status:** with Route A's binding in place, the suite loads **your patched code** (verified: the
prebuilt 1.0.3 binding instead panics with `node:module` → `node:modle …`). The suite is
currently gated earlier by vitest's config loader (`config must export or return an object`) — a
vite 8 / vitest 4 issue on s390x; esbuild itself works. This does not affect the compatibility
verdict, which rests on Section 9.3.

### 9.7 Known dev-tooling gaps on native s390x (not rolldown bugs)

| Tool | Symptom | Cause / workaround |
|---|---|---|
| `corepack` | `Cannot find matching keyid` | stale bundled keys; install pnpm via `npm i -g pnpm@11.4.0` |
| `workerd` | `Unsupported platform: linux s390x BE` | no s390x build; `pnpm install --ignore-scripts` |
| `playwright-chromium` | no s390x browser | no s390x build; `--ignore-scripts` |
| cargo `--target s390x` link | `linker s390x-linux-gnu-gcc not found` | symlink to native `gcc` (9.5) |
| `oxnode` / `build.ts` (`@oxc-node/core`) | exit 129 when **executing** TS | loads fine, runtime crash; use Route A, skip `dist` |
| vitest `test:main` | `config must export or return an object` | vite 8 / vitest 4 config loader on s390x |
| `rollup-tests` | `Cannot find module rolldown/dist/index.mjs` | `dist` not built (blocked by `build.ts`) |
