use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_shared_read_unreachable_reader() {
  // `seg.js` and `seg2.js` are emitted AllowExtension entries in the `group` and `group2` chunks.
  // Each entry re-exports `foreign` from the `dep.js` chunk, so it keeps its facade (issue
  // #11767). `app.js` and `app2.js` import `counter.cjs`, so it is in a common chunk. The facade of
  // `seg.js` re-exports `y` of `counter.cjs` through the `group` chunk, and `app.js` re-exports `x`
  // and `z`. The facade of `seg2.js` re-exports `z` through `app.js`. `main.js` loads the `app.js`
  // entry with `import()`, so the `app.js` chunk cannot export a forward, and no chunk on a path
  // from the facade of `seg2.js` to the common chunk can export a forward. Thus the facade of
  // `seg2.js` cannot call an accessor. It reads the public export `z` of the `app.js` chunk, and
  // that export calls the accessor. Thus one accessor reads `x`, `z` and `y` of `counter.cjs` in
  // the order of Node, and each load order gives the values of Node.
  let options = BundlerOptions {
    input: Some(vec![
      InputItem { name: Some("app".into()), import: "./app.js".into() },
      InputItem { name: Some("app2".into()), import: "./app2.js".into() },
      InputItem { name: Some("dep".into()), import: "./dep.js".into() },
      InputItem { name: Some("main".into()), import: "./main.js".into() },
    ]),
    code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
      include_dependencies_recursively: Some(false),
      groups: Some(vec![
        MatchGroup {
          name: MatchGroupName::Static("group".to_string()),
          test: Some(MatchGroupTest::Regex(HybridRegex::new(r"seg\.js$").unwrap())),
          ..Default::default()
        },
        MatchGroup {
          name: MatchGroupName::Static("group2".to_string()),
          test: Some(MatchGroupTest::Regex(HybridRegex::new(r"seg2\.js$").unwrap())),
          ..Default::default()
        },
      ]),
      ..Default::default()
    })),
    ..Default::default()
  };
  manual_integration_test!()
    .build(TestMeta { expect_executed: true, snapshot: false, ..Default::default() })
    .run_multiple(
      super::cross_chunk_reexport_fixture::es_and_cjs(options),
      vec![super::cross_chunk_reexport_fixture::emit_chunks(&["./seg.js", "./seg2.js"])],
    )
    .await;
}
