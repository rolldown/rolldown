use std::{path::Path, process::Command};

use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_common::{
  Output, RawCompressOptions, RawMangleOptions, RawMinifyOptions, RawMinifyOptionsDetailed,
};

#[tokio::test(flavor = "multi_thread")]
async fn transferred_imports_preserve_order_bindings_and_maps() {
  let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("tests/rolldown/topics/chunk_ast_dce/transferred_imports");
  let pipeline = "chunk-ast";
  for mode in ["none", "dce", "compress", "mangle", "full"] {
    let minify = match mode {
      "none" => RawMinifyOptions::Bool(false),
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
    let map_json = chunk.map.as_ref().unwrap().to_json_string();
    let map = oxc_sourcemap::SourceMap::from_json_string(&map_json).unwrap();
    super::map_assertions::assert_valid_map(&chunk.code, &map);
    for (marker, source, content) in [
      ("transfer-consumer-anchor", "consumer.js", include_str!("transferred_imports/consumer.js")),
      ("transfer-main-anchor", "main.js", include_str!("transferred_imports/main.js")),
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
    if mode == "none" {
      // Both namespace declarations moved from main.js to consumer.js. Their bindings must
      // remain available to main, and donor offsets must not map into the recipient source.
      for binding in ["var import_first", "var import_second"] {
        super::map_assertions::assert_unmapped(&chunk.code, &map, binding);
        assert!(
          chunk.code.find(binding).unwrap() < chunk.code.find("transfer-consumer-anchor").unwrap()
        );
      }
      assert!(
        chunk.code.find("var import_first").unwrap()
          < chunk.code.find("var import_second").unwrap()
      );
    }
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../../tmp/adoption-audit/transferred-imports")
      .join(pipeline)
      .join(mode);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("main.mjs"), &chunk.code).unwrap();
    std::fs::write(directory.join("main.mjs.map"), map_json).unwrap();
    let runner = directory.join("execute.mjs");
    std::fs::write(
      &runner,
      "import assert from 'node:assert/strict';\nglobalThis.__transferEvents = [];\nconst entry = await import('./main.mjs');\nassert.deepEqual(entry.values, [17, 25]);\nassert.deepEqual(globalThis.__transferEvents, ['first', 'second', 'dependency', 'transfer-consumer-anchor', 'transfer-main-anchor']);\n",
    )
    .unwrap();
    let executed = Command::new("node").arg(runner).output().unwrap();
    assert!(
      executed.status.success(),
      "{pipeline}/{mode}: {}",
      String::from_utf8_lossy(&executed.stderr)
    );
  }
}
