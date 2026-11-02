use std::{path::Path, process::Command};

use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_common::{
  Output, RawCompressOptions, RawMangleOptions, RawMinifyOptions, RawMinifyOptionsDetailed,
};

#[tokio::test(flavor = "multi_thread")]
async fn finalized_external_imports_preserve_live_reexports() {
  let pipeline = "chunk-ast";
  let producer = "export * from './external.mjs';\nglobalThis.__externalEvents.push('finalized-import-anchor');\n";
  for mode in ["dce", "compress", "mangle", "full"] {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../../tmp/adoption-audit/finalized-external-imports")
      .join(pipeline)
      .join(mode);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("entry.js"), "module.exports = require('./producer.js');\n")
      .unwrap();
    std::fs::write(directory.join("producer.js"), producer).unwrap();
    std::fs::write(directory.join("external.mjs"), "globalThis.__externalEvents.push('external');\nexport let value = 41;\nexport function increment() { value++; }\n").unwrap();
    let minify = match mode {
      "dce" => RawMinifyOptions::DeadCodeEliminationOnly,
      "full" => RawMinifyOptions::Bool(true),
      _ => RawMinifyOptions::Object(RawMinifyOptionsDetailed {
        compress: (mode == "compress").then(RawCompressOptions::default),
        mangle: (mode == "mangle").then(RawMangleOptions::default),
        mangle_properties: None,
        remove_whitespace: mode == "mangle",
        ascii_only: false,
      }),
    };
    let output = Bundler::new(BundlerOptions {
      cwd: Some(directory.clone()),
      input: Some(vec![InputItem { name: Some("main".into()), import: "./entry.js".into() }]),
      external: Some(vec!["./external.mjs".into()].into()),
      treeshake: rolldown_common::TreeshakeOptions::Boolean(false),
      entry_filenames: Some("main.mjs".to_string().into()),
      minify: Some(minify),
      sourcemap: Some(SourceMapType::File),
      ..Default::default()
    })
    .unwrap()
    .generate()
    .await
    .unwrap();
    let chunk = output
      .assets
      .iter()
      .find_map(|asset| if let Output::Chunk(chunk) = asset { Some(chunk) } else { None })
      .unwrap();
    let json = chunk.map.as_ref().unwrap().to_json_string();
    let map = oxc_sourcemap::SourceMap::from_json_string(&json).unwrap();
    super::map_assertions::assert_valid_map(&chunk.code, &map);
    super::map_assertions::assert_mapping(
      &chunk.code,
      &map,
      "finalized-import-anchor",
      "producer.js",
      producer,
      "'finalized-import-anchor'",
    );
    std::fs::write(directory.join("main.mjs"), &chunk.code).unwrap();
    std::fs::write(directory.join("main.mjs.map"), map.to_json_string()).unwrap();
    let runner = directory.join("execute.mjs");
    std::fs::write(&runner, "import assert from 'node:assert/strict';\nglobalThis.__externalEvents = [];\nconst { default: entry } = await import('./main.mjs');\nassert.equal(entry.value, 41);\nentry.increment();\nassert.equal(entry.value, 42);\nassert.deepEqual(__externalEvents, ['external', 'finalized-import-anchor']);\n").unwrap();
    let executed = Command::new("node").arg(runner).output().unwrap();
    assert!(
      executed.status.success(),
      "{pipeline}/{mode}: {}",
      String::from_utf8_lossy(&executed.stderr)
    );
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn external_star_exports_share_chunk_optimization() {
  let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("tests/rolldown/topics/chunk_ast_dce/generated_glue");
  let pipeline = "chunk-ast";
  for mode in ["dce", "compress", "mangle", "full"] {
    let minify = match mode {
      "dce" => RawMinifyOptions::DeadCodeEliminationOnly,
      "full" => RawMinifyOptions::Bool(true),
      _ => RawMinifyOptions::Object(RawMinifyOptionsDetailed {
        compress: (mode == "compress").then(RawCompressOptions::default),
        mangle: (mode == "mangle").then(RawMangleOptions::default),
        mangle_properties: None,
        remove_whitespace: mode == "mangle",
        ascii_only: false,
      }),
    };
    let output = Bundler::new(BundlerOptions {
      cwd: Some(fixture.clone()),
      input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
      external: Some(vec!["./external.mjs".into()].into()),
      entry_filenames: Some("main.mjs".to_string().into()),
      minify: Some(minify),
      sourcemap: Some(SourceMapType::File),
      ..Default::default()
    })
    .unwrap()
    .generate()
    .await
    .unwrap();
    let chunk = output
      .assets
      .iter()
      .find_map(|asset| if let Output::Chunk(chunk) = asset { Some(chunk) } else { None })
      .unwrap();
    let map = chunk.map.as_ref().unwrap();
    super::map_assertions::assert_valid_map(&chunk.code, map);
    super::map_assertions::assert_mapping(
      &chunk.code,
      map,
      "external-star-anchor",
      "main.js",
      include_str!("generated_glue/main.js"),
      "'external-star-anchor'",
    );
    super::map_assertions::assert_unmapped(&chunk.code, map, "export");
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../../tmp/adoption-audit/generated-glue")
      .join(pipeline)
      .join(mode);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("main.mjs"), &chunk.code).unwrap();
    std::fs::write(directory.join("main.mjs.map"), map.to_json_string()).unwrap();
    std::fs::copy(fixture.join("external.mjs"), directory.join("external.mjs")).unwrap();
    let runner = directory.join("execute.mjs");
    std::fs::write(&runner, "import assert from 'node:assert/strict';\nglobalThis.__starEvents = [];\nconst entry = await import('./main.mjs');\nassert.equal(entry.value, 41);\nassert.equal(entry.result, 42);\nentry.increment();\nassert.equal(entry.value, 42);\nassert.deepEqual(globalThis.__starEvents, ['external', 'external-star-anchor']);\n").unwrap();
    let executed = Command::new("node").arg(runner).output().unwrap();
    assert!(
      executed.status.success(),
      "{pipeline}/{mode}: {}",
      String::from_utf8_lossy(&executed.stderr)
    );
  }
}
