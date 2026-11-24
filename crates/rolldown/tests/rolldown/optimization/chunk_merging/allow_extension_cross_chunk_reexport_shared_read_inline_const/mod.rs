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
  // `counter.cjs`. The entry re-exports `foreign` from the `dep.js` chunk. The entry also
  // re-exports the constant `n` of `counter.cjs` as `first`. `main.js` loads the entry with
  // `import()`, so the early check treats the `group` chunk as a chunk that cannot get a new
  // export. A facade then cannot call an accessor in that chunk, and the early check rejects
  // `counter.cjs`. Thus the entry gets no facade, as without the cross-chunk check, and code
  // outside the bundle does not get `foreign` from it. The `import()` gives the namespace object of
  // the entry from the `group` chunk. `inlineConst: true` puts the constant in the getter of
  // `first`, so the getter does not read `n`.
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
