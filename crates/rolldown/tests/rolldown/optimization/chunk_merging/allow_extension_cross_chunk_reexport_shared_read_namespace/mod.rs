use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_shared_read_namespace() {
  // `seg.js` and `seg2.js` are emitted AllowExtension entries, and the `group` chunk holds their
  // modules. Each entry re-exports `foreign` from the `dep.js` chunk, so it keeps its facade
  // (issue #11767). Each entry also re-exports `n` of a CommonJS module, with a different name, so
  // no name conflict keeps a facade. The facade reads `n` when it loads. `main.js` loads each entry
  // with `import()`, which gives the namespace object of the entry from the `group` chunk. The
  // getter of the export in that object and the facade share one read of `n`, so they give the
  // same value in each load order.
  // - `seg.js` re-exports `n` of `counter.cjs` as `first`. `app.js` also imports `counter.cjs`, so
  //   `counter.cjs` is in a common chunk. The getter calls the accessor of that chunk, and the
  //   facade calls a forward in the `group` chunk.
  // - `seg2.js` re-exports `n` of `counter2.cjs` as `second`. The `group` chunk holds
  //   `counter2.cjs`, so it declares the accessor.
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
        test: Some(MatchGroupTest::Regex(HybridRegex::new(r"(seg2?\.js|counter2\.cjs)$").unwrap())),
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
      vec![super::cross_chunk_reexport_fixture::emit_chunks(&["./seg.js", "./seg2.js"])],
    )
    .await;
}
