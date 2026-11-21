use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_keeps_facade() {
  // The `group` chunk holds the modules of all three emitted AllowExtension entries.
  // `seg_dynamic.js` and `seg_static.js` re-export bindings that are in the `task.js` chunk, so
  // they keep their facades (issue #11767). `seg_dynamic.js` is also an `import()` target, and
  // `seg_static.js` is not. `seg_local.js` exports only its own binding, so the chunk optimizer
  // removes its facade, and the group chunk exports `s_local`.
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
        "./seg_dynamic.js",
        "./seg_static.js",
        "./seg_local.js",
      ])],
    )
    .await;
}
