use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_shared_read_exported_namespace() {
  // `seg_a.js` and `seg_b.js` are emitted AllowExtension entries, and the `group` chunk holds their
  // modules. Each entry re-exports `foreign` from the `dep.js` chunk and `y` from its own CommonJS
  // module. A getter of the namespace of a CommonJS module reads a property each time that code
  // reads it, and the shared read does not include such a getter. Thus the early check rejects
  // each module:
  // - `counter_a.cjs`: the `app.js` entry exports the namespace of the module.
  // - `counter_b.cjs`: `other.js` re-exports the namespace of the module, so it re-exports all
  //   properties of the module. `app.js` passes the namespace object of `other.js` to a function.
  //
  // The entries get no facade, as without the cross-chunk check, and code outside the bundle does
  // not get `foreign` from them.
  let options = BundlerOptions {
    input: Some(vec![
      InputItem { name: Some("app".into()), import: "./app.js".into() },
      InputItem { name: Some("dep".into()), import: "./dep.js".into() },
    ]),
    code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
      include_dependencies_recursively: Some(false),
      groups: Some(vec![MatchGroup {
        name: MatchGroupName::Static("group".to_string()),
        test: Some(MatchGroupTest::Regex(HybridRegex::new(r"seg_[ab]\.js$").unwrap())),
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
      vec![super::cross_chunk_reexport_fixture::emit_chunks(&["./seg_a.js", "./seg_b.js"])],
    )
    .await;
}
