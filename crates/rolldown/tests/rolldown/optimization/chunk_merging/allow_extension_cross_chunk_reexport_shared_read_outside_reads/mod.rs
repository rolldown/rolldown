use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_shared_read_outside_reads() {
  // `seg_a.js`, `seg_b.js` and `seg_c.js` are emitted AllowExtension entries, and the `group` chunk
  // holds their modules. Each entry re-exports `foreign` from the `dep.js` chunk and `y` from its
  // own CommonJS module. The `group` chunk also holds the CommonJS modules, so each entry can share
  // the read of `y`. But code reads the other property `x` of each module outside a shared read, so
  // the early check rejects each module with a different rule:
  // - `counter_a.cjs`: `app.js` reads `x` in its body. A shared read reads all properties at its
  //   first call, so the read in code can get another value.
  // - `counter_b.cjs`: `app.js` passes the namespace object of `other.js` to a function. A getter
  //   of that object reads `x` each time that code reads it.
  // - `counter_c.cjs`: `app.js` imports `lazy.js`, so the `app.js` chunk holds it, and the
  //   optimizer removes the facade of the dynamic entry `lazy.js`. Thus `import()` of `lazy.js`
  //   gives the namespace object of `lazy.js`, which has a getter of `x`.
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
        test: Some(MatchGroupTest::Regex(
          HybridRegex::new(r"(seg_[abc]\.js|counter_[abc]\.cjs)$").unwrap(),
        )),
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
      vec![super::cross_chunk_reexport_fixture::emit_chunks(&[
        "./seg_a.js",
        "./seg_b.js",
        "./seg_c.js",
      ])],
    )
    .await;
}
