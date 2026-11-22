use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_shared_read_barrel_cycle() {
  // `seg.js` is an emitted AllowExtension entry, and the `group` chunk holds its module and
  // `seg2.js`. The entry re-exports `foreign` from the `dep.js` chunk, and `y` of `counter.cjs`
  // through the `mid.js` entry. The `mid.js` chunk holds `counter.cjs`. `main.js` loads `mid.js`
  // with `import()`, so the `mid.js` chunk cannot export an accessor. `main.js` also loads
  // `seg.js` with `import()`, so the `group` chunk reads `y` for the namespace object of `seg.js`.
  // Thus the `group` chunk must read the public export `y` of the `mid.js` chunk, and the `mid.js`
  // chunk must not import the `group` chunk.
  //
  // `mid.js` imports `s2` from `barrel.js`, which re-exports it from `seg2.js`. `barrel.js` has no
  // side effects, so it is in no chunk. But the `mid.js` chunk imports `s2` from the `group` chunk,
  // so the two chunks make a cycle. The early check follows the static imports through
  // `barrel.js`, finds the cycle, and rejects `counter.cjs`. The entry gets no facade, as without
  // the cross-chunk check, and code outside the bundle does not get `foreign` from it.
  let options = BundlerOptions {
    input: Some(vec![
      InputItem { name: Some("main".into()), import: "./main.js".into() },
      InputItem { name: Some("mid".into()), import: "./mid.js".into() },
    ]),
    code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
      include_dependencies_recursively: Some(false),
      groups: Some(vec![MatchGroup {
        name: MatchGroupName::Static("group".to_string()),
        test: Some(MatchGroupTest::Regex(HybridRegex::new(r"seg2?\.js$").unwrap())),
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
