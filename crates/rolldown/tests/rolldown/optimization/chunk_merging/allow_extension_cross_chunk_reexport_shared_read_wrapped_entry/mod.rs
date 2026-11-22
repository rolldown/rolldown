use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_shared_read_wrapped_entry() {
  // The layout of `allow_extension_cross_chunk_reexport_shared_read_unreachable_reader`, and
  // `wrap.cjs` in `main.js` requires `app.js`. Thus the `app.js` module is wrapped. The `app.js`
  // chunk calls the wrapper before its exports read `x` and `z`, so the facade of `seg2.js` can read
  // the public export `z` of the chunk (`is_entry_binding`). `main.js` also loads `seg2.js` with
  // `import()`. This gives the namespace object of `seg2.js` from the `group2` chunk, and the getter
  // of `z` in this object reads the same public export. Thus one accessor reads `x`, `z` and `y` of
  // `counter.cjs` in the order of Node, and each load order gives the values of Node.
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
