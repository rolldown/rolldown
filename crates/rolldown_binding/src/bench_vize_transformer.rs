//! `BenchVizeTransformer` — the Vue-bench analog to `BenchOxcTransformer`.
//!
//! Vize pins OXC at a forked git rev that conflicts with rolldown's OXC
//! version, so we can't depend on Vize crates directly from rolldown_binding.
//! Instead, we `dlopen` a standalone cdylib (`bench_vize_sfc_lib`, built from
//! `scripts/bench/seven-way-vue/native/`) that statically links Vize and
//! exports a tiny C ABI matching `rolldown_native_plugin_abi`.
//!
//! The handle-passing contract is identical to `BenchOxcTransformer`: a
//! `NativeStringHolder` carrying (source, id) is handed across the napi
//! boundary as an `i64` bigint, the napi method reads through the handle
//! (zero-copy on the JS side), dispatches into Vize via the FFI pointer
//! pair, and returns a new handle wrapping the compiled output.

use std::sync::Arc;

use anyhow::Context as _;
use libloading::{Library, Symbol};
use napi_derive::napi;
use rolldown_native_plugin_abi::{
  ABI_VERSION, FnAbiVersion, FnDropOutput, FnTransform, NativeStr, SYM_ABI_VERSION,
  SYM_DROP_OUTPUT, SYM_TRANSFORM, TransformOutput,
};

use crate::native_bridge::NativeStringHolder;

#[napi]
pub struct BenchVizeTransformer {
  // Keep the Library alive; the fn pointers below are only valid while it is.
  _lib: Arc<Library>,
  transform: FnTransform,
  drop_output: FnDropOutput,
}

#[napi]
impl BenchVizeTransformer {
  /// `libPath` points to `libbench_vize_sfc_lib.dylib` (or `.so`/`.dll`) built
  /// from `scripts/bench/seven-way-vue/native/`. dlopens it and resolves the
  /// three ABI symbols once at construction; subsequent calls go through raw
  /// fn pointers.
  #[napi(constructor)]
  pub fn new(lib_path: String) -> napi::Result<Self> {
    // SAFETY: dlopen runs the library's initializers; user-supplied path.
    let lib = unsafe { Library::new(&lib_path) }
      .with_context(|| format!("failed to dlopen Vize bench cdylib: {lib_path}"))
      .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;

    let (transform, drop_output) = unsafe {
      let abi_version: Symbol<FnAbiVersion> = lib
        .get(SYM_ABI_VERSION.as_bytes())
        .with_context(|| format!("missing symbol {SYM_ABI_VERSION} in {lib_path}"))
        .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;
      let v = abi_version();
      if v != ABI_VERSION {
        return Err(napi::Error::from_reason(format!(
          "Vize cdylib reports ABI version {v}, host expects {ABI_VERSION}"
        )));
      }

      let transform: Symbol<FnTransform> = lib
        .get(SYM_TRANSFORM.as_bytes())
        .with_context(|| format!("missing symbol {SYM_TRANSFORM} in {lib_path}"))
        .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;
      let drop_output: Symbol<FnDropOutput> = lib
        .get(SYM_DROP_OUTPUT.as_bytes())
        .with_context(|| format!("missing symbol {SYM_DROP_OUTPUT} in {lib_path}"))
        .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;

      (*transform, *drop_output)
    };

    Ok(Self { _lib: Arc::new(lib), transform, drop_output })
  }

  #[napi(
    ts_args_type = "sourceHandle: bigint",
    ts_return_type = "bigint | null"
  )]
  pub fn transform_native(&self, source_handle: i64) -> Option<i64> {
    // SAFETY: caller supplies a handle produced by
    // `NativeStringHolder::into_raw_handle{_with_id}` whose box is alive.
    let holder = unsafe { NativeStringHolder::handle_as_ref(source_handle) };
    self.transform_inner(holder)
  }

  #[napi(
    ts_args_type = "sourceHandle: bigint",
    ts_return_type = "Promise<bigint | null>"
  )]
  pub async fn transform_native_async(&self, source_handle: i64) -> Option<i64> {
    napi::tokio::task::yield_now().await;
    // SAFETY: same contract as `transform_native`.
    let holder = unsafe { NativeStringHolder::handle_as_ref(source_handle) };
    self.transform_inner(holder)
  }
}

impl BenchVizeTransformer {
  /// Skip-or-compile policy:
  /// - non-`.vue` id ⇒ return `None` (no transform; rolldown keeps the
  ///   original source — this matters because rolldown's internal runtime
  ///   module also flows through transform hooks).
  /// - Vize compile success ⇒ wrap output in a new holder.
  /// - Vize compile failure for a `.vue` id ⇒ emit a stub (the original .vue
  ///   source isn't valid TS for rolldown's parser, so we can't fall back to
  ///   "leave unchanged"). Matches the JS-side utils variants.
  fn transform_inner(&self, holder: &NativeStringHolder) -> Option<i64> {
    let id_str = holder.id_str();
    if !id_str.ends_with(".vue") {
      return None;
    }
    let output = self
      .invoke_vize(holder.as_str(), id_str)
      .unwrap_or_else(|_| "export default {};\n".to_string());
    Some(NativeStringHolder::from_string(output).into_raw_handle())
  }

  fn invoke_vize(&self, source_str: &str, id_str: &str) -> anyhow::Result<String> {
    let source = NativeStr { ptr: source_str.as_ptr(), len: source_str.len() };
    let id_bytes = id_str.as_bytes();
    let id = NativeStr { ptr: id_bytes.as_ptr(), len: id_bytes.len() };

    let mut out = TransformOutput::ZEROED;
    // SAFETY: ABI says transform is thread-safe; `out` is a valid pointer to
    // writable storage we own; `source`/`id` outlive the call.
    let rc = unsafe { (self.transform)(source, id, &raw mut out) };
    if rc != 0 {
      let msg = if out.error.len > 0 {
        // SAFETY: ABI says `error` is valid UTF-8 until drop_output runs.
        unsafe { out.error.as_str() }.to_owned()
      } else {
        format!("Vize transform returned error code {rc}")
      };
      unsafe { (self.drop_output)(&raw mut out) };
      anyhow::bail!(msg);
    }

    // SAFETY: ABI says `code` is valid UTF-8 until drop_output runs.
    let owned: String = unsafe { out.code.as_str() }.to_owned();
    unsafe { (self.drop_output)(&raw mut out) };
    Ok(owned)
  }
}
