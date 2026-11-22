use rolldown::{BundlerOptions, InputItem, PreserveEntrySignatures};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_esm_and_commonjs() {
  // `seg.js` is an emitted AllowExtension entry, and the `group` chunk holds its module. The entry
  // re-exports `task` from the `task.js` entry chunk and `foo` of `dep.cjs`. The `entry.js` entry
  // also re-exports `foo`, so `dep.cjs` gets its own chunk. The facade keeps both re-exports in ES
  // and CJS output (issue #11767). Without the facade, the `group` chunk exports neither name.
  let options = BundlerOptions {
    input: Some(vec![
      InputItem { name: Some("entry".into()), import: "./entry.js".into() },
      InputItem { name: Some("task".into()), import: "./task.js".into() },
    ]),
    preserve_entry_signatures: Some(PreserveEntrySignatures::AllowExtension),
    code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
      include_dependencies_recursively: Some(false),
      groups: Some(vec![MatchGroup {
        name: MatchGroupName::Static("group".to_string()),
        test: Some(MatchGroupTest::Regex(HybridRegex::new(r"seg\.js$").unwrap())),
        ..Default::default()
      }]),
      ..Default::default()
    })),
    ..Default::default()
  };
  manual_integration_test!()
    .build(TestMeta { expect_executed: true, snapshot: false, ..Default::default() })
    .run_multiple(
      super::cross_chunk_reexport_fixture::es_and_cjs(options),
      vec![super::cross_chunk_reexport_fixture::emit_chunks(&["./seg.js"])],
    )
    .await;
}
