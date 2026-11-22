use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_shared_read_through_entry() {
  // `seg.js` is an emitted AllowExtension entry, and the `group` chunk holds its module. The entry
  // re-exports `foreign` from the `dep.js` chunk, so it keeps its facade (issue #11767). The facade
  // re-exports `n` of `counter.cjs` through `app.js`, so it reads `n` from the namespace in the
  // `app.js` chunk. `app2.js` also imports `counter.cjs`, so `counter.cjs` is in a common chunk
  // that the facade does not import. The `app.js` chunk imports that chunk, and its signature
  // allows new exports. Thus a function in the `app.js` chunk calls the accessor of `n` for the
  // facade, and the facade gets no new import.
  let options = BundlerOptions {
    input: Some(vec![
      InputItem { name: Some("app".into()), import: "./app.js".into() },
      InputItem { name: Some("app2".into()), import: "./app2.js".into() },
      InputItem { name: Some("dep".into()), import: "./dep.js".into() },
    ]),
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
    .build(TestMeta { expect_executed: true, ..Default::default() })
    .run_multiple(
      super::cross_chunk_reexport_fixture::es_and_cjs(options),
      vec![super::cross_chunk_reexport_fixture::emit_chunks(&["./seg.js"])],
    )
    .await;
}
