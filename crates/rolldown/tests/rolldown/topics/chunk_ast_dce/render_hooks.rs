use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_common::{
  AttachDebugInfo, CommentsOptions, ExperimentalOptions, Output, OutputFormat, RawMinifyOptions,
};
use rolldown_plugin::{
  HookRenderChunkArgs, HookRenderChunkOutput, HookRenderChunkReturn, HookUsage, Plugin,
  PluginContext,
};
use std::{
  borrow::Cow,
  path::Path,
  process::Command,
  sync::{Arc, Mutex},
};

#[derive(Debug)]
struct Observe {
  replace: bool,
  observations: Arc<Mutex<Vec<String>>>,
}
impl Plugin for Observe {
  fn name(&self) -> Cow<'static, str> {
    "observe-unoptimized-chunk".into()
  }
  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::RenderChunk
  }
  async fn render_chunk(
    &self,
    _ctx: &PluginContext,
    args: &HookRenderChunkArgs<'_>,
  ) -> HookRenderChunkReturn {
    self.observations.lock().unwrap().push(args.code.to_string());
    assert!(!args.code.is_empty());
    Ok(self.replace.then(|| HookRenderChunkOutput {
      code: format!("{}\nglobalThis.__hookResult = 6 * 7;", args.code),
      map: rolldown_plugin::HookTransformOutputMap::default(),
    }))
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn unchanged_and_replaced_chunks_optimize_after_hooks() {
  let fixture =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/rolldown/topics/chunk_ast_dce/path_coverage");
  let pipeline = "chunk-ast";
  for format in [OutputFormat::Esm, OutputFormat::Cjs, OutputFormat::Iife, OutputFormat::Umd] {
    for replace in [false, true] {
      let observations = Arc::new(Mutex::new(Vec::new()));
      let output = Bundler::with_plugins(
        BundlerOptions {
          cwd: Some(fixture.clone()),
          input: Some(vec![InputItem {
            name: Some("main".into()),
            import: "./cross/main.js".into(),
          }]),
          format: Some(format),
          name: Some("HookEntry".into()),
          entry_filenames: Some("main.cjs".to_string().into()),
          sourcemap: Some(SourceMapType::File),
          minify: Some(RawMinifyOptions::Bool(true)),
          ..Default::default()
        },
        vec![Plugin::new_shared(Observe { replace, observations: Arc::clone(&observations) })],
      )
      .unwrap()
      .generate()
      .await
      .unwrap();
      let observations = observations.lock().unwrap();
      assert_eq!(observations.len(), 1);
      assert!(observations[0].contains("cross-inlined-value"));
      let chunk = output
        .assets
        .iter()
        .find_map(|asset| if let Output::Chunk(chunk) = asset { Some(chunk) } else { None })
        .unwrap();
      assert_ne!(observations[0], chunk.code);
      assert!(!chunk.code.contains("6 * 7"));
      let map = chunk.map.as_ref().unwrap();
      super::map_assertions::assert_mapping(
        &chunk.code,
        map,
        "cross-main-anchor",
        "cross/main.js",
        include_str!("path_coverage/cross/main.js"),
        "'cross-main-anchor'",
      );
      let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tmp/adoption-audit/render-hooks")
        .join(pipeline)
        .join(format!("{format:?}-{replace}"));
      std::fs::create_dir_all(&directory).unwrap();
      let filename = if matches!(format, OutputFormat::Esm) { "main.mjs" } else { "main.cjs" };
      std::fs::write(directory.join(filename), &chunk.code).unwrap();
      let runner = directory.join("execute.mjs");
      let load = match format {
        OutputFormat::Esm => "await import('./main.mjs')",
        OutputFormat::Iife => {
          "(vm.runInThisContext(readFileSync(new URL('./main.cjs', import.meta.url), 'utf8')), globalThis.HookEntry)"
        }
        _ => "createRequire(import.meta.url)('./main.cjs')",
      };
      std::fs::write(&runner, format!("import assert from 'node:assert/strict';\nimport {{ createRequire }} from 'node:module';\nimport {{ readFileSync }} from 'node:fs';\nimport vm from 'node:vm';\nglobalThis.__pathEvents = [];\nconst entry = {load};\nassert.equal(entry.result.sum, 42);\nassert.equal(entry.result.value, 'cross-inlined-value');\nassert.deepEqual(globalThis.__pathEvents, ['cross-dep-anchor', 'cross-main-anchor']);\nassert.equal(globalThis.__hookResult, {});\n", if replace { "42" } else { "undefined" })).unwrap();
      let result = Command::new("node").arg(runner).output().unwrap();
      assert!(
        result.status.success(),
        "{pipeline}/{format:?}/{replace}: {}",
        String::from_utf8_lossy(&result.stderr)
      );
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn unchanged_hook_preserves_legal_debug_header() {
  let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("tests/rolldown/topics/chunk_ast_dce/generated_glue");
  for format in [OutputFormat::Esm, OutputFormat::Cjs] {
    for legal in [false, true] {
      let observations = Arc::new(Mutex::new(Vec::new()));
      let output = Bundler::with_plugins(
        BundlerOptions {
          cwd: Some(fixture.clone()),
          input: Some(vec![InputItem {
            name: Some("main".into()),
            import: "./directives.js".into(),
          }]),
          format: Some(format),
          minify: Some(RawMinifyOptions::Bool(true)),
          comments: Some(CommentsOptions { legal, annotation: false, jsdoc: false }),
          experimental: Some(ExperimentalOptions {
            attach_debug_info: Some(AttachDebugInfo::Full),
            ..Default::default()
          }),
          ..Default::default()
        },
        vec![Plugin::new_shared(Observe {
          replace: false,
          observations: Arc::clone(&observations),
        })],
      )
      .unwrap()
      .generate()
      .await
      .unwrap();
      let observations = observations.lock().unwrap();
      assert_eq!(observations.len(), 1);
      let header = observations[0].lines().next().unwrap();
      assert!(header.starts_with("//! "), "{header}");
      let chunk = output
        .assets
        .iter()
        .find_map(|asset| if let Output::Chunk(chunk) = asset { Some(chunk) } else { None })
        .unwrap();
      assert_eq!(chunk.code.contains(header), legal, "{format:?}/{legal}: {}", chunk.code);
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn unchanged_hook_keeps_directive_prologue() {
  let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("tests/rolldown/topics/chunk_ast_dce/generated_glue");
  for preserve_modules in [false, true] {
    let output = Bundler::with_plugins(
      BundlerOptions {
        cwd: Some(fixture.clone()),
        input: Some(vec![InputItem {
          name: Some("main".into()),
          import: "./directives.js".into(),
        }]),
        format: Some(OutputFormat::Cjs),
        preserve_modules: Some(preserve_modules),
        entry_filenames: Some("directives.cjs".to_string().into()),
        minify: Some(RawMinifyOptions::Bool(true)),
        ..Default::default()
      },
      vec![Plugin::new_shared(Observe { replace: false, observations: Arc::default() })],
    )
    .unwrap()
    .generate()
    .await
    .unwrap();
    let chunk = output
      .assets
      .iter()
      .find_map(|asset| if let Output::Chunk(chunk) = asset { Some(chunk) } else { None })
      .unwrap();
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../../tmp/adoption-audit/render-hooks/directives")
      .join(format!("chunk-ast-{preserve_modules}"));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("directives.cjs"), &chunk.code).unwrap();
    let runner = directory.join("execute.cjs");
    std::fs::write(&runner, "const assert = require('node:assert/strict');\nconst { strictThis } = require('./directives.cjs');\nassert.equal(strictThis(), undefined);\n").unwrap();
    let result = Command::new("node").arg(runner).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
  }
}
