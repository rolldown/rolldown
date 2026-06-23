use std::path::Path;

use arcstr::ArcStr;
use napi_derive::napi;
use oxc::allocator::Allocator;
use oxc::codegen::{Codegen, CodegenOptions, CodegenReturn};
use oxc::diagnostics::Severity as OxcSeverity;
use oxc::parser::{ParseOptions, Parser};
use oxc::span::SourceType;
use oxc::transformer::{TransformOptions, Transformer};
use rolldown_ecmascript::semantic_builder_for_transform;
use rolldown_error::{BuildDiagnostic, EventKind, Severity};

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
  /// `Promise<string>`. Yields once before the CPU work so the napi async
  /// machinery treats it as truly async.
  #[napi]
  pub async fn transform_str_async(&self, source: String, id: String) -> String {
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
    // `default_plugin_options()` uses `panic_threshold: "none"` — matches the
    // JS-side plugins which pass `{ panicThreshold: 'none' }` explicitly.
    react_compiler: Some(oxc_react_compiler::default_plugin_options()),
    ..Default::default()
  };

  let transform_ret = Transformer::new(&allocator, path, &transform_options)
    .build_with_scoping(scoping, &mut program);

  // Match the per-module work that `pre_process_ecma_ast.rs` does for the
  // `builtin` variant: convert each oxc diagnostic into a `BuildDiagnostic`
  // (the costly part — source-snippet refs + message/label string clones).
  // We drop the result; rolldown's `builtin` path appends them to a
  // bundle-scope `warnings` Vec, which is amortized across the whole build.
  if !transform_ret.diagnostics.is_empty() {
    let source_arc = ArcStr::from(source);
    let (errors, warnings): (Vec<_>, Vec<_>) = transform_ret
      .diagnostics
      .into_iter()
      .partition(|d| d.severity == OxcSeverity::Error);
    let _converted_errors = BuildDiagnostic::from_oxc_diagnostics(
      errors,
      &source_arc,
      id,
      Severity::Error,
      EventKind::TransformError,
    );
    let _converted_warnings = BuildDiagnostic::from_oxc_diagnostics(
      warnings,
      &source_arc,
      id,
      Severity::Warning,
      EventKind::ToleratedTransform,
    );
  }

  let codegen_ret: CodegenReturn =
    Codegen::new().with_options(CodegenOptions::default()).build(&program);

  codegen_ret.code
}

#[cfg(test)]
mod tests {
  use super::*;

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
  }
}
