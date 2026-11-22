use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_shared_read_no_route() {
  // `seg.js` is an emitted AllowExtension entry, and the `group` chunk holds its module. The entry
  // re-exports `foreign` from the `dep.js` chunk. It also re-exports `y` from `counter.cjs` and `z`
  // through `app.js`, so it imports `app.js`. Thus the `app.js` chunk holds `counter.cjs`.
  // `main.js` loads the `app.js` entry with `import()`, so the `app.js` chunk cannot export an
  // accessor. `app.js` re-exports `x`, `y` and `z`. A facade could read `z` through the public
  // export `z` of the `app.js` chunk. But it reads `y` through the namespace in the `group` chunk,
  // which is not an entry chunk. Thus that read can neither call an accessor nor read a public
  // export, and the early check rejects `counter.cjs`. The entry gets no facade, as without the
  // cross-chunk check, and code outside the bundle does not get `foreign` from it. Each property
  // keeps its own read, so `app.js` gets the values of Node when it loads alone.
  let options = BundlerOptions {
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
  };
  manual_integration_test!()
    .build(TestMeta { expect_executed: true, snapshot: false, ..Default::default() })
    .run_multiple(
      super::cross_chunk_reexport_fixture::es_and_cjs(options),
      vec![super::cross_chunk_reexport_fixture::emit_chunks(&["./seg.js"])],
    )
    .await;
}
