use std::{path::Path, process::Command};

use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_common::{
  CodeSplittingMode, ExperimentalOptions, ManualCodeSplittingOptions, MatchGroup, MatchGroupName,
  MatchGroupTest, Output, RawCompressOptions, RawMangleOptions, RawMinifyOptions,
  RawMinifyOptionsDetailed,
};
use rolldown_utils::js_regex::HybridRegex;

const FIXTURE: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/topics/chunk_ast_dce/wrapped_entry");

fn minify(mode: &str) -> RawMinifyOptions {
  match mode {
    "dce" => RawMinifyOptions::DeadCodeEliminationOnly,
    "full" => RawMinifyOptions::Bool(true),
    _ => RawMinifyOptions::Object(RawMinifyOptionsDetailed {
      compress: (mode == "compress").then(RawCompressOptions::default),
      mangle: (mode == "mangle").then(RawMangleOptions::default),
      mangle_properties: None,
      remove_whitespace: mode == "mangle",
      ascii_only: false,
    }),
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn wrapped_entry_initialization_and_maps() {
  let pipeline = "chunk-ast";
  for mode in ["dce", "compress", "mangle", "full"] {
    let minify = minify(mode);
    let output = Bundler::new(BundlerOptions {
      cwd: Some(FIXTURE.into()),
      input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
      strict_execution_order: Some(true),
      entry_filenames: Some("[name].mjs".to_string().into()),
      minify: Some(minify),
      sourcemap: Some(SourceMapType::File),
      ..Default::default()
    })
    .unwrap()
    .generate()
    .await
    .unwrap();
    let chunks: Vec<_> = output
      .assets
      .iter()
      .filter_map(|asset| match asset {
        Output::Chunk(chunk) => Some(chunk),
        Output::Asset(_) => None,
      })
      .collect();
    assert_eq!(chunks.len(), 1);
    let chunk = chunks[0];
    let serialized = chunk.map.as_ref().unwrap().to_json_string();
    let map = oxc_sourcemap::SourceMap::from_json_string(&serialized).unwrap();
    super::map_assertions::assert_valid_map(&chunk.code, &map);
    for (marker, source, content) in [
      ("entry-anchor", "main.js", include_str!("wrapped_entry/main.js")),
      ("dep-anchor", "dep.js", include_str!("wrapped_entry/dep.js")),
      ("after-await-anchor", "dep.js", include_str!("wrapped_entry/dep.js")),
    ] {
      super::map_assertions::assert_mapping(
        &chunk.code,
        &map,
        marker,
        source,
        content,
        &format!("'{marker}'"),
      );
    }
    super::map_assertions::assert_unmapped(&chunk.code, &map, "export");
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../../tmp/adoption-audit/glue-migration/runtime")
      .join(pipeline)
      .join(mode);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("main.mjs"), &chunk.code).unwrap();
    std::fs::write(directory.join("main.mjs.map"), serialized).unwrap();
    let runner = directory.join("execute.mjs");
    std::fs::write(&runner, "import assert from 'node:assert/strict';\nglobalThis.__wrappedEvents = [];\nglobalThis.__wrappedInput = 40;\nconst entry = await import('./main.mjs');\nassert.equal(entry.initial, 40);\nassert.equal(entry.completed, 'ready-anchor');\nassert.equal(entry.update(), 42);\nassert.equal(entry.update(), 44);\nassert.deepEqual(globalThis.__wrappedEvents, ['dep-anchor', 'after-await-anchor', 'entry-anchor']);\nawait import('./main.mjs');\nassert.equal(globalThis.__wrappedEvents.length, 3);\n").unwrap();
    let executed = Command::new("node").arg(runner).output().unwrap();
    assert!(
      executed.status.success(),
      "{pipeline}/{mode}: {}",
      String::from_utf8_lossy(&executed.stderr)
    );
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn shared_async_entry_initialization() {
  let pipeline = "chunk-ast";
  for mode in ["dce", "compress", "mangle", "full"] {
    let output = Bundler::new(BundlerOptions {
      cwd: Some(FIXTURE.into()),
      input: Some(vec![
        InputItem { name: Some("main".into()), import: "./main.js".into() },
        InputItem { name: Some("secondary".into()), import: "./secondary.js".into() },
      ]),
      strict_execution_order: Some(false),
      entry_filenames: Some("[name].mjs".to_string().into()),
      chunk_filenames: Some("[name]-[hash].mjs".to_string().into()),
      minify: Some(minify(mode)),
      sourcemap: Some(SourceMapType::File),
      ..Default::default()
    })
    .unwrap()
    .generate()
    .await
    .unwrap();
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../../tmp/adoption-audit/glue-migration/shared-async")
      .join(pipeline)
      .join(mode);
    std::fs::create_dir_all(&directory).unwrap();
    let mut anchors = 0;
    for asset in &output.assets {
      let Output::Chunk(chunk) = asset else { continue };
      let serialized = chunk.map.as_ref().unwrap().to_json_string();
      let map = oxc_sourcemap::SourceMap::from_json_string(&serialized).unwrap();
      super::map_assertions::assert_valid_map(&chunk.code, &map);
      for (marker, source, content) in [
        ("entry-anchor", "main.js", include_str!("wrapped_entry/main.js")),
        ("secondary-anchor", "secondary.js", include_str!("wrapped_entry/secondary.js")),
        ("dep-anchor", "dep.js", include_str!("wrapped_entry/dep.js")),
        ("after-await-anchor", "dep.js", include_str!("wrapped_entry/dep.js")),
      ] {
        if chunk.code.contains(marker) {
          super::map_assertions::assert_mapping(
            &chunk.code,
            &map,
            marker,
            source,
            content,
            &format!("'{marker}'"),
          );
          anchors += 1;
        }
      }
      std::fs::write(directory.join(chunk.filename.as_str()), &chunk.code).unwrap();
      std::fs::write(directory.join(chunk.sourcemap_filename.as_ref().unwrap()), serialized)
        .unwrap();
    }
    assert_eq!(anchors, 4);
    let runner = directory.join("execute.mjs");
    std::fs::write(&runner, "import assert from 'node:assert/strict';\nglobalThis.__wrappedEvents = [];\nglobalThis.__wrappedInput = 40;\nconst main = await import('./main.mjs');\nassert.equal(main.initial, 40);\nassert.equal(main.update(), 42);\nassert.deepEqual(globalThis.__wrappedEvents, ['dep-anchor', 'after-await-anchor', 'entry-anchor']);\nconst secondary = await import('./secondary.mjs');\nassert.equal(secondary.current(), 42);\nassert.deepEqual(globalThis.__wrappedEvents, ['dep-anchor', 'after-await-anchor', 'entry-anchor', 'secondary-anchor']);\nassert.equal(main.update(), 44);\nassert.equal(secondary.current(), 44);\nawait import('./secondary.mjs');\nassert.equal(globalThis.__wrappedEvents.length, 4);\n").unwrap();
    let executed = Command::new("node").arg(runner).output().unwrap();
    assert!(
      executed.status.success(),
      "{pipeline}/{mode}: {}",
      String::from_utf8_lossy(&executed.stderr)
    );
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_facade_preserves_lazy_entry_execution() {
  let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("tests/rolldown/function/experimental/strict_execution_order/entry_shared_chunk_facade");
  let pipeline = "chunk-ast";
  for on_demand in [false, true] {
    for mode in ["dce", "compress", "mangle", "full"] {
      let output = Bundler::new(BundlerOptions {
        cwd: Some(fixture.clone()),
        input: Some(vec![
          InputItem { name: Some("b".into()), import: "./b.js".into() },
          InputItem { name: Some("e".into()), import: "./e.js".into() },
        ]),
        strict_execution_order: Some(true),
        experimental: Some(ExperimentalOptions {
          on_demand_wrapping: Some(on_demand),
          ..Default::default()
        }),
        code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
          groups: Some(vec![MatchGroup {
            name: MatchGroupName::Static("implementation".into()),
            test: Some(MatchGroupTest::Regex(
              HybridRegex::new(r"[\\/]entry_shared_chunk_facade[\\/](?:e|shared)\.js$").unwrap(),
            )),
            ..Default::default()
          }]),
          ..Default::default()
        })),
        entry_filenames: Some("[name].mjs".to_string().into()),
        chunk_filenames: Some("[name]-[hash].mjs".to_string().into()),
        minify: Some(minify(mode)),
        sourcemap: Some(SourceMapType::File),
        ..Default::default()
      })
      .unwrap()
      .generate()
      .await
      .unwrap();
      let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tmp/adoption-audit/glue-migration/facade")
        .join(pipeline)
        .join(format!("{on_demand}-{mode}"));
      std::fs::create_dir_all(&directory).unwrap();
      let mut checked = 0;
      let mut facade = false;
      for asset in &output.assets {
        let Output::Chunk(chunk) = asset else { continue };
        let serialized = chunk.map.as_ref().unwrap().to_json_string();
        let map = oxc_sourcemap::SourceMap::from_json_string(&serialized).unwrap();
        super::map_assertions::assert_valid_map(&chunk.code, &map);
        if chunk.filename.as_str() == "e.mjs" {
          assert!(chunk.module_ids.is_empty(), "must exercise a generated-only facade");
          assert!(map.get_tokens().all(|token| token.get_source_id().is_none()));
          facade = true;
        }
        for (marker, source) in [("S body", "shared.js"), ("B body ", "b.js"), ("E body", "e.js")] {
          if chunk.code.contains(marker) {
            let content = std::fs::read_to_string(fixture.join(source)).unwrap();
            super::map_assertions::assert_mapping(
              &chunk.code,
              &map,
              marker,
              source,
              &content,
              &format!("'{marker}'"),
            );
            checked += 1;
          }
        }
        std::fs::write(directory.join(chunk.filename.as_str()), &chunk.code).unwrap();
        if let Some(filename) = &chunk.sourcemap_filename {
          std::fs::write(directory.join(filename), serialized).unwrap();
        }
      }
      assert!(facade);
      assert_eq!(checked, 3);
      let runner = directory.join("execute.mjs");
      std::fs::write(&runner, "import assert from 'node:assert/strict';\nglobalThis.events = [];\nawait import('./b.mjs');\nassert.deepEqual(globalThis.events, ['S body', 'B body S']);\nawait import('./e.mjs');\nassert.deepEqual(globalThis.events, ['S body', 'B body S', 'E body']);\nawait import('./e.mjs');\nassert.equal(globalThis.events.length, 3);\n").unwrap();
      let executed = Command::new("node").arg(runner).output().unwrap();
      assert!(
        executed.status.success(),
        "{pipeline}/{on_demand}/{mode}: {}",
        String::from_utf8_lossy(&executed.stderr)
      );
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn consumer_local_initializers_preserve_order() {
  let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/rolldown/function/experimental/strict_execution_order/cjs_reexport_barrel_consumer_local");
  let pipeline = "chunk-ast";
  for on_demand in [false, true] {
    for mode in ["dce", "compress", "mangle", "full"] {
      let output = Bundler::new(BundlerOptions {
        cwd: Some(fixture.clone()),
        input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
        strict_execution_order: Some(true),
        experimental: Some(ExperimentalOptions {
          on_demand_wrapping: Some(on_demand),
          ..Default::default()
        }),
        preserve_entry_signatures: Some(rolldown_common::PreserveEntrySignatures::AllowExtension),
        entry_filenames: Some("[name].mjs".to_string().into()),
        chunk_filenames: Some("[name]-[hash].mjs".to_string().into()),
        minify: Some(minify(mode)),
        sourcemap: Some(SourceMapType::File),
        ..Default::default()
      })
      .unwrap()
      .generate()
      .await
      .unwrap();
      let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tmp/adoption-audit/glue-migration/consumer-local")
        .join(pipeline)
        .join(format!("{on_demand}-{mode}"));
      std::fs::create_dir_all(&directory).unwrap();
      let mut anchors = 0;
      for asset in &output.assets {
        let Output::Chunk(chunk) = asset else { continue };
        let serialized = chunk.map.as_ref().unwrap().to_json_string();
        let map = oxc_sourcemap::SourceMap::from_json_string(&serialized).unwrap();
        super::map_assertions::assert_valid_map(&chunk.code, &map);
        for (marker, source) in [
          ("clone-deep", "node_modules/pure-barrel/clone-deep.cjs"),
          ("stack", "node_modules/pure-barrel/stack.js"),
          ("cn", "node_modules/pure-barrel/cn.js"),
        ] {
          let generated = ['"', '\'', '`']
            .into_iter()
            .map(|quote| format!("{quote}{marker}{quote}"))
            .find(|literal| chunk.code.contains(literal));
          if let Some(generated) = generated {
            let content = std::fs::read_to_string(fixture.join(source)).unwrap();
            super::map_assertions::assert_mapping(
              &chunk.code,
              &map,
              &generated,
              source,
              &content,
              &format!("'{marker}'"),
            );
            anchors += 1;
          }
        }
        std::fs::write(directory.join(chunk.filename.as_str()), &chunk.code).unwrap();
        if let Some(filename) = &chunk.sourcemap_filename {
          std::fs::write(directory.join(filename), serialized).unwrap();
        }
      }
      assert!(anchors >= 3);
      let runner = directory.join("execute.mjs");
      std::fs::write(&runner, "import assert from 'node:assert/strict';\nglobalThis.__events = [];\nconst entry = await import('./main.mjs');\nassert.deepEqual(globalThis.__events, ['cn', 'main:a b']);\nconst ns = await entry.loadNamespace();\nassert.deepEqual(globalThis.__events, ['cn', 'main:a b', 'stack', 'clone-deep']);\nassert.equal(ns.cn('a', 'b'), 'a b');\nassert.deepEqual(ns.cloneDeep({value: 1}), {value: 1});\nassert.deepEqual(new ns.Stack().items, []);\nassert.equal((await entry.loadRoute()).value, 1);\nassert.deepEqual(globalThis.__events, ['cn', 'main:a b', 'stack', 'clone-deep', 'route']);\nawait entry.loadNamespace();\nassert.equal(globalThis.__events.length, 5);\n").unwrap();
      let executed = Command::new("node").arg(runner).output().unwrap();
      assert!(
        executed.status.success(),
        "{pipeline}/{on_demand}/{mode}: {}",
        String::from_utf8_lossy(&executed.stderr)
      );
    }
  }
}
