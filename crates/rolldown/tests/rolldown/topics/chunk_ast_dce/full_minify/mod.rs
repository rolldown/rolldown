mod comments;
mod virtual_owner;

use rolldown::{BundlerOptions, InputItem, SourceMapType};
use rolldown_common::{
  RawCompressOptions, RawMangleOptions, RawMinifyOptions, RawMinifyOptionsDetailed,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

// Full minify shares chunk-wide scoping and private-member mappings across per-module prints.
// The two modules deliberately use the same private name, including a nested class, to catch
// class IDs being restarted at each module boundary.
#[tokio::test(flavor = "multi_thread")]
async fn full_minify() {
  for (name, minify, keep_names) in [
    ("full", RawMinifyOptions::Bool(true), false),
    ("keep_names", RawMinifyOptions::Bool(true), true),
    (
      "mangle_only",
      RawMinifyOptions::Object(RawMinifyOptionsDetailed {
        mangle: Some(RawMangleOptions::default()),
        mangle_properties: None,
        compress: None,
        remove_whitespace: false,
        ascii_only: false,
      }),
      false,
    ),
    (
      "compress_only",
      RawMinifyOptions::Object(RawMinifyOptionsDetailed {
        mangle: None,
        mangle_properties: None,
        compress: Some(RawCompressOptions::default()),
        remove_whitespace: false,
        ascii_only: true,
      }),
      false,
    ),
  ] {
    manual_integration_test!()
      .build(TestMeta { snapshot: false, config_name: Some(name.into()), ..Default::default() })
      .run(BundlerOptions {
        input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
        minify: Some(minify),
        keep_names: Some(keep_names),
        sourcemap: Some(SourceMapType::File),
        ..Default::default()
      })
      .await;
  }
}
