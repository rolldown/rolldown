use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_shared_read_cycle() {
  // `seg.js` is an emitted AllowExtension entry, and the `group` chunk holds its module. The entry
  // re-exports `foreign` from the `dep.js` chunk. It also re-exports `z` of `counter.cjs` through
  // `app.js`, and the `app.js` chunk holds `counter.cjs`. `main.js` loads the `app.js` entry with
  // `import()`, so the `app.js` chunk cannot export an accessor. `app.js` reads `z` of the
  // namespace of `seg.js` in its body. A getter of that namespace object reads `z` each time that
  // code reads it, so the early check rejects `counter.cjs`. The entry gets no facade, as without
  // the cross-chunk check, and code outside the bundle does not get `foreign` from it.
  //
  // The getters of `counter.cjs` give constant values, so the test checks only that the read in
  // the cycle gets a value. The test uses only ESM output. In CJS output, the getter reads the
  // namespace of `counter.cjs` through the exports object of the `app.js` chunk, and the chunk
  // fills this object at its end. Thus the read in the cycle fails in CJS output.
  manual_integration_test!()
    .build(TestMeta { expect_executed: true, snapshot: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![
          InputItem { name: Some("app".into()), import: "./app.js".into() },
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
      },
      vec![super::cross_chunk_reexport_fixture::emit_chunks(&["./seg.js"])],
    )
    .await;
}
