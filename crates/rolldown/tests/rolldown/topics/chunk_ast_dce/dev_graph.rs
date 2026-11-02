use std::{path::Path, process::Command};

use rolldown::{
  Bundler, BundlerOptions, DevModeOptions, ExperimentalOptions, InputItem, SourceMapType,
};
use rolldown_common::{
  CodeSplittingMode, InlineCommonChunksOptions, ManualCodeSplittingOptions, Output,
  PreserveEntrySignatures, RawMinifyOptions, ScanMode,
};

const RUNTIME: &str = r"
class GraphAuditRuntime extends DevRuntime {
  createModuleHotContext() { return { accept() {}, acceptExports() {} }; }
  registerGraph(delta) {
    super.registerGraph(delta);
    globalThis.graphRows.push(...delta.ids.slice(0, delta.localCount));
  }
}
globalThis.__rolldown_runtime__ = new GraphAuditRuntime('graph-audit');
";

#[tokio::test(flavor = "multi_thread")]
async fn dev_graph_survives_incremental_ast_consumption() {
  let pipeline = "chunk-ast";
  for inline in [false, true] {
    for full in [false, true] {
      let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tmp/adoption-audit/dev-graph")
        .join(pipeline)
        .join(format!("{inline}-{full}"));
      let input = directory.join("input");
      std::fs::create_dir_all(&input).unwrap();
      for name in ["a", "b"] {
        std::fs::write(input.join(format!("{name}.js")), format!("import {{ read, load }} from './shared.js';\nglobalThis.{name} = read;\nglobalThis.load = load;\nglobalThis.events.push('dev-{name}-anchor');\n")).unwrap();
      }
      std::fs::write(
        input.join("lazy.js"),
        "globalThis.events.push('dev-lazy-anchor');\nexport const lazy = 23;\n",
      )
      .unwrap();
      let leaf_path = input.join("leaf.js");
      std::fs::write(
        &leaf_path,
        "export const value = 1;\nimport.meta.hot.acceptExports(['value']);\n",
      )
      .unwrap();
      let shared = "import { value } from './leaf.js';\nexport function read() { return value; }\nexport function load() { return import('./lazy.js'); }\nimport.meta.hot.accept();\nglobalThis.events.push('dev-shared-anchor');\nif (!globalThis.graphRows.includes('shared.js')) throw new Error('missing factory graph');\n";
      std::fs::write(input.join("shared.js"), shared).unwrap();
      let mut bundler = Bundler::new(BundlerOptions {
        cwd: Some(input),
        input: Some(
          ["a", "b"]
            .into_iter()
            .map(|name| InputItem { name: Some(name.into()), import: format!("./{name}.js") })
            .collect(),
        ),
        entry_filenames: Some("[name].mjs".to_string().into()),
        chunk_filenames: Some("[name]-[hash].mjs".to_string().into()),
        preserve_entry_signatures: Some(PreserveEntrySignatures::False),
        code_splitting: inline.then(|| {
          CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
            experimental_inline_common_chunks: Some(InlineCommonChunksOptions {
              max_size: Some(f64::INFINITY),
              exclude: None,
            }),
            ..Default::default()
          })
        }),
        experimental: Some(ExperimentalOptions {
          dev_mode: Some(DevModeOptions { implement: Some(RUNTIME.into()), ..Default::default() }),
          ..Default::default()
        }),
        minify: Some(if full {
          RawMinifyOptions::Bool(true)
        } else {
          RawMinifyOptions::DeadCodeEliminationOnly
        }),
        sourcemap: Some(SourceMapType::File),
        ..Default::default()
      })
      .unwrap();
      for generation in 0..3 {
        if generation == 1 {
          std::fs::write(
            &leaf_path,
            "export const value = 2;\nimport.meta.hot.acceptExports(['value']);\n",
          )
          .unwrap();
        }
        let scan = match generation {
          0 => ScanMode::Full,
          1 => ScanMode::Partial(vec![arcstr::ArcStr::from(leaf_path.to_str().unwrap())]),
          _ => ScanMode::Partial(vec![]),
        };
        let output = bundler.incremental_generate(scan).await.unwrap();
        let dist = directory.join(format!("dist-{generation}"));
        std::fs::create_dir_all(&dist).unwrap();
        let mut copies = 0;
        for asset in &output.assets {
          let Output::Chunk(chunk) = asset else { continue };
          std::fs::write(dist.join(chunk.filename.as_str()), &chunk.code).unwrap();
          if !chunk.code.contains("dev-shared-anchor") {
            continue;
          }
          copies += 1;
          let json = chunk.map.as_ref().unwrap().to_json_string();
          let map = oxc_sourcemap::SourceMap::from_json_string(&json).unwrap();
          super::map_assertions::assert_valid_map(&chunk.code, &map);
          super::map_assertions::assert_mapping(
            &chunk.code,
            &map,
            "dev-shared-anchor",
            "shared.js",
            shared,
            "'dev-shared-anchor'",
          );
          if !full {
            super::map_assertions::assert_unmapped(&chunk.code, &map, "registerGraph(");
          }
          std::fs::write(dist.join(format!("{}.map", chunk.filename)), json).unwrap();
        }
        assert!(copies >= if inline { 2 } else { 1 });
        let runner = dist.join("execute.mjs");
        std::fs::write(&runner, format!("import assert from 'node:assert/strict';\nglobalThis.graphRows = [];\nglobalThis.events = [];\nawait import('./a.mjs');\nawait import('./b.mjs');\nassert.equal(a(), {});\nassert.equal(b(), {});\nassert.equal(events.filter(event => event === 'dev-shared-anchor').length, 1);\nassert.deepEqual(__rolldown_runtime__.getImporters('leaf.js'), ['shared.js']);\nassert.deepEqual([...__rolldown_runtime__.getImportedBindings('shared.js', 'leaf.js')], ['value']);\nassert.deepEqual(__rolldown_runtime__.getImporters('lazy.js'), ['shared.js']);\nassert.equal(events.includes('dev-lazy-anchor'), false);\nassert.equal((await load()).lazy, 23);\nassert.deepEqual(__rolldown_runtime__.getImporters('lazy.js'), ['shared.js']);\nassert.equal(events.filter(event => event === 'dev-lazy-anchor').length, 1);\nassert.ok(__rolldown_runtime__.entries.has('a.js'));\nassert.ok(__rolldown_runtime__.entries.has('b.js'));\n", if generation == 0 { 1 } else { 2 }, if generation == 0 { 1 } else { 2 })).unwrap();
        let result = Command::new("node").arg(runner).output().unwrap();
        assert!(
          result.status.success(),
          "{pipeline}/{inline}/{full}/{generation}: {}",
          String::from_utf8_lossy(&result.stderr)
        );
      }
    }
  }
}
