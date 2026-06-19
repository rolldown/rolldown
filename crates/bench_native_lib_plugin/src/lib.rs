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
    // `default_plugin_options()` uses `panic_threshold: "none"` — matches the
    // JS-side plugins which pass `{ panicThreshold: 'none' }` explicitly.
    react_compiler: Some(oxc_react_compiler::default_plugin_options()),
    ..Default::default()
  };

  let _ = Transformer::new(&allocator, path, &transform_options)
    .build_with_scoping(scoping, &mut program);

  let codegen_ret: CodegenReturn =
    Codegen::new().with_options(CodegenOptions::default()).build(&program);

  codegen_ret.code
}
