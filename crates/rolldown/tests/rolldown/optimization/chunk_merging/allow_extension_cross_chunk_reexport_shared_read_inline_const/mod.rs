use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, InlineConstOption, ManualCodeSplittingOptions, MatchGroup, MatchGroupName,
  MatchGroupTest, OptimizationOption,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_shared_read_inline_const() {
  // `seg.js` is an emitted AllowExtension entry, and the `group` chunk holds its module and
  // `counter.cjs`. The entry re-exports `foreign` from the `dep.js` chunk, so it keeps its facade
  // (issue #11767). The entry also re-exports the constant `n` of `counter.cjs` as `first`.
  // `main.js` loads the entry with `import()`, which gives the namespace object of the entry from
  // the `group` chunk. `inlineConst: true` puts the constant in the getter of `first`, so the
  // getter does not read `n`. Thus the facade is the only chunk that reads `n`, and it keeps its
  // own read. The output has no accessor and no forward.
  manual_integration_test!()
    .build(TestMeta { expect_executed: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![
          InputItem { name: Some("dep".into()), import: "./dep.js".into() },
          InputItem { name: Some("main".into()), import: "./main.js".into() },
        ]),
        code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
          include_dependencies_recursively: Some(false),
          groups: Some(vec![MatchGroup {
            name: MatchGroupName::Static("group".to_string()),
            test: Some(MatchGroupTest::Regex(
              HybridRegex::new(r"(seg\.js|counter\.cjs)$").unwrap(),
            )),
            ..Default::default()
          }]),
          ..Default::default()
        })),
        optimization: Some(OptimizationOption {
          inline_const: Some(InlineConstOption::Bool(true)),
          ..Default::default()
        }),
        ..Default::default()
      },
      vec![super::cross_chunk_reexport_fixture::emit_chunks(&["./seg.js"])],
    )
    .await;
}
