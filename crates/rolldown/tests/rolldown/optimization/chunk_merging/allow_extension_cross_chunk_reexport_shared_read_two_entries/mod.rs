use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_shared_read_two_entries() {
  // `seg.js` is an emitted AllowExtension entry, and the `group` chunk holds its module. The entry
  // re-exports `foreign` from the `dep.js` chunk, so it keeps its facade (issue #11767). The `a.js`
  // and `b.js` entries export `n` of `counter.cjs`. `main.js` loads them with `import()`, so
  // neither chunk can export an accessor. The facade re-exports `aval` from `a.js` and `n` from
  // `b.js`, so it imports both chunks, and the `a.js` chunk comes first. The facade reads `n`
  // through the public export `n` of the `b.js` chunk, because this chunk holds the namespace that
  // the facade reads `n` from. Thus the facade keeps its import of the `b.js` chunk. The `group`
  // chunk also imports the `b.js` chunk, so the test values do not show a lost import. A debug
  // assertion of the generate stage fails if the facade loses that import.
  let options = BundlerOptions {
    input: Some(vec![
      InputItem { name: Some("a".into()), import: "./a.js".into() },
      InputItem { name: Some("b".into()), import: "./b.js".into() },
      InputItem { name: Some("app2".into()), import: "./app2.js".into() },
      InputItem { name: Some("dep".into()), import: "./dep.js".into() },
      InputItem { name: Some("main".into()), import: "./main.js".into() },
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
    .build(TestMeta { expect_executed: true, snapshot: false, ..Default::default() })
    .run_multiple(
      super::cross_chunk_reexport_fixture::es_and_cjs(options),
      vec![super::cross_chunk_reexport_fixture::emit_chunks(&["./seg.js"])],
    )
    .await;
}
