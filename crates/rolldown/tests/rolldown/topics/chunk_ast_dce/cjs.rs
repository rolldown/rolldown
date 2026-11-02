use std::{path::Path, process::Command};

use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_common::{
  Output, OutputFormat, RawCompressOptions, RawMangleOptions, RawMinifyOptions,
  RawMinifyOptionsDetailed,
};

#[tokio::test(flavor = "multi_thread")]
async fn runtime_precedes_external_require() {
  let fixture =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/rolldown/topics/chunk_ast_dce/cjs");
  let pipeline = "chunk-ast";
  for strict in [
    rolldown_common::StrictMode::Auto,
    rolldown_common::StrictMode::Always,
    rolldown_common::StrictMode::Never,
  ] {
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
        external: Some(vec!["./external.cjs".into()].into()),
        format: Some(OutputFormat::Cjs),
        strict: Some(strict),
        entry_filenames: Some("[name].cjs".to_string().into()),
        sourcemap: Some(SourceMapType::File),
        minify: Some(minify),
        ..Default::default()
      })
      .unwrap()
      .generate()
      .await
      .unwrap();
      let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tmp/adoption-audit/cjs-migration")
        .join(pipeline)
        .join(format!("{strict:?}-{mode}"));
      std::fs::create_dir_all(&directory).unwrap();
      let mut anchors = 0;
      for asset in &output.assets {
        let Output::Chunk(chunk) = asset else { continue };
        let serialized = chunk.map.as_ref().unwrap().to_json_string();
        let map = oxc_sourcemap::SourceMap::from_json_string(&serialized).unwrap();
        super::map_assertions::assert_valid_map(&chunk.code, &map);
        if chunk.code.contains("cjs-entry-anchor") {
          super::map_assertions::assert_mapping(
            &chunk.code,
            &map,
            "cjs-entry-anchor",
            "main.js",
            include_str!("cjs/main.js"),
            "'cjs-entry-anchor'",
          );
          anchors += 1;
        }
        std::fs::write(directory.join(chunk.filename.as_str()), &chunk.code).unwrap();
        std::fs::write(directory.join(chunk.sourcemap_filename.as_ref().unwrap()), serialized)
          .unwrap();
      }
      assert_eq!(anchors, 1);
      std::fs::copy(fixture.join("external.cjs"), directory.join("external.cjs")).unwrap();
      let runner = directory.join("execute.cjs");
      std::fs::write(&runner, "const assert = require('node:assert/strict');\nglobalThis.__cjsEvents = [];\nconst entry = require('./main.cjs');\nassert.equal(entry.default, 42);\nassert.equal(entry.current, 1);\nentry.increment();\nassert.equal(entry.current, 2);\nassert.equal(entry.__esModule, true);\nassert.deepEqual(globalThis.__cjsEvents, ['external', 'cjs-entry-anchor']);\nrequire('./main.cjs');\nassert.equal(globalThis.__cjsEvents.length, 2);\n").unwrap();
      let executed = Command::new("node").arg(runner).output().unwrap();
      assert!(
        executed.status.success(),
        "{pipeline}/{mode}: {}",
        String::from_utf8_lossy(&executed.stderr)
      );
    }
  }
}
