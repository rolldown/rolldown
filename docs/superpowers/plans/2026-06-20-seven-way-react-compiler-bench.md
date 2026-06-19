# Seven-Way React Compiler Bench Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build seven implementations of the same React Compiler transform plus a bench harness that compares them on the Infisical frontend corpus.

**Architecture:** A new types-only crate (`rolldown_native_plugin_abi`) defines a `#[repr(C)]` C ABI for dlopen-loadable plugins. A new `bench_native_lib_plugin` cdylib implements that ABI. Inside `rolldown_binding`, a `NativeStringHolder` (enum-inner: `ArcStr` or `String`) backs two new sync-vs-async transform-hook fields, and a `NativeLibPlugin` loader dlopens the cdylib and dispatches its transform fn directly. JS plumbing adds `defineNativeLibPlugin`, passes the new bridge fields through `bindingify-plugin.ts`, and patches `parallel-plugin-worker.ts` so worker JS threads stay alive after bootstrap. A bench harness in `scripts/bench/seven-way-react-compiler/` defines the seven variants and runs the primary + secondary tables.

**Tech Stack:** Rust (rolldown_binding crate, oxc 0.136, libloading 0.8), napi-rs 3.0, Node.js (Node 20.19+/22.12+), pnpm + vp + just, vitest.

**Spec:** `docs/superpowers/specs/2026-06-20-seven-way-react-compiler-bench-design.md`

---

## File map

**Create:**
- `crates/rolldown_native_plugin_abi/Cargo.toml`
- `crates/rolldown_native_plugin_abi/src/lib.rs` — types, ABI_VERSION, symbol-name constants
- `crates/bench_native_lib_plugin/Cargo.toml`
- `crates/bench_native_lib_plugin/src/lib.rs` — three extern "C" symbols
- `crates/rolldown_binding/src/native_bridge.rs` — `NativeStringHolder` + tests
- `crates/rolldown_binding/src/bench_oxc_transformer.rs` — four `#[napi]` methods
- `crates/rolldown_binding/src/options/plugin/native_lib_plugin.rs` — dlopen loader, `Plugin` impl
- `crates/rolldown_binding/src/options/plugin/binding_native_lib_plugin.rs` — `{name, path}` napi object
- `packages/rolldown/src/plugin/native-lib-plugin.ts` — `defineNativeLibPlugin`
- `packages/rolldown/tests/native-bridge-plugin.test.ts` — round-trip integration test
- `scripts/bench/seven-way-react-compiler/.gitignore`
- `scripts/bench/seven-way-react-compiler/setup.mjs` — Infisical sparse-clone
- `scripts/bench/seven-way-react-compiler/parallel-impl.mjs` — variant 7's parallel impl
- `scripts/bench/seven-way-react-compiler/run.mjs` — bench runner
- `scripts/bench/seven-way-react-compiler/results.md` — populated after running

**Modify:**
- `Cargo.toml` — add `libloading` and `rolldown_native_plugin_abi` workspace deps
- `crates/rolldown_binding/Cargo.toml` — add `libloading`, `oxc_react_compiler`, `rolldown_ecmascript`, `rolldown_native_plugin_abi`
- `crates/rolldown_binding/src/lib.rs` — register `native_bridge` and `bench_oxc_transformer` modules
- `crates/rolldown_binding/src/options/plugin/mod.rs` — register `native_lib_plugin` + `binding_native_lib_plugin` modules; `pub use`
- `crates/rolldown_binding/src/options/plugin/binding_plugin_options.rs` — add two bridge fields; widen the placeholder Either to `Either3`
- `crates/rolldown_binding/src/options/plugin/js_plugin.rs` — dispatch sync bridge → async bridge → existing transform
- `crates/rolldown_binding/src/options/plugin/parallel_js_plugin.rs` — OR the new fields into the transform dispatch condition
- `crates/rolldown_binding/src/options/binding_input_options/mod.rs` — widen `plugins` ts_type to include `BindingNativeLibPlugin`
- `crates/rolldown_binding/src/options/binding_output_options/mod.rs` — same
- `crates/rolldown_binding/src/utils/normalize_binding_options.rs` — match `Either3` variants when converting plugins to `SharedPluginable`
- `packages/rolldown/src/plugin/index.ts` — `RolldownPlugin` union extends to include `NativeLibPlugin`
- `packages/rolldown/src/plugin/bindingify-plugin.ts` — pass `transformNativeBridge` and `transformNativeBridgeAsync` through
- `packages/rolldown/src/plugin/generated/hook-usage.ts` — `HookUsageKind.transform` when either bridge field is set
- `packages/rolldown/src/parallel-plugin-worker.ts` — drop `parentPort.unref()` on success path; add `setInterval` keep-alive
- `packages/rolldown/src/utils/bindingify-input-options.ts` — detect `_nativeLib` and emit the binding native-lib descriptor
- `packages/rolldown/src/experimental-index.ts` — re-export `defineNativeLibPlugin`

---

## Build/test commands (reference)

| Action | Command |
|---|---|
| Workspace cargo check | `cargo check -p <crate>` |
| Unit tests (binding) | `cargo test -p rolldown_binding --lib` |
| Clippy (binding) | `cargo clippy -p rolldown_binding --all-targets -- --deny warnings` |
| Build binding (debug) | `just build-rolldown-binding` |
| Build rolldown (debug, binding + dist) | `just build-rolldown` |
| Build rolldown (release) | `just build-rolldown-release` |
| Build the cdylib (debug) | `cargo build -p bench_native_lib_plugin` |
| Build the cdylib (release) | `cargo build --release -p bench_native_lib_plugin` |
| JS test by name | `just t-node-rolldown -- <pattern>` |
| Rust fmt | `cargo fmt --all -- --emit=files` |

---

## Task 1: Create `rolldown_native_plugin_abi` types crate

**Files:**
- Create: `crates/rolldown_native_plugin_abi/Cargo.toml`
- Create: `crates/rolldown_native_plugin_abi/src/lib.rs`

- [ ] **Step 1: Write the Cargo manifest**

Create `crates/rolldown_native_plugin_abi/Cargo.toml`:

```toml
[package]
name = "rolldown_native_plugin_abi"
version = "0.0.1"
publish = false
edition.workspace = true
homepage.workspace = true
license.workspace = true
repository.workspace = true

[lints]
workspace = true

[lib]
doctest = false
```

- [ ] **Step 2: Write the ABI types and constants**

Create `crates/rolldown_native_plugin_abi/src/lib.rs`:

```rust
//! Stable C ABI for rolldown native plugins loaded via dlopen.
//!
//! A plugin is a shared library (`.dylib`/`.so`/`.dll`) that exports a small,
//! fixed set of `extern "C"` symbols. Rolldown loads the library at startup
//! and dispatches transform calls directly from its worker threads via raw
//! function pointers — no JS, no napi, no ThreadsafeFunction.
//!
//! # ABI guarantees
//!
//! The types in this crate are `#[repr(C)]` so their field layout is stable
//! across Rust versions and compilers (under the same target ABI). Pointer
//! and `usize` widths follow the target's platform. There is no Rust-specific
//! data on the wire.
//!
//! # Symbol contract
//!
//! Every plugin must export these symbols with C linkage:
//!
//! - `rolldown_native_plugin_abi_version() -> u32` returns the plugin's ABI
//!   version. Rolldown refuses to load plugins whose version differs from
//!   `ABI_VERSION`.
//! - `rolldown_native_plugin_transform(source, id, *mut out) -> i32` runs the
//!   transform. Returns 0 on success; non-zero on error.
//! - `rolldown_native_plugin_drop_output(*mut out)` releases the buffers in
//!   `out`. The host calls this exactly once per successful `transform`.
//!
//! # Lifetime rules
//!
//! - `source`/`id` are borrowed for the duration of the `transform` call only.
//!   The plugin must NOT retain pointers into them after returning.
//! - `out.code` (and `out.error`, if present) must remain valid until the host
//!   calls `rolldown_native_plugin_drop_output`. The host calls drop exactly
//!   once per successful `transform`.
//! - Buffers in `out` are owned and freed by the plugin (so allocator
//!   mismatches between host and plugin are impossible).

/// Bump on every breaking change to the symbol set or the types below.
pub const ABI_VERSION: u32 = 1;

/// Borrowed view of a UTF-8 byte slice. Equivalent layout to
/// `{ const u8 *ptr; size_t len; }`. The pointer may be null iff `len == 0`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStr {
  pub ptr: *const u8,
  pub len: usize,
}

impl NativeStr {
  pub const EMPTY: Self = Self { ptr: std::ptr::null(), len: 0 };

  /// # Safety
  /// `self` must reference a valid UTF-8 buffer for `'a`.
  pub unsafe fn as_str<'a>(&self) -> &'a str {
    if self.len == 0 {
      return "";
    }
    unsafe {
      let bytes = std::slice::from_raw_parts(self.ptr, self.len);
      std::str::from_utf8_unchecked(bytes)
    }
  }
}

// SAFETY: the pointers in NativeStr only carry references for the duration
// of a single call; the buffer's owner upholds Send/Sync.
unsafe impl Send for NativeStr {}
unsafe impl Sync for NativeStr {}

/// Out-parameter for `transform`. On success the plugin populates `code` with
/// a pointer to the transformed source. On error, `code` is left as
/// `NativeStr::EMPTY` and `error` points at a UTF-8 error message (optional —
/// may also be empty).
///
/// `plugin_data` is an opaque pointer the plugin may use to track per-output
/// state (e.g. a pointer to a `Box<String>` so `drop_output` can reclaim it).
/// Host code must not interpret `plugin_data`.
#[repr(C)]
pub struct TransformOutput {
  pub code: NativeStr,
  pub error: NativeStr,
  pub plugin_data: *mut core::ffi::c_void,
}

impl TransformOutput {
  pub const ZEROED: Self = Self {
    code: NativeStr::EMPTY,
    error: NativeStr::EMPTY,
    plugin_data: std::ptr::null_mut(),
  };
}

unsafe impl Send for TransformOutput {}
unsafe impl Sync for TransformOutput {}

/// Function-pointer types for the three symbols a plugin exports. Useful for
/// hosts that resolve symbols via `dlsym`/`libloading`.
pub type FnAbiVersion = unsafe extern "C" fn() -> u32;
pub type FnTransform =
  unsafe extern "C" fn(source: NativeStr, id: NativeStr, out: *mut TransformOutput) -> i32;
pub type FnDropOutput = unsafe extern "C" fn(out: *mut TransformOutput);

pub const SYM_ABI_VERSION: &str = "rolldown_native_plugin_abi_version";
pub const SYM_TRANSFORM: &str = "rolldown_native_plugin_transform";
pub const SYM_DROP_OUTPUT: &str = "rolldown_native_plugin_drop_output";
```

- [ ] **Step 3: Verify it builds**

Run:
```
cargo check -p rolldown_native_plugin_abi
```
Expected: `Finished dev profile`. No warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/rolldown_native_plugin_abi/
git commit -m "feat(abi): add rolldown_native_plugin_abi types crate"
```

---

## Task 2: Add `libloading` + `rolldown_native_plugin_abi` to workspace deps

**Files:**
- Modify: `Cargo.toml` — add workspace dependencies

- [ ] **Step 1: Add libloading and the ABI crate to workspace deps**

In `Cargo.toml`, add a `libloading` entry alphabetically in the third-party section (search for `mimalloc-safe` and add just above it):

```toml
libloading = "0.8.5"
```

In the same file's rolldown-paths section (search for `rolldown_fs_watcher` and add just below it):

```toml
rolldown_native_plugin_abi = { version = "0.0.1", path = "crates/rolldown_native_plugin_abi" }
```

- [ ] **Step 2: Verify workspace still resolves**

Run:
```
cargo check --workspace -p rolldown_native_plugin_abi
```
Expected: `Finished` without errors.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add libloading and rolldown_native_plugin_abi to workspace deps"
```

---

## Task 3: Create `bench_native_lib_plugin` cdylib

**Files:**
- Create: `crates/bench_native_lib_plugin/Cargo.toml`
- Create: `crates/bench_native_lib_plugin/src/lib.rs`

- [ ] **Step 1: Cargo manifest**

Create `crates/bench_native_lib_plugin/Cargo.toml`:

```toml
[package]
name = "bench_native_lib_plugin"
version = "0.0.1"
publish = false
edition.workspace = true
homepage.workspace = true
license.workspace = true
repository.workspace = true

[lints]
workspace = true

[lib]
crate-type = ["cdylib"]
doctest = false

[dependencies]
oxc = { workspace = true }
oxc_react_compiler = { workspace = true }
rolldown_ecmascript = { workspace = true }
rolldown_native_plugin_abi = { workspace = true }
```

- [ ] **Step 2: Implement the three extern "C" symbols**

Create `crates/bench_native_lib_plugin/src/lib.rs`:

```rust
//! Bench native-lib plugin. Exports the three rolldown native-plugin ABI
//! symbols and runs the same parse → semantic → transform(react_compiler=ON)
//! → codegen pipeline as `BenchOxcTransformer`.
//!
//! Built as a `cdylib`, loaded by rolldown via `dlopen`/`libloading`.

use std::path::Path;

use oxc::allocator::Allocator;
use oxc::codegen::{Codegen, CodegenOptions, CodegenReturn};
use oxc::parser::{ParseOptions, Parser};
use oxc::span::SourceType;
use oxc::transformer::{TransformOptions, Transformer};
use rolldown_ecmascript::semantic_builder_for_transform;
use rolldown_native_plugin_abi::{ABI_VERSION, NativeStr, TransformOutput};

/// Owned String tracked through `plugin_data` so `drop_output` can reclaim it.
struct OwnedOutput {
  code: String,
}

#[unsafe(no_mangle)]
pub extern "C" fn rolldown_native_plugin_abi_version() -> u32 {
  ABI_VERSION
}

/// # Safety
/// `out` must be a valid, writable `TransformOutput`. `source` and `id` must
/// reference valid UTF-8 buffers for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rolldown_native_plugin_transform(
  source: NativeStr,
  id: NativeStr,
  out: *mut TransformOutput,
) -> i32 {
  let result = std::panic::catch_unwind(|| {
    // SAFETY: caller upholds the lifetime contract.
    let src = unsafe { source.as_str() };
    let id_str = unsafe { id.as_str() };
    run_transform(src, id_str)
  });

  let Ok(code) = result else {
    return -1;
  };

  let owned = Box::new(OwnedOutput { code });
  let code_native = NativeStr { ptr: owned.code.as_ptr(), len: owned.code.len() };
  let plugin_data = Box::into_raw(owned).cast::<core::ffi::c_void>();

  // SAFETY: caller guarantees `out` is writable.
  unsafe {
    (*out).code = code_native;
    (*out).error = NativeStr::EMPTY;
    (*out).plugin_data = plugin_data;
  }
  0
}

/// # Safety
/// `out` must point to a `TransformOutput` populated by a prior successful
/// call to `rolldown_native_plugin_transform` and not yet dropped.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rolldown_native_plugin_drop_output(out: *mut TransformOutput) {
  if out.is_null() {
    return;
  }
  // SAFETY: `plugin_data` was the result of `Box::into_raw` on `Box<OwnedOutput>`.
  unsafe {
    let pd = (*out).plugin_data;
    if !pd.is_null() {
      drop(Box::from_raw(pd.cast::<OwnedOutput>()));
    }
    (*out).code = NativeStr::EMPTY;
    (*out).error = NativeStr::EMPTY;
    (*out).plugin_data = std::ptr::null_mut();
  }
}

fn run_transform(source: &str, id: &str) -> String {
  let path = Path::new(id);
  let source_type = SourceType::from_path(path).unwrap_or_else(|_| SourceType::tsx());

  let allocator = Allocator::default();
  let parse_ret = Parser::new(&allocator, source, source_type)
    .with_options(ParseOptions { allow_return_outside_function: true, ..Default::default() })
    .parse();

  let mut program = parse_ret.program;

  let semantic_ret = semantic_builder_for_transform().build(&program);
  let scoping = semantic_ret.semantic.into_scoping();

  let transform_options = TransformOptions {
    react_compiler: Some(oxc_react_compiler::default_plugin_options()),
    ..Default::default()
  };

  let _ = Transformer::new(&allocator, path, &transform_options)
    .build_with_scoping(scoping, &mut program);

  let codegen_ret: CodegenReturn =
    Codegen::new().with_options(CodegenOptions::default()).build(&program);

  codegen_ret.code
}
```

- [ ] **Step 3: Build the cdylib**

Run:
```
cargo build -p bench_native_lib_plugin
```
Expected: build succeeds, `target/debug/libbench_native_lib_plugin.dylib` exists (on macOS) or `.so`/`.dll` on Linux/Windows.

- [ ] **Step 4: Lint**

Run:
```
cargo clippy -p bench_native_lib_plugin --all-targets -- --deny warnings
```
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/bench_native_lib_plugin/
git commit -m "feat(bench): bench_native_lib_plugin cdylib implementing the C ABI"
```

---

## Task 4: Add Cargo deps for the binding's bridge work

**Files:**
- Modify: `crates/rolldown_binding/Cargo.toml`

- [ ] **Step 1: Add deps**

In `crates/rolldown_binding/Cargo.toml`, locate the `[dependencies]` section and add four entries alphabetically:

- `libloading = { workspace = true }` (after `itertools = { workspace = true }`)
- `oxc_react_compiler = { workspace = true }` (after `oxc_parser_napi = { workspace = true }`)
- `rolldown_ecmascript = { workspace = true }` (after `rolldown_common = { workspace = true }`)
- `rolldown_native_plugin_abi = { workspace = true }` (after `rolldown_devtools = { workspace = true }`)

- [ ] **Step 2: Verify it compiles**

Run:
```
cargo check -p rolldown_binding
```
Expected: builds; the new deps aren't used yet but cargo doesn't care.

- [ ] **Step 3: Commit**

```bash
git add crates/rolldown_binding/Cargo.toml Cargo.lock
git commit -m "chore(binding): add libloading + oxc_react_compiler + rolldown_ecmascript + rolldown_native_plugin_abi deps"
```

---

## Task 5: `NativeStringHolder` bridge type with unit tests

**Files:**
- Create: `crates/rolldown_binding/src/native_bridge.rs`
- Modify: `crates/rolldown_binding/src/lib.rs` — add `pub mod native_bridge;`

- [ ] **Step 1: Write the file with tests**

Create `crates/rolldown_binding/src/native_bridge.rs`:

```rust
use arcstr::ArcStr;

#[repr(C)]
pub struct NativeStrRef {
  pub ptr: *const u8,
  pub len: usize,
}

/// Backing buffer for a `NativeStringHolder`. Two flavors so each side can pick
/// the cheapest representation:
/// - `ArcStr` on the input path: a `clone()` is an Arc count bump, not a copy.
/// - `String` on the output path: extracting it back into `HookTransformOutput::code`
///   is a move, not a copy.
enum HolderInner {
  ArcStr(ArcStr),
  String(String),
}

pub struct NativeStringHolder {
  inner: HolderInner,
  view: NativeStrRef,
}

impl NativeStringHolder {
  pub fn from_arcstr(s: ArcStr) -> Self {
    let view = NativeStrRef { ptr: s.as_ptr(), len: s.len() };
    Self { inner: HolderInner::ArcStr(s), view }
  }

  pub fn from_string(s: String) -> Self {
    let view = NativeStrRef { ptr: s.as_ptr(), len: s.len() };
    Self { inner: HolderInner::String(s), view }
  }

  pub fn as_str(&self) -> &str {
    // SAFETY: `inner` owns the buffer for the lifetime of `self`; bytes are valid UTF-8.
    unsafe {
      let bytes = std::slice::from_raw_parts(self.view.ptr, self.view.len);
      std::str::from_utf8_unchecked(bytes)
    }
  }

  pub fn into_string(self) -> String {
    match self.inner {
      HolderInner::String(s) => s,
      HolderInner::ArcStr(s) => s.as_str().to_owned(),
    }
  }

  pub fn into_raw_handle(self) -> i64 {
    Box::into_raw(Box::new(self)) as i64
  }

  /// # Safety
  /// `handle` must originate from `into_raw_handle` and must not have been
  /// reclaimed yet (no double-free).
  pub unsafe fn from_raw_handle(handle: i64) -> Self {
    *unsafe { Box::from_raw(handle as *mut Self) }
  }

  /// # Safety
  /// `handle` must originate from `into_raw_handle` and the Holder must outlive
  /// the returned borrow (don't call `from_raw_handle` while the `&str` is live).
  pub unsafe fn handle_as_str<'a>(handle: i64) -> &'a str {
    let holder: &'a Self = unsafe { &*(handle as *const Self) };
    holder.as_str()
  }
}

// SAFETY: both `ArcStr` and `String` are Send+Sync; the raw `view.ptr` aliases
// the inner buffer and is only read while the inner value (and therefore the
// buffer) is alive.
unsafe impl Send for NativeStringHolder {}
unsafe impl Sync for NativeStringHolder {}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn round_trips_a_string_including_multibyte_chars() {
    let s = "hello ✨ 世界".to_string();
    let holder = NativeStringHolder::from_string(s.clone());
    assert_eq!(holder.as_str(), &s);
    assert_eq!(holder.into_string(), s);
  }

  #[test]
  fn ptr_and_len_match_arcstr_source() {
    let arc = ArcStr::from("abc");
    let p = arc.as_ptr();
    let l = arc.len();
    let holder = NativeStringHolder::from_arcstr(arc);
    assert_eq!(holder.view.ptr, p);
    assert_eq!(holder.view.len, l);
  }

  #[test]
  fn into_string_from_string_is_a_move() {
    let holder = NativeStringHolder::from_string("unique".to_string());
    assert_eq!(holder.into_string(), "unique");
  }

  #[test]
  fn into_string_from_arcstr_copies() {
    let arc = ArcStr::from("shared");
    let holder = NativeStringHolder::from_arcstr(arc.clone());
    assert_eq!(holder.into_string(), "shared");
    assert_eq!(arc, "shared");
  }

  #[test]
  fn raw_handle_round_trip_with_string_inner() {
    let s = "round-trip ✨";
    let holder = NativeStringHolder::from_string(s.to_string());
    let handle = holder.into_raw_handle();
    unsafe {
      assert_eq!(NativeStringHolder::handle_as_str(handle), s);
      let reclaimed = NativeStringHolder::from_raw_handle(handle);
      assert_eq!(reclaimed.into_string(), s);
    }
  }

  #[test]
  fn raw_handle_round_trip_with_arcstr_inner() {
    let s = "shared-input";
    let holder = NativeStringHolder::from_arcstr(ArcStr::from(s));
    let handle = holder.into_raw_handle();
    unsafe {
      assert_eq!(NativeStringHolder::handle_as_str(handle), s);
      let reclaimed = NativeStringHolder::from_raw_handle(handle);
      drop(reclaimed);
    }
  }
}
```

- [ ] **Step 2: Register the module**

In `crates/rolldown_binding/src/lib.rs`, find the section with `pub mod binding_bundler;` etc and add:

```rust
pub mod native_bridge;
```

Place it alphabetically (after `mod generated;` and before `pub mod options;`).

- [ ] **Step 3: Run tests**

Run:
```
cargo test -p rolldown_binding --lib native_bridge
```
Expected: 6 passed.

- [ ] **Step 4: Lint**

Run:
```
cargo clippy -p rolldown_binding --all-targets -- --deny warnings
```
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/rolldown_binding/src/native_bridge.rs crates/rolldown_binding/src/lib.rs
git commit -m "feat(binding): NativeStringHolder bridge type with ArcStr/String inner"
```

---

## Task 6: `BenchOxcTransformer` napi class

**Files:**
- Create: `crates/rolldown_binding/src/bench_oxc_transformer.rs`
- Modify: `crates/rolldown_binding/src/lib.rs` — add `pub mod bench_oxc_transformer;`

- [ ] **Step 1: Write the file**

Create `crates/rolldown_binding/src/bench_oxc_transformer.rs`:

```rust
use std::path::Path;

use napi_derive::napi;
use oxc::allocator::Allocator;
use oxc::codegen::{Codegen, CodegenOptions, CodegenReturn};
use oxc::parser::{ParseOptions, Parser};
use oxc::span::SourceType;
use oxc::transformer::{TransformOptions, Transformer};
use rolldown_ecmascript::semantic_builder_for_transform;

use crate::native_bridge::NativeStringHolder;

#[napi]
pub struct BenchOxcTransformer {}

#[napi]
impl BenchOxcTransformer {
  #[napi(constructor)]
  pub fn new() -> Self {
    Self {}
  }

  #[napi]
  pub fn transform_str(&self, source: String, id: String) -> String {
    run_transform(&source, &id)
  }

  /// Async string variant. Same contract as `transform_str` but returns a
  /// `Promise<string>`. Used to test the parallel async-call shape on the
  /// non-bridge path.
  #[napi]
  pub async fn transform_str_async(&self, source: String, id: String) -> String {
    // Yield once so the async body has an await point.
    napi::tokio::task::yield_now().await;
    run_transform(&source, &id)
  }

  #[napi(ts_args_type = "sourceHandle: bigint, id: string", ts_return_type = "bigint")]
  pub fn transform_native(&self, source_handle: i64, id: String) -> i64 {
    // SAFETY: caller supplies a handle previously produced by
    // `NativeStringHolder::into_raw_handle` whose backing box is still alive.
    let src: &str = unsafe { NativeStringHolder::handle_as_str(source_handle) };
    let output = run_transform(src, &id);
    NativeStringHolder::from_string(output).into_raw_handle()
  }

  #[napi(
    ts_args_type = "sourceHandle: bigint, id: string",
    ts_return_type = "Promise<bigint>"
  )]
  pub async fn transform_native_async(&self, source_handle: i64, id: String) -> i64 {
    napi::tokio::task::yield_now().await;
    // SAFETY: same contract as `transform_native`.
    let src: &str = unsafe { NativeStringHolder::handle_as_str(source_handle) };
    let output = run_transform(src, &id);
    NativeStringHolder::from_string(output).into_raw_handle()
  }
}

fn run_transform(source: &str, id: &str) -> String {
  let path = Path::new(id);
  let source_type = SourceType::from_path(path).unwrap_or_else(|_| SourceType::tsx());

  let allocator = Allocator::default();
  let parse_ret = Parser::new(&allocator, source, source_type)
    .with_options(ParseOptions { allow_return_outside_function: true, ..Default::default() })
    .parse();

  let mut program = parse_ret.program;

  let semantic_ret = semantic_builder_for_transform().build(&program);
  let scoping = semantic_ret.semantic.into_scoping();

  let transform_options = TransformOptions {
    react_compiler: Some(oxc_react_compiler::default_plugin_options()),
    ..Default::default()
  };

  let _ = Transformer::new(&allocator, path, &transform_options)
    .build_with_scoping(scoping, &mut program);

  let codegen_ret: CodegenReturn =
    Codegen::new().with_options(CodegenOptions::default()).build(&program);

  codegen_ret.code
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::Arc;

  const SAMPLE: &str = r"
    import * as React from 'react';
    export function Counter() {
      const [n, setN] = React.useState(0);
      return <button onClick={() => setN(n + 1)}>{n}</button>;
    }
  ";

  #[test]
  fn run_transform_runs_react_compiler() {
    let out = run_transform(SAMPLE, "Counter.tsx");
    // React Compiler emits a `_c(N)` cache reference. If oxc changes the marker
    // (e.g. to `useMemoCache` or `react-compiler-runtime`), update accordingly.
    assert!(out.contains("_c("), "expected React Compiler cache call in output, got:\n{out}");
  }

  #[test]
  fn str_and_native_paths_produce_identical_output() {
    let direct = run_transform(SAMPLE, "Counter.tsx");

    let holder = NativeStringHolder::from_string(SAMPLE.to_string());
    let via_native = run_transform(holder.as_str(), "Counter.tsx");

    assert_eq!(direct, via_native);

    let _ = Arc::new(()); // unused import suppressant; remove if cargo complains
  }
}
```

- [ ] **Step 2: Register the module**

In `crates/rolldown_binding/src/lib.rs`, add (alphabetically, before `pub mod binding_bundler;`):

```rust
pub mod bench_oxc_transformer;
```

- [ ] **Step 3: Run the tests**

Run:
```
cargo test -p rolldown_binding --lib bench_oxc_transformer
```
Expected: 2 passed.

- [ ] **Step 4: Build the binding to compile the napi class into the .node**

Run:
```
just build-rolldown-binding
```
Expected: builds.

- [ ] **Step 5: Verify TS generation**

Run:
```
grep -n "BenchOxcTransformer\|transformStr\|transformNative" packages/rolldown/src/binding.d.cts | head -10
```
Expected: class declaration, four method signatures with `transformStr`, `transformStrAsync`, `transformNative(sourceHandle: bigint, id: string): bigint`, `transformNativeAsync(...): Promise<bigint>`.

If `transformNativeAsync` shows `Promise<unknown>` or anything other than `Promise<bigint>`, double-check the `ts_return_type` attribute.

- [ ] **Step 6: Lint**

Run:
```
cargo clippy -p rolldown_binding --all-targets -- --deny warnings
```

If clippy fires `unused_async` on the two async methods, add `#[expect(clippy::unused_async)]` immediately *before* the `#[napi(...)]` attribute (not after, the macro doesn't pass through attributes uniformly).

- [ ] **Step 7: Commit**

```bash
git add crates/rolldown_binding/src/bench_oxc_transformer.rs crates/rolldown_binding/src/lib.rs packages/rolldown/src/binding.cjs packages/rolldown/src/binding.d.cts packages/rolldown/src/rolldown-binding.wasi-browser.js packages/rolldown/src/rolldown-binding.wasi.cjs Cargo.lock
git commit -m "feat(binding): BenchOxcTransformer napi class with four transform methods"
```

---

## Task 7: Split bridge fields on `BindingPluginOptions`

**Files:**
- Modify: `crates/rolldown_binding/src/options/plugin/binding_plugin_options.rs`

- [ ] **Step 1: Add the two split fields**

Open the file. At the top, locate the imports and add (or merge with existing `use crate::types::{...}`):

```rust
use napi::bindgen_prelude::Promise;
```

Locate the existing `transform_filter` field. Immediately after it (still inside the struct), insert:

```rust
  /// Experimental: sync zero-copy bridge transform hook. Takes a `bigint`
  /// handle wrapping a `Box<NativeStringHolder>` (see crates/rolldown_binding/
  /// src/native_bridge.rs) and returns a fresh handle (or null). Avoids the
  /// UTF-8 ↔ UTF-16 round trip on the source code body. Sync-only — for the
  /// async variant see `transform_native_bridge_async`.
  #[napi(ts_type = "(sourceHandle: bigint, id: string) => bigint | null | undefined")]
  pub transform_native_bridge:
    Option<JsCallback<FnArgs<(i64, String)>, Option<i64>>>,

  /// Experimental: async zero-copy bridge transform hook. Takes a `bigint`
  /// handle and MUST return a `Promise<bigint>` (or `Promise<null>` /
  /// `Promise<undefined>`). The JS thread is freed immediately on dispatch
  /// while the napi-side `transformNativeAsync` resolves the Promise.
  #[napi(ts_type = "(sourceHandle: bigint, id: string) => Promise<bigint | null | undefined>")]
  pub transform_native_bridge_async:
    Option<JsCallback<FnArgs<(i64, String)>, Promise<Option<i64>>>>,
```

You'll also need `JsCallback` in scope. If only `MaybeAsyncJsCallback` is currently imported via `crate::types::{… js_callback::MaybeAsyncJsCallback}`, change that import to:

```rust
use crate::types::{
  …
  js_callback::{JsCallback, MaybeAsyncJsCallback},
};
```

(Keep the surrounding `…` lines verbatim; only the `js_callback::…` line changes.)

- [ ] **Step 2: Verify it compiles**

Run:
```
cargo check -p rolldown_binding
```

If napi-rs rejects `JsCallback<…, Promise<Option<i64>>>` with a missing-trait error (likely `FromNapiValue` or `TypeName`), fall back to:

```rust
pub transform_native_bridge_async:
  Option<MaybeAsyncJsCallback<FnArgs<(i64, String)>, Option<i64>>>,
```

and note in the commit message that the strict-Promise validation falls back to `MaybeAsyncJsCallback`. This is the documented escape hatch from the spec's open questions.

- [ ] **Step 3: Rebuild binding to regenerate TS types**

Run:
```
just build-rolldown-binding
```
Then:
```
grep -n "transformNativeBridge" packages/rolldown/src/binding.d.cts | head -5
```
Expected: two lines, one for sync, one for async.

- [ ] **Step 4: Commit**

```bash
git add crates/rolldown_binding/src/options/plugin/binding_plugin_options.rs packages/rolldown/src/binding.cjs packages/rolldown/src/binding.d.cts packages/rolldown/src/rolldown-binding.wasi-browser.js packages/rolldown/src/rolldown-binding.wasi.cjs
git commit -m "feat(binding): split bridge into sync + async fields on BindingPluginOptions"
```

---

## Task 8: `JsPlugin::transform` dispatch order

**Files:**
- Modify: `crates/rolldown_binding/src/options/plugin/js_plugin.rs`

- [ ] **Step 1: Add imports**

Near the top of the file (in the existing `use crate::…` block), make sure these are present:

```rust
use crate::native_bridge::NativeStringHolder;
use crate::types::{
  …
  js_callback::{JsCallbackExt, MaybeAsyncJsCallbackExt},
};
```

If `JsCallbackExt` isn't already in scope (it lives in `crate::types::js_callback`), import it.

- [ ] **Step 2: Insert the bridge branches at the top of `transform`**

Locate `async fn transform(...)` in `impl Plugin for JsPlugin`. Immediately after the function's opening brace (before the existing `let Some(cb) = &self.transform else { … };` line), insert:

```rust
    // Sync zero-copy bridge. Skip the regular include/exclude filter (PoC scope).
    if let Some(cb) = &self.transform_native_bridge {
      use std::sync::Arc;
      let source_handle =
        NativeStringHolder::from_arcstr(args.code.clone()).into_raw_handle();
      let _ = Arc::new(()); // keep `use std::sync::Arc` honest if not used elsewhere

      let result_handle = cb
        .invoke_async((source_handle, args.id.to_string()).into())
        .await?;

      // Reclaim the source holder regardless of result.
      // SAFETY: source_handle was just produced above; the sync JsCallback has returned.
      drop(unsafe { NativeStringHolder::from_raw_handle(source_handle) });

      return Ok(result_handle.map(|h| {
        // SAFETY: callee returned a handle produced by `NativeStringHolder::into_raw_handle`.
        let holder = unsafe { NativeStringHolder::from_raw_handle(h) };
        rolldown_plugin::HookTransformOutput {
          code: Some(holder.into_string()),
          map: rolldown_plugin::HookTransformOutputMap::Omitted,
          side_effects: None,
          module_type: None,
        }
      }));
    }

    // Async zero-copy bridge: JS returns Promise<bigint>; we await it.
    if let Some(cb) = &self.transform_native_bridge_async {
      use std::sync::Arc;
      let source_handle =
        NativeStringHolder::from_arcstr(args.code.clone()).into_raw_handle();
      let _ = Arc::new(());

      let result_handle = cb
        .await_call((source_handle, args.id.to_string()).into())
        .await?;

      drop(unsafe { NativeStringHolder::from_raw_handle(source_handle) });

      return Ok(result_handle.map(|h| {
        let holder = unsafe { NativeStringHolder::from_raw_handle(h) };
        rolldown_plugin::HookTransformOutput {
          code: Some(holder.into_string()),
          map: rolldown_plugin::HookTransformOutputMap::Omitted,
          side_effects: None,
          module_type: None,
        }
      }));
    }
```

(The `let _ = Arc::new(());` line is a defensive no-op so the `use std::sync::Arc;` line doesn't get flagged unused if both branches end up dead-code-eliminated; remove it once the build confirms imports are clean.)

If you used the `MaybeAsyncJsCallback` fallback in Task 7's Step 2, the async-branch's `cb.await_call(...)` call already matches `MaybeAsyncJsCallback`'s extension. If you used `JsCallback<…, Promise<…>>`, you'll need `.await_call` to be defined for that shape — implement a small extension if not already present, or use `cb.invoke_async(...).await?` and then explicitly `.await` the `Promise<…>` it returns.

- [ ] **Step 3: Build**

Run:
```
just build-rolldown-binding
```
Expected: builds.

- [ ] **Step 4: Lint**

Run:
```
cargo clippy -p rolldown_binding --all-targets -- --deny warnings
```
Expected: clean. Fix any clippy items it raises (most likely `clippy::redundant_clone` on `args.code.clone()` — that's a false positive here; either `.clone()` an ArcStr or use `arcstr::ArcStr::clone(&args.code)` explicitly).

- [ ] **Step 5: Commit**

```bash
git add crates/rolldown_binding/src/options/plugin/js_plugin.rs
git commit -m "feat(binding): dispatch transformNativeBridge[Async] before the regular transform in JsPlugin"
```

---

## Task 9: `ParallelJsPlugin::transform` extension

**Files:**
- Modify: `crates/rolldown_binding/src/options/plugin/parallel_js_plugin.rs`

- [ ] **Step 1: OR the new fields into the dispatch condition**

Locate `async fn transform(...)` in `impl Plugin for ParallelJsPlugin`. Replace the existing body with:

```rust
  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    let p = self.first_plugin();
    if p.transform.is_some()
      || p.transform_native_bridge.is_some()
      || p.transform_native_bridge_async.is_some()
    {
      self.run_single(|plugin| Box::pin(Plugin::transform(plugin, ctx, args))).await
    } else {
      Ok(None)
    }
  }
```

- [ ] **Step 2: Build**

Run:
```
just build-rolldown-binding
```

- [ ] **Step 3: Commit**

```bash
git add crates/rolldown_binding/src/options/plugin/parallel_js_plugin.rs
git commit -m "feat(binding): ParallelJsPlugin::transform dispatches when either bridge field is set"
```

---

## Task 10: `NativeLibPlugin` loader + Either3 widening

**Files:**
- Create: `crates/rolldown_binding/src/options/plugin/native_lib_plugin.rs`
- Create: `crates/rolldown_binding/src/options/plugin/binding_native_lib_plugin.rs`
- Modify: `crates/rolldown_binding/src/options/plugin/mod.rs`
- Modify: `crates/rolldown_binding/src/options/plugin/binding_plugin_options.rs` (widen `Either` → `Either3`)
- Modify: `crates/rolldown_binding/src/options/binding_input_options/mod.rs` (ts_type widen)
- Modify: `crates/rolldown_binding/src/options/binding_output_options/mod.rs` (ts_type widen)
- Modify: `crates/rolldown_binding/src/utils/normalize_binding_options.rs` (Either3 match arms)

- [ ] **Step 1: Create the loader**

Create `crates/rolldown_binding/src/options/plugin/native_lib_plugin.rs`:

```rust
use std::borrow::Cow;
use std::sync::Arc;

use anyhow::{Context as _, anyhow};
use libloading::{Library, Symbol};
use rolldown_native_plugin_abi::{
  ABI_VERSION, FnAbiVersion, FnDropOutput, FnTransform, NativeStr, SYM_ABI_VERSION,
  SYM_DROP_OUTPUT, SYM_TRANSFORM, TransformOutput,
};
use rolldown_plugin::{
  HookTransformOutput, HookTransformOutputMap, HookUsage, Plugin, __inner::SharedPluginable,
};

pub struct NativeLibPlugin {
  name: String,
  // Keep the Library alive for the lifetime of this plugin. The fn pointers
  // below are only valid while `_lib` is loaded.
  _lib: Arc<Library>,
  transform: FnTransform,
  drop_output: FnDropOutput,
}

impl std::fmt::Debug for NativeLibPlugin {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("NativeLibPlugin")
      .field("name", &self.name)
      .field("_lib", &"<libloading::Library>")
      .field("transform", &(self.transform as *const ()))
      .field("drop_output", &(self.drop_output as *const ()))
      .finish()
  }
}

impl NativeLibPlugin {
  pub fn load(name: String, path: &str) -> napi::Result<Self> {
    // SAFETY: we trust the user-supplied path. dlopen executes the library's
    // initializers, which is inherently unsafe.
    let lib = unsafe { Library::new(path) }
      .with_context(|| format!("failed to dlopen native plugin: {path}"))
      .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;

    let (transform, drop_output) = unsafe {
      let abi_version: Symbol<FnAbiVersion> = lib
        .get(SYM_ABI_VERSION.as_bytes())
        .with_context(|| format!("missing symbol {SYM_ABI_VERSION} in {path}"))
        .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;
      let v = abi_version();
      if v != ABI_VERSION {
        return Err(napi::Error::from_reason(format!(
          "native plugin {path} reports ABI version {v}, host expects {ABI_VERSION}"
        )));
      }

      let transform: Symbol<FnTransform> = lib
        .get(SYM_TRANSFORM.as_bytes())
        .with_context(|| format!("missing symbol {SYM_TRANSFORM} in {path}"))
        .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;
      let drop_output: Symbol<FnDropOutput> = lib
        .get(SYM_DROP_OUTPUT.as_bytes())
        .with_context(|| format!("missing symbol {SYM_DROP_OUTPUT} in {path}"))
        .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;

      (*transform, *drop_output)
    };

    Ok(Self { name, _lib: Arc::new(lib), transform, drop_output })
  }

  pub fn into_shared(self) -> SharedPluginable {
    Arc::new(self)
  }
}

impl Plugin for NativeLibPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Owned(self.name.clone())
  }

  async fn transform(
    &self,
    _ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    let source = NativeStr { ptr: args.code.as_ptr(), len: args.code.len() };
    let id_bytes = args.id.as_bytes();
    let id = NativeStr { ptr: id_bytes.as_ptr(), len: id_bytes.len() };

    let mut out = TransformOutput::ZEROED;
    // SAFETY: the plugin's `transform` is thread-safe per ABI contract; `out`
    // is a valid pointer to writable storage we own; `source`/`id` live until
    // this call returns.
    let rc = unsafe { (self.transform)(source, id, &raw mut out) };

    if rc != 0 {
      let msg = if out.error.len > 0 {
        // SAFETY: ABI contract says `error` (when non-empty) is valid UTF-8 until drop_output runs.
        unsafe { out.error.as_str() }.to_owned()
      } else {
        format!("native plugin returned error code {rc}")
      };
      unsafe { (self.drop_output)(&raw mut out) };
      return Err(anyhow!(msg));
    }

    // SAFETY: ABI contract says `code` is valid UTF-8 until drop_output runs.
    let owned: String = unsafe { out.code.as_str() }.to_owned();
    unsafe { (self.drop_output)(&raw mut out) };

    Ok(Some(HookTransformOutput {
      code: Some(owned),
      map: HookTransformOutputMap::Omitted,
      side_effects: None,
      module_type: None,
    }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}
```

- [ ] **Step 2: Create the napi descriptor**

Create `crates/rolldown_binding/src/options/plugin/binding_native_lib_plugin.rs`:

```rust
use super::native_lib_plugin::NativeLibPlugin;

/// JS-facing descriptor for a native plugin loaded via the
/// `rolldown_native_plugin_abi` C ABI. The plugin's `.dylib`/`.so`/`.dll` at
/// `path` must export the three required symbols
/// (`rolldown_native_plugin_abi_version`, `rolldown_native_plugin_transform`,
/// `rolldown_native_plugin_drop_output`).
///
/// Plugins are loaded once at bundle setup and dispatched directly from
/// rolldown's worker threads — no napi, no JS thread, no `ThreadsafeFunction`.
#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingNativeLibPlugin {
  /// Display name (used in diagnostics and telemetry).
  pub name: String,
  /// Filesystem path to the plugin shared library.
  pub path: String,
}

impl TryFrom<BindingNativeLibPlugin> for NativeLibPlugin {
  type Error = napi::Error;

  fn try_from(value: BindingNativeLibPlugin) -> Result<Self, Self::Error> {
    NativeLibPlugin::load(value.name, &value.path)
  }
}
```

- [ ] **Step 3: Register the modules**

In `crates/rolldown_binding/src/options/plugin/mod.rs`, add (alphabetically):

```rust
mod binding_native_lib_plugin;
mod native_lib_plugin;
```

(Place `binding_native_lib_plugin` after `binding_load_context;` and `native_lib_plugin` after `js_plugin;`.)

And in the `pub use` block:

```rust
pub use binding_native_lib_plugin::*;
pub use native_lib_plugin::*;
```

(`binding_native_lib_plugin` before `binding_plugin_options`, `native_lib_plugin` after `js_plugin`.)

- [ ] **Step 4: Widen the `Either` to `Either3` in the placeholder**

In `crates/rolldown_binding/src/options/plugin/binding_plugin_options.rs`, replace:

```rust
use napi::bindgen_prelude::{Either, FnArgs};
```

with:

```rust
use napi::bindgen_prelude::{Either3, FnArgs};
```

(Keep the `Promise` import you added in Task 7.)

Then change the placeholder type definition. Replace:

```rust
/// none is parallel js plugin
pub type BindingPluginOrParallelJsPluginPlaceholder<'env> =
  Option<Either<BindingPluginOptions, BindingBuiltinPlugin<'env>>>;
```

with:

```rust
/// none is parallel js plugin
pub type BindingPluginOrParallelJsPluginPlaceholder<'env> = Option<
  Either3<
    BindingPluginOptions,
    super::binding_native_lib_plugin::BindingNativeLibPlugin,
    BindingBuiltinPlugin<'env>,
  >,
>;
```

- [ ] **Step 5: Update the dispatch sites**

In `crates/rolldown_binding/src/utils/normalize_binding_options.rs`, find both `match plugin {` blocks (one under `#[cfg(not(target_family = "wasm"))]` and one under `#[cfg(target_family = "wasm")]`). Replace each occurrence of:

```rust
        |plugin| match plugin {
          Either::A(plugin_options) => JsPlugin::new_shared(plugin_options),
          Either::B(builtin) => {
```

with:

```rust
        |plugin| match plugin {
          Either3::A(plugin_options) => JsPlugin::new_shared(plugin_options),
          Either3::B(native_lib) => native_lib
            .try_into()
            .map(|p: super::super::options::plugin::NativeLibPlugin| p.into_shared()),
          Either3::C(builtin) => {
```

(Keep the existing body of the original `Either::B` arm — now `Either3::C` — verbatim.)

You also need `Either3` in scope. Find the existing import of `Either` and add `Either3` to it:

```rust
use napi::bindgen_prelude::{Either, Either3, FnArgs};
```

- [ ] **Step 6: Widen the ts_type annotations**

In `crates/rolldown_binding/src/options/binding_input_options/mod.rs`, locate:

```rust
  #[napi(ts_type = "(BindingBuiltinPlugin | BindingPluginOptions | undefined)[]")]
  pub plugins: Vec<BindingPluginOrParallelJsPluginPlaceholder<'env>>,
```

Replace with:

```rust
  #[napi(
    ts_type = "(BindingBuiltinPlugin | BindingPluginOptions | BindingNativeLibPlugin | undefined)[]"
  )]
  pub plugins: Vec<BindingPluginOrParallelJsPluginPlaceholder<'env>>,
```

Do the same in `crates/rolldown_binding/src/options/binding_output_options/mod.rs`.

- [ ] **Step 7: Build the binding**

Run:
```
just build-rolldown-binding
```

If you see `error[E0277]: the trait bound BindingNativeLibPlugin: std::fmt::Debug is not satisfied` from the `Either3` derive expansion, ensure `BindingNativeLibPlugin` derives `Debug` (already done in Step 2). If you see "Cannot find name `BindingNativeLibPlugin`" in the generated TS, build full rolldown so the TS gen reruns:

```
just build-rolldown
```

Verify:
```
grep -n "BindingNativeLibPlugin" packages/rolldown/src/binding.d.cts | head -5
```
Expected: the `interface BindingNativeLibPlugin { name: string; path: string }` declaration plus the union sites.

- [ ] **Step 8: Lint**

Run:
```
cargo clippy -p rolldown_binding --all-targets -- --deny warnings
```

If clippy complains about `implicit borrow as raw pointer` on `&raw mut out`, the syntax should be correct on Rust 1.96; if your toolchain doesn't support `&raw mut`, fall back to `std::ptr::from_mut(&mut out)`.

- [ ] **Step 9: Commit**

```bash
git add crates/rolldown_binding/src/options/plugin/native_lib_plugin.rs \
        crates/rolldown_binding/src/options/plugin/binding_native_lib_plugin.rs \
        crates/rolldown_binding/src/options/plugin/mod.rs \
        crates/rolldown_binding/src/options/plugin/binding_plugin_options.rs \
        crates/rolldown_binding/src/utils/normalize_binding_options.rs \
        crates/rolldown_binding/src/options/binding_input_options/mod.rs \
        crates/rolldown_binding/src/options/binding_output_options/mod.rs \
        packages/rolldown/src/binding.cjs packages/rolldown/src/binding.d.cts \
        packages/rolldown/src/rolldown-binding.wasi-browser.js packages/rolldown/src/rolldown-binding.wasi.cjs
git commit -m "feat(binding): NativeLibPlugin loader + Either3 plugin variant chain"
```

---

## Task 11: JS surface for native-lib plugins

**Files:**
- Create: `packages/rolldown/src/plugin/native-lib-plugin.ts`
- Modify: `packages/rolldown/src/plugin/index.ts` — extend `RolldownPlugin` union
- Modify: `packages/rolldown/src/experimental-index.ts` — export `defineNativeLibPlugin`
- Modify: `packages/rolldown/src/utils/bindingify-input-options.ts` — recognize `_nativeLib`

- [ ] **Step 1: Define the marker type and factory**

Create `packages/rolldown/src/plugin/native-lib-plugin.ts`:

```ts
// Marker type for plugins backed by a native shared library that implements
// the rolldown_native_plugin_abi C ABI. The host loads the library via dlopen
// and dispatches transforms directly on its worker threads — no napi callback,
// no JS thread.

export type NativeLibPlugin = {
  _nativeLib: {
    name: string;
    path: string;
  };
};

export function defineNativeLibPlugin(opts: {
  name: string;
  path: string;
}): NativeLibPlugin {
  if (import.meta.browserBuild) {
    throw new Error('`defineNativeLibPlugin` is not supported in browser build');
  }
  return { _nativeLib: { name: opts.name, path: opts.path } };
}
```

- [ ] **Step 2: Extend the `RolldownPlugin` union**

In `packages/rolldown/src/plugin/index.ts`, find:

```ts
import type { ParallelPlugin } from './parallel-plugin';
```

Add immediately above it:

```ts
import type { NativeLibPlugin } from './native-lib-plugin';
```

Then find:

```ts
export type RolldownPlugin<A = any> = Plugin<A> | BuiltinPlugin | ParallelPlugin;
```

Replace with:

```ts
export type RolldownPlugin<A = any> =
  | Plugin<A>
  | BuiltinPlugin
  | ParallelPlugin
  | NativeLibPlugin;
```

- [ ] **Step 3: Re-export from experimental**

In `packages/rolldown/src/experimental-index.ts`, find:

```ts
export { defineParallelPlugin } from './plugin/parallel-plugin';
```

Add immediately above:

```ts
export { defineNativeLibPlugin } from './plugin/native-lib-plugin';
```

- [ ] **Step 4: Recognize `_nativeLib` in bindingify**

In `packages/rolldown/src/utils/bindingify-input-options.ts`, find the block:

```ts
  const plugins = rawPlugins.map((plugin) => {
    if ('_parallel' in plugin) {
      return undefined;
    }
    if (plugin instanceof BuiltinPlugin) {
```

Replace with:

```ts
  const plugins = rawPlugins.map((plugin) => {
    if ('_parallel' in plugin) {
      return undefined;
    }
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    if ('_nativeLib' in (plugin as any)) {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      return (plugin as any)._nativeLib as { name: string; path: string };
    }
    if (plugin instanceof BuiltinPlugin) {
```

- [ ] **Step 5: Build**

Run:
```
just build-rolldown
```
Expected: dist regenerates with `defineNativeLibPlugin` exported.

- [ ] **Step 6: Verify dist has the wiring**

Run:
```
grep -rn "_nativeLib" packages/rolldown/dist | head -5
```
Expected: at least one hit showing the recognition logic compiled.

- [ ] **Step 7: Commit**

```bash
git add packages/rolldown/src/plugin/native-lib-plugin.ts \
        packages/rolldown/src/plugin/index.ts \
        packages/rolldown/src/experimental-index.ts \
        packages/rolldown/src/utils/bindingify-input-options.ts \
        packages/rolldown/dist/
git commit -m "feat(rolldown): defineNativeLibPlugin JS surface"
```

---

## Task 12: JS plumbing — pass bridge fields through bindingify-plugin + hook-usage

**Files:**
- Modify: `packages/rolldown/src/plugin/bindingify-plugin.ts`
- Modify: `packages/rolldown/src/plugin/generated/hook-usage.ts`

- [ ] **Step 1: Pass both bridge fields through**

In `packages/rolldown/src/plugin/bindingify-plugin.ts`, find:

```ts
    transform,
    transformMeta,
    transformFilter,
    moduleParsed,
```

Insert immediately after `transformFilter,`:

```ts
    // Experimental opaque-handle transform hooks. Pass-through; the Rust
    // adapter (JsPlugin::transform) picks these up before the regular path.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    transformNativeBridge: (plugin as any).transformNativeBridge,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    transformNativeBridgeAsync: (plugin as any).transformNativeBridgeAsync,
```

- [ ] **Step 2: Update hook-usage to register transform for the bridge fields**

In `packages/rolldown/src/plugin/generated/hook-usage.ts`, find:

```ts
  if (plugin.transform) {
    hookUsage.union(HookUsageKind.transform);
  }
```

Replace with:

```ts
  if (plugin.transform) {
    hookUsage.union(HookUsageKind.transform);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  if ((plugin as any).transformNativeBridge) {
    hookUsage.union(HookUsageKind.transform);
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  if ((plugin as any).transformNativeBridgeAsync) {
    hookUsage.union(HookUsageKind.transform);
  }
```

- [ ] **Step 3: Build**

Run:
```
just build-rolldown
```

- [ ] **Step 4: Commit**

```bash
git add packages/rolldown/src/plugin/bindingify-plugin.ts \
        packages/rolldown/src/plugin/generated/hook-usage.ts \
        packages/rolldown/dist/
git commit -m "feat(rolldown): pass transformNativeBridge{,Async} through bindingify + hook-usage"
```

---

## Task 13: Parallel-plugin worker keep-alive

**Files:**
- Modify: `packages/rolldown/src/parallel-plugin-worker.ts`

- [ ] **Step 1: Patch the worker**

In `packages/rolldown/src/parallel-plugin-worker.ts`, find the IIFE's tail:

```ts
    registerPlugins(registryId, plugins);

    parentPort!.postMessage({ type: 'success' });
  } catch (error) {
    parentPort!.postMessage({ type: 'error', error });
  } finally {
    parentPort!.unref();
  }
})();
```

Replace with:

```ts
    registerPlugins(registryId, plugins);

    parentPort!.postMessage({ type: 'success' });
  } catch (error) {
    parentPort!.postMessage({ type: 'error', error });
    parentPort!.unref();
    return;
  }
  // Hold the worker alive (poll-style) so the TSFNs that wrap each plugin
  // hook can be dispatched. The main thread terminates each worker explicitly
  // via `worker.terminate()` when the build completes. Required on Node 24.x:
  // without this the worker's JS event loop exits as soon as bootstrap
  // returns, and the first hook dispatch from the main thread gets
  // `Status::Closing`. Reproduces without this patch in
  // `examples/par-plugin/parallel-noop-plugin/`.
  setInterval(() => {}, 1 << 30);
})();
```

- [ ] **Step 2: Build**

Run:
```
just build-rolldown
```

- [ ] **Step 3: Commit**

```bash
git add packages/rolldown/src/parallel-plugin-worker.ts packages/rolldown/dist/
git commit -m "fix(rolldown): keep parallel-plugin worker JS thread alive after bootstrap"
```

---

## Task 14: JS integration test

**Files:**
- Create: `packages/rolldown/tests/native-bridge-plugin.test.ts`

- [ ] **Step 1: Write the test**

Create `packages/rolldown/tests/native-bridge-plugin.test.ts`:

```ts
import { rolldown } from 'rolldown';
import type { Plugin } from 'rolldown';
import { transformSync } from 'rolldown/utils';
import { describe, expect, it } from 'vitest';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const binding = require('../src/binding.cjs') as {
  BenchOxcTransformer: new () => {
    transformNative(sourceHandle: bigint, id: string): bigint;
    transformNativeAsync(sourceHandle: bigint, id: string): Promise<bigint>;
  };
};

const SAMPLE_TSX = `
import * as React from 'react';
export function Counter() {
  const [n, setN] = React.useState(0);
  return <button onClick={() => setN(n + 1)}>{n}</button>;
}
`;

async function runWithBridge(bridgeKind: 'sync' | 'async') {
  const transformer = new binding.BenchOxcTransformer();
  let captured: string | undefined;

  const virtualEntry: Plugin = {
    name: 'virtual',
    resolveId(id) {
      if (id === 'entry.tsx') return id;
      if (id === 'react') return { id, external: true };
      return null;
    },
    load(id) {
      if (id === 'entry.tsx') return { code: SAMPLE_TSX, moduleType: 'tsx' };
      return null;
    },
  };

  const bridgePlugin =
    bridgeKind === 'sync'
      ? ({
          name: 'bridge-sync',
          transformNativeBridge(handle: bigint, id: string) {
            return transformer.transformNative(handle, id);
          },
        } as unknown as Plugin)
      : ({
          name: 'bridge-async',
          transformNativeBridgeAsync(handle: bigint, id: string) {
            return transformer.transformNativeAsync(handle, id);
          },
        } as unknown as Plugin);

  const capture: Plugin = {
    name: 'capture',
    transform(code) {
      captured = code;
      return null;
    },
  };

  const bundle = await rolldown({
    input: 'entry.tsx',
    plugins: [virtualEntry, bridgePlugin, capture],
  });
  await bundle.generate({ format: 'esm' });
  await bundle.close();

  return captured;
}

describe('native-bridge plugin paths', () => {
  it('sync bridge matches rolldown/utils transformSync', async () => {
    const expected = transformSync('Counter.tsx', SAMPLE_TSX, {
      reactCompiler: true,
    }).code;
    const actual = await runWithBridge('sync');
    expect(actual).toBeDefined();
    expect(actual).toBe(expected);
  });

  it('async bridge matches rolldown/utils transformSync', async () => {
    const expected = transformSync('Counter.tsx', SAMPLE_TSX, {
      reactCompiler: true,
    }).code;
    const actual = await runWithBridge('async');
    expect(actual).toBeDefined();
    expect(actual).toBe(expected);
  });
});
```

- [ ] **Step 2: Run the test**

Run:
```
just t-node-rolldown -- native-bridge
```
Expected: both `sync bridge` and `async bridge` tests pass.

If `transformSync` from `rolldown/utils` produces different code than the bridge does (different sourcemap defaults, etc.), inspect the diff. The fix is in `BenchOxcTransformer::run_transform` to match `transformSync`'s defaults. Most likely the difference is sourcemap/comment options — both should be off here.

If the async bridge test hangs, it's the same upstream deadlock as the prior exploration. At LIMIT=15 (3 entry files like this) you should be far under the threshold; if it hangs at this scale that's a regression worth investigating.

- [ ] **Step 3: Commit**

```bash
git add packages/rolldown/tests/native-bridge-plugin.test.ts
git commit -m "test: round-trip transformNativeBridge sync + async vs rolldown/utils.transformSync"
```

---

## Task 15: Bench fixture setup script

**Files:**
- Create: `scripts/bench/seven-way-react-compiler/.gitignore`
- Create: `scripts/bench/seven-way-react-compiler/setup.mjs`

- [ ] **Step 1: gitignore**

Create `scripts/bench/seven-way-react-compiler/.gitignore`:

```
.fixture/
out-*/
corpus.json
*.local.json
*.log
```

- [ ] **Step 2: setup.mjs**

Create `scripts/bench/seven-way-react-compiler/setup.mjs`:

```js
#!/usr/bin/env node
// Sparse-clone Infisical and snapshot the list of frontend source files into corpus.json.

import { execSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const FIXTURE_DIR = resolve(__dirname, '.fixture');
const REPO_DIR = join(FIXTURE_DIR, 'infisical');
const FRONTEND_DIR = join(REPO_DIR, 'frontend');
const CORPUS_JSON = join(__dirname, 'corpus.json');

mkdirSync(FIXTURE_DIR, { recursive: true });

if (!existsSync(REPO_DIR)) {
  console.log('Cloning Infisical (sparse, depth=1)...');
  execSync(
    `git clone --depth=1 --filter=blob:none --sparse https://github.com/Infisical/infisical "${REPO_DIR}"`,
    { stdio: 'inherit' },
  );
  execSync(`git -C "${REPO_DIR}" sparse-checkout set frontend`, { stdio: 'inherit' });
} else {
  console.log('Reusing existing clone at', REPO_DIR);
}

if (!existsSync(FRONTEND_DIR)) {
  throw new Error(`Expected ${FRONTEND_DIR} to exist after sparse-checkout`);
}

const EXT = new Set(['.tsx', '.ts', '.jsx', '.js']);
const SKIP_DIRS = new Set(['node_modules', '.next', 'dist', 'build', '.git']);

const files = [];
function walk(dir) {
  for (const name of readdirSync(dir)) {
    if (SKIP_DIRS.has(name)) continue;
    const p = join(dir, name);
    const s = statSync(p);
    if (s.isDirectory()) walk(p);
    else if (s.isFile()) {
      // Skip TypeScript declaration files — they describe types, not runnable code.
      if (/\.d\.[cm]?ts$/.test(name)) continue;
      const dot = name.lastIndexOf('.');
      if (dot > 0 && EXT.has(name.slice(dot))) {
        files.push(relative(FRONTEND_DIR, p));
      }
    }
  }
}
walk(FRONTEND_DIR);

files.sort();
writeFileSync(
  CORPUS_JSON,
  JSON.stringify({ root: FRONTEND_DIR, files }, null, 2) + '\n',
);
console.log(`Wrote ${files.length} files to ${relative(process.cwd(), CORPUS_JSON)}`);
```

- [ ] **Step 3: Run setup**

Run:
```
node scripts/bench/seven-way-react-compiler/setup.mjs
```
Expected: clone completes in ~30s; "Wrote ~3847 files to scripts/bench/seven-way-react-compiler/corpus.json".

- [ ] **Step 4: Commit**

```bash
git add scripts/bench/seven-way-react-compiler/.gitignore scripts/bench/seven-way-react-compiler/setup.mjs
git commit -m "bench: Infisical sparse-clone setup script for seven-way bench"
```

---

## Task 16: Parallel-plugin impl + bench runner

**Files:**
- Create: `scripts/bench/seven-way-react-compiler/parallel-impl.mjs`
- Create: `scripts/bench/seven-way-react-compiler/run.mjs`

- [ ] **Step 1: parallel-impl.mjs**

Create `scripts/bench/seven-way-react-compiler/parallel-impl.mjs`:

```js
// Parallel-plugin implementation for variant 7 (bridge-parallel), dynamically
// imported by each rolldown worker. One BenchOxcTransformer per worker.

import { defineParallelPluginImplementation } from 'rolldown/parallelPlugin';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const binding = require('../../../packages/rolldown/src/binding.cjs');

export default defineParallelPluginImplementation((_options, _context) => {
  const transformer = new binding.BenchOxcTransformer();
  return {
    name: 'oxc-bench-bridge-parallel',
    transformNativeBridge(sourceHandle, id) {
      if (!id.endsWith('.tsx') && !id.endsWith('.jsx')) return undefined;
      try {
        return transformer.transformNative(sourceHandle, id);
      } catch {
        return undefined;
      }
    },
  };
});
```

- [ ] **Step 2: run.mjs**

Create `scripts/bench/seven-way-react-compiler/run.mjs`:

```js
#!/usr/bin/env node
// Bench seven variants of the same React Compiler transform on the Infisical
// frontend corpus. See docs/superpowers/specs/2026-06-20-seven-way-react-compiler-bench-design.md.

import { existsSync, readFileSync, rmSync } from 'node:fs';
import { createRequire } from 'node:module';
import { performance } from 'node:perf_hooks';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { rolldown } from 'rolldown';
import { defineNativeLibPlugin, defineParallelPlugin } from 'rolldown/experimental';
import { transform as utilsTransform, transformSync as utilsTransformSync } from 'rolldown/utils';

const require = createRequire(import.meta.url);
const binding = require('../../../packages/rolldown/src/binding.cjs');

const __dirname = dirname(fileURLToPath(import.meta.url));
const CORPUS_JSON = join(__dirname, 'corpus.json');
const FIXTURE_DIR = join(__dirname, '.fixture');

if (!existsSync(CORPUS_JSON)) {
  console.error('corpus.json not found. Run setup.mjs first.');
  process.exit(1);
}

const ITERATIONS = Number(process.env.ITERS ?? 6);
const corpus = JSON.parse(readFileSync(CORPUS_JSON, 'utf8'));
const ROOT = corpus.root;
const FILES = corpus.files.slice(0, Number(process.env.LIMIT ?? corpus.files.length));

console.log(`corpus: ${FILES.length} files under ${ROOT}`);
console.log(`iterations: ${ITERATIONS} (1 warm-up dropped, ${ITERATIONS - 1} measured)`);

const ENTRY_ID = '\0seven-way-bench:entry';
const SOURCE_EXTS = ['.tsx', '.ts', '.jsx', '.js', '.mjs', '.cjs'];
const isBareSpecifier = (s) =>
  !!s && !s.startsWith('.') && !s.startsWith('/') && !s.startsWith('\0');
const looksLikeSourceImport = (s) =>
  SOURCE_EXTS.some((ext) => s.endsWith(ext)) || !/\.[a-z0-9]+$/i.test(s);

const shouldTransform = (id) => id.endsWith('.tsx') || id.endsWith('.jsx');

function makeBasePlugins() {
  const entrySource = FILES.map((f) => `import ${JSON.stringify(join(ROOT, f))};`).join('\n');
  return [
    {
      name: 'virtual-entry',
      resolveId(id) {
        if (id === ENTRY_ID) return id;
        if (isBareSpecifier(id)) return { id, external: true };
        if (!looksLikeSourceImport(id)) return { id, external: true };
        return null;
      },
      load(id) {
        if (id === ENTRY_ID) return entrySource;
        return null;
      },
    },
  ];
}

// --- Variant 1: utils-sync ---
function utilsSyncPlugin() {
  return {
    name: 'oxc-bench-utils-sync',
    transform(code, id) {
      if (!shouldTransform(id)) return null;
      try {
        return utilsTransformSync(id, code, { reactCompiler: true }).code;
      } catch {
        return null;
      }
    },
  };
}

// --- Variant 2: utils-async ---
function utilsAsyncPlugin() {
  return {
    name: 'oxc-bench-utils-async',
    async transform(code, id) {
      if (!shouldTransform(id)) return null;
      try {
        const r = await utilsTransform(id, code, { reactCompiler: true });
        return r.code;
      } catch {
        return null;
      }
    },
  };
}

// --- Variants 3 + 4: bridge sync / async ---
const transformer = new binding.BenchOxcTransformer();

function bridgeSyncPlugin() {
  return {
    name: 'oxc-bench-bridge-sync',
    transformNativeBridge(sourceHandle, id) {
      if (!shouldTransform(id)) return undefined;
      try {
        return transformer.transformNative(sourceHandle, id);
      } catch {
        return undefined;
      }
    },
  };
}

function bridgeAsyncPlugin() {
  return {
    name: 'oxc-bench-bridge-async',
    transformNativeBridgeAsync(sourceHandle, id) {
      if (!shouldTransform(id)) return Promise.resolve(undefined);
      return transformer.transformNativeAsync(sourceHandle, id).catch(() => undefined);
    },
  };
}

// --- Variant 5: native-lib ---
const NATIVE_LIB_PATH = process.env.NATIVE_LIB_PATH ?? resolve(
  __dirname,
  '../../../target/release/libbench_native_lib_plugin.dylib',
);
function nativeLibPlugin() {
  return defineNativeLibPlugin({ name: 'oxc-bench-native-lib', path: NATIVE_LIB_PATH });
}

// --- Variant 7: bridge-parallel ---
const PARALLEL_IMPL = resolve(__dirname, 'parallel-impl.mjs');
const makeParallelPlugin = defineParallelPlugin(PARALLEL_IMPL);
function bridgeParallelPlugin() {
  return makeParallelPlugin({});
}

// Variant 6 (builtin) doesn't append a transform plugin; it sets the bundler-level
// `transform.reactCompiler` option instead.

async function runOnce(variant) {
  const basePlugins = makeBasePlugins();
  let transformPlugin;
  let bundlerTransform;

  switch (variant) {
    case 'utils-sync':
      transformPlugin = utilsSyncPlugin();
      break;
    case 'utils-async':
      transformPlugin = utilsAsyncPlugin();
      break;
    case 'bridge-sync':
      transformPlugin = bridgeSyncPlugin();
      break;
    case 'bridge-async':
      transformPlugin = bridgeAsyncPlugin();
      break;
    case 'native-lib':
      transformPlugin = nativeLibPlugin();
      break;
    case 'bridge-parallel':
      transformPlugin = bridgeParallelPlugin();
      break;
    case 'builtin':
      transformPlugin = null;
      bundlerTransform = { reactCompiler: true };
      break;
    default:
      throw new Error(`unknown variant: ${variant}`);
  }

  const plugins = transformPlugin ? [...basePlugins, transformPlugin] : basePlugins;

  const t0 = performance.now();
  const bundle = await rolldown({
    input: ENTRY_ID,
    plugins,
    transform: bundlerTransform,
    logLevel: 'silent',
  });
  await bundle.generate({ format: 'esm' });
  await bundle.close();
  return performance.now() - t0;
}

function stats(samples) {
  const sorted = [...samples].sort((a, b) => a - b);
  const min = sorted[0];
  const med = sorted[Math.floor(sorted.length / 2)];
  const p95 = sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))];
  const mean = samples.reduce((a, b) => a + b, 0) / samples.length;
  return { min, med, p95, mean };
}

async function benchVariant(name) {
  console.log(`\n--- variant: ${name} ---`);
  const samples = [];
  for (let i = 0; i < ITERATIONS; i++) {
    rmSync(join(FIXTURE_DIR, `out-${name}`), { recursive: true, force: true });
    const ms = await runOnce(name);
    if (i === 0) {
      console.log(`  warm-up: ${ms.toFixed(1)} ms`);
    } else {
      console.log(`  iter ${i}: ${ms.toFixed(1)} ms`);
      samples.push(ms);
    }
  }
  return stats(samples);
}

const variants = (
  process.env.VARIANTS ?? 'utils-sync,bridge-sync,native-lib,builtin,bridge-parallel'
)
  .split(',')
  .map((v) => v.trim())
  .filter(Boolean);

const results = {};
for (const v of variants) {
  results[v] = await benchVariant(v);
}

console.log('\n--- summary (lower is better) ---');
for (const v of variants) {
  console.log(`${v.padEnd(16)}:`, results[v]);
}
const baseline = results['utils-sync'];
if (baseline) {
  for (const v of variants) {
    if (v === 'utils-sync') continue;
    const medX = (baseline.med / results[v].med).toFixed(3);
    const minX = (baseline.min / results[v].min).toFixed(3);
    console.log(`speedup utils-sync→${v.padEnd(16)} median: ${medX}x  min: ${minX}x`);
  }
}
```

- [ ] **Step 3: Sanity-run with small limits**

Run:
```
LIMIT=10 ITERS=3 node scripts/bench/seven-way-react-compiler/run.mjs
```

Expected: completes within ~30s, reports timings for `utils-sync`, `bridge-sync`, `native-lib`, `builtin`, `bridge-parallel`.

If the `native-lib` variant fails with "missing symbol" or "failed to dlopen", build the cdylib in debug mode first:
```
cargo build -p bench_native_lib_plugin
NATIVE_LIB_PATH=target/debug/libbench_native_lib_plugin.dylib LIMIT=10 ITERS=3 \
  node scripts/bench/seven-way-react-compiler/run.mjs
```

- [ ] **Step 4: Sanity-run async variants at LIMIT=10**

Run:
```
VARIANTS=utils-async,bridge-async LIMIT=10 ITERS=2 \
  node scripts/bench/seven-way-react-compiler/run.mjs
```
Expected: both complete (they only deadlock above ~16 in-flight).

- [ ] **Step 5: Commit**

```bash
git add scripts/bench/seven-way-react-compiler/parallel-impl.mjs \
        scripts/bench/seven-way-react-compiler/run.mjs
git commit -m "bench: seven-variant runner script + parallel-impl"
```

---

## Task 17: Release builds + primary + secondary bench

**Files:**
- Create: `scripts/bench/seven-way-react-compiler/results.md`

- [ ] **Step 1: Release build rolldown**

Run:
```
just build-rolldown-release
```
Expected: `Finished release profile [optimized]` for the binding and dist regenerates with the release `.node`.

- [ ] **Step 2: Release build the cdylib**

Run:
```
cargo build --release -p bench_native_lib_plugin
```
Expected: `target/release/libbench_native_lib_plugin.dylib` exists.

- [ ] **Step 3: Run the primary table (full corpus, sync variants)**

Run:
```
ITERS=6 node scripts/bench/seven-way-react-compiler/run.mjs 2>&1 | tee /tmp/seven-way-primary.log
```
Expected: completes in ~1.5–3 minutes per variant × 5 variants ≈ 10–20 minutes total. Each variant prints warm-up + 5 iterations, then the summary block prints the stats and speedups vs `utils-sync`.

- [ ] **Step 4: Run the secondary table (LIMIT=15, all seven variants)**

Run:
```
LIMIT=15 ITERS=6 VARIANTS=utils-sync,utils-async,bridge-sync,bridge-async,native-lib,builtin,bridge-parallel \
  node scripts/bench/seven-way-react-compiler/run.mjs 2>&1 | tee /tmp/seven-way-secondary.log
```
Expected: completes within ~1 minute total (15 files per iteration is fast).

If `utils-async` or `bridge-async` hangs at LIMIT=15, drop to LIMIT=10 and rerun. Document the threshold in `results.md`.

- [ ] **Step 5: Write results.md**

Create `scripts/bench/seven-way-react-compiler/results.md`:

```markdown
# Seven-Way React Compiler Bench Results

**Date:** <today's date>
**Machine:** <`uname -a` + `sysctl -n machdep.cpu.brand_string` on macOS, or `lscpu | head -3` on Linux>
**Rolldown commit:** <`git rev-parse --short HEAD`>
**Binding build:** release (`just build-rolldown-release`)
**Plugin cdylib build:** release (`cargo build --release -p bench_native_lib_plugin`)
**Corpus:** Infisical `frontend/` — <N> source files (`.d.ts` filtered out)
**Iterations:** 6 (1 warm-up dropped, 5 measured)

## Variants

- **utils-sync** — JS plugin's `transform` hook calls `transformSync` from `rolldown/utils` with `reactCompiler: true`.
- **utils-async** — JS plugin's `async transform` hook awaits `transform` from `rolldown/utils`.
- **bridge-sync** — JS plugin's `transformNativeBridge` hook receives a `bigint` handle wrapping `Box<NativeStringHolder>`. Calls `BenchOxcTransformer.transformNative`.
- **bridge-async** — JS plugin's `transformNativeBridgeAsync` returns `Promise<bigint>`. Calls `BenchOxcTransformer.transformNativeAsync`.
- **native-lib** — `defineNativeLibPlugin({ path })` loads `bench_native_lib_plugin.dylib`. Dispatch direct from rolldown's worker threads via the `rolldown_native_plugin_abi` C ABI. No napi, no JS thread.
- **builtin** — no plugin; bundler-level `transform.reactCompiler = true`. Theoretical floor.
- **bridge-parallel** — `bridge-sync` registered via `defineParallelPlugin`. ~8 JS worker threads each calling `transformNative` in parallel.

## Primary table — full corpus (sync variants)

```
<paste contents of /tmp/seven-way-primary.log here>
```

| Variant | min (ms) | median (ms) | p95 (ms) | mean (ms) | speedup vs utils-sync |
|---|---:|---:|---:|---:|---:|
| utils-sync | | | | | 1.00x |
| bridge-sync | | | | | |
| native-lib | | | | | |
| builtin | | | | | |
| bridge-parallel | | | | | |

## Secondary table — LIMIT=15, all seven

```
<paste contents of /tmp/seven-way-secondary.log here>
```

| Variant | min (ms) | median (ms) | p95 (ms) | mean (ms) | speedup vs utils-sync |
|---|---:|---:|---:|---:|---:|
| utils-sync | | | | | 1.00x |
| utils-async | | | | | |
| bridge-sync | | | | | |
| bridge-async | | | | | |
| native-lib | | | | | |
| builtin | | | | | |
| bridge-parallel | | | | | |

## Reading the numbers

<paragraph explaining what stands out. Some prompts:
- How big is the gap between utils-sync and bridge-sync? If small, the bridge layer's UTF-conversion savings are dwarfed by per-module React Compiler cost on this workload.
- How does native-lib compare to bridge-sync? native-lib skips napi entirely — if the gap is wider than expected, the napi dispatch is non-trivial; if narrower, dispatch cost is small relative to transform cost.
- How does builtin compare to native-lib? builtin skips one parse cycle; the gap measures the plugin pipeline's own overhead.
- How does bridge-parallel compare to bridge-sync? Should be ~N-way faster up to the per-pipeline serial bottleneck (resolve, codegen).
- In the secondary table, do utils-async and bridge-async show ~1.5x speedups at small scale? That's the JS-thread-freeing win when async dispatch works.
>

## Caveats

- mimalloc may emit "invalid pointer" warnings throughout — pre-existing rolldown/oxc allocation pattern, not caused by any variant here.
- The async variants (`utils-async`, `bridge-async`) deadlock above ~16 concurrent in-flight transforms on Node 20+/22+/24+ — a generic napi-rs 3.x `async fn` ↔ tokio interaction. Only the secondary table runs them.
- React Compiler is the only transform; heavier transforms would amplify the parallelism win, trivially cheap ones would shrink it.
```

Fill in the bracketed sections with real data from the logs.

- [ ] **Step 6: Commit results**

```bash
git add scripts/bench/seven-way-react-compiler/results.md
git commit -m "bench(results): seven-way React Compiler bench on Infisical frontend"
```

---

## Self-review checklist (after all tasks)

- [ ] Spec section "Variant matrix" — every variant has a runner + an entry in `runOnce`'s switch.
- [ ] Spec "Implementation surface" items 1–18 each map to a task above.
- [ ] Spec "Benchmark methodology" prerequisites — Task 17 runs `just build-rolldown-release` before benching.
- [ ] Spec "Success criteria" 1–6 — all map to a Run/build step that produces visible output.
- [ ] No `transformNativeBridge` filter handling on the new fields (intentional — see spec scope).
- [ ] Both bridge fields' result paths drop the source holder before reclaiming the result holder (verify in `js_plugin.rs`).
- [ ] `results.md` notes the LIMIT=15 (or 10) threshold actually used for the secondary table.
- [ ] The `bench_native_lib_plugin` cdylib is built in **release** for the bench (debug-mode dispatch costs would mask the variant comparison).
- [ ] Task 13's `setInterval(() => {}, 1 << 30)` is in place; the bench's `bridge-parallel` variant doesn't deadlock during setup.
