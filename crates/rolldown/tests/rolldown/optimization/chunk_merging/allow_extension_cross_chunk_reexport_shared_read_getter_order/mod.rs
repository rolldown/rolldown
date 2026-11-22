use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_shared_read_getter_order() {
  // `seg.js` is an emitted AllowExtension entry, and the `group` chunk holds its module. The entry
  // re-exports `foreign` from the `dep.js` chunk, so it keeps its facade (issue #11767). The facade
  // re-exports `y` of `xy.cjs`, `yx.cjs`, `defxy.cjs` and `defyx.cjs`, and the `app.js` entry
  // re-exports `x` of each CommonJS module. The getters of a module share one counter, so each
  // value depends on the order of the reads. Node reads all properties of a CommonJS module one
  // time, in source order: `x` first in `xy.cjs` and `defxy.cjs`, and `y` first in `yx.cjs` and
  // `defyx.cjs`. In `defxy.cjs` and `defyx.cjs`, the first place of a property is a call of
  // `Object.defineProperty` whose getter only returns a property, a form that Node finds. One
  // accessor for each module reads the properties in this order. Thus the values do not depend on
  // the entry that loads first, and an entry that reads one property also reads the other
  // property.
  //
  // `side.js` imports only `own.js`, an ES module in the `group` chunk. Thus the chunk of `side.js`
  // loads the chunks of the accessors, but no getter runs before the first read.
  //
  // `seg_unknown.js` is also an emitted entry in the `group` chunk, and it also re-exports
  // `foreign`. It re-exports `y` of `unknown.cjs`. The first place of `x` in `unknown.cjs` is a
  // call of `Object.defineProperty` in a form that the bundler does not identify, so the order of
  // its properties is not known. Thus the early check rejects `unknown.cjs`, and the entry gets no
  // facade, as without the cross-chunk check. Code outside the bundle does not get `foreign` from
  // it.
  let options = BundlerOptions {
    input: Some(vec![
      InputItem { name: Some("app".into()), import: "./app.js".into() },
      InputItem { name: Some("dep".into()), import: "./dep.js".into() },
      InputItem { name: Some("side".into()), import: "./side.js".into() },
    ]),
    code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
      include_dependencies_recursively: Some(false),
      groups: Some(vec![MatchGroup {
        name: MatchGroupName::Static("group".to_string()),
        test: Some(MatchGroupTest::Regex(HybridRegex::new(r"(seg|seg_unknown|own)\.js$").unwrap())),
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
      vec![super::cross_chunk_reexport_fixture::emit_chunks(&["./seg.js", "./seg_unknown.js"])],
    )
    .await;
}
