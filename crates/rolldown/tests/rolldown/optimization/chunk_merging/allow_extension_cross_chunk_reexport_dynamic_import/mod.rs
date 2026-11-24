use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_dynamic_import() {
  // The two emitted AllowExtension entries re-export `foreign` from the `dep.js` chunk, so each
  // entry keeps its facade for code outside the bundle (issue #11767). The test loads the facades
  // directly. An `import()` in the bundle loads the `group` chunk and reads the namespace object
  // of the entry, as it does when the facade is removed. `reader.js` reads the namespace object of
  // `seg_namespace.js` as a value, and `import()` returns the same object. `seg_static.js` is not
  // an `import()` target.
  manual_integration_test!()
    .build(TestMeta { expect_executed: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: Some("index".into()), import: "./index.js".into() }]),
        code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
          include_dependencies_recursively: Some(false),
          groups: Some(vec![MatchGroup {
            name: MatchGroupName::Static("group".to_string()),
            test: Some(MatchGroupTest::Regex(HybridRegex::new("seg_").unwrap())),
            ..Default::default()
          }]),
          ..Default::default()
        })),
        ..Default::default()
      },
      vec![super::cross_chunk_reexport_fixture::emit_chunks(&[
        "./seg_static.js",
        "./seg_namespace.js",
      ])],
    )
    .await;
}
