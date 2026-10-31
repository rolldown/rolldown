//! Test-only path evidence. See internal-docs/chunk-ast-minify/implementation.md.

use std::{
  fs::OpenOptions,
  io::Write as _,
  sync::{LazyLock, Mutex},
};

use rolldown_common::{MinifyOptions, NormalizedBundlerOptions};

static OUTPUT: LazyLock<Option<Mutex<std::fs::File>>> = LazyLock::new(|| {
  std::env::var_os("ROLLDOWN_CHUNK_AST_COVERAGE").map(|path| {
    Mutex::new(
      OpenOptions::new().create(true).append(true).open(path).expect("open path coverage log"),
    )
  })
});

pub(in crate::stages::generate_stage) fn record(
  options: &NormalizedBundlerOptions,
  stage: &str,
  reason: &str,
  chunk: Option<&str>,
) {
  let Some(output) = &*OUTPUT else { return };
  let minify = match &options.minify {
    MinifyOptions::DeadCodeEliminationOnly(options) => Some(options),
    MinifyOptions::Enabled(options) => Some(&options.options),
    MinifyOptions::Disabled => None,
  };
  let event = serde_json::json!({
    "cwd": options.cwd,
    "out_dir": options.out_dir,
    "format": format!("{:?}", options.format),
    "stage": stage,
    "reason": reason,
    "chunk": chunk,
    "compress": minify.is_some_and(|options| options.compress.is_some()),
    "mangle": minify.is_some_and(|options| options.mangle.is_some()),
    "mangle_properties": minify.is_some_and(|options| options.mangle_properties.is_some()),
  });
  let mut output = output.lock().expect("path coverage lock");
  serde_json::to_writer(&mut *output, &event).expect("write path coverage record");
  writeln!(output).expect("finish path coverage record");
}
