use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_strict_order() {
  // `seg_dynamic.js` re-exports `task` from the `task.js` chunk. Thus the chunk optimizer keeps its
  // facade for code outside the bundle, and it sends `import()` in the bundle to the `group` chunk
  // (issue #11767). Strict execution order wraps `seg_dynamic.js`. An emitted entry in the order
  // wrap plan keeps a facade in all layouts, so `restore_order_wrap_entry_facades` sends `import()`
  // back to the facade. `seg_sib.js` is in the same `group` chunk, and it does not run.
  manual_integration_test!()
    .build(TestMeta { expect_executed: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: Some("index".into()), import: "./index.js".into() }]),
        strict_execution_order: Some(true),
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
      vec![super::cross_chunk_reexport_fixture::emit_chunks(&["./seg_dynamic.js", "./seg_sib.js"])],
    )
    .await;
}
