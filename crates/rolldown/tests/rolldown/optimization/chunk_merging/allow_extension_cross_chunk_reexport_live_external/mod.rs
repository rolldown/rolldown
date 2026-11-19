use rolldown::{BundlerOptions, InputItem, OutputFormat};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{
  integration_test::NamedBundlerOptions, manual_integration_test, test_config::TestMeta,
};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_live_external() {
  // `seg.js` is an emitted AllowExtension entry, and the `group` chunk holds its module. The entry
  // re-exports `foreign` from the `dep.js` chunk, so it keeps its facade (issue #11767). It also
  // re-exports `value` and `change` from `barrel.js`, which gets them through `export *` from the
  // external module `ext`. `value` is a live binding of an ES module, not a CommonJS property.
  // Thus the facade does not save its value:
  // - In ESM output, the facade re-exports `value` from `ext`.
  // - In CJS output, the facade exports a getter of `value`, or a copy with
  //   `externalLiveBindings: false`.
  let options = BundlerOptions {
    input: Some(vec![InputItem { name: Some("dep".into()), import: "./dep.js".into() }]),
    external: Some(vec!["ext".to_string()].into()),
    code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
      include_dependencies_recursively: Some(false),
      groups: Some(vec![MatchGroup {
        name: MatchGroupName::Static("group".to_string()),
        test: Some(MatchGroupTest::Regex(HybridRegex::new(r"(seg|barrel)\.js$").unwrap())),
        ..Default::default()
      }]),
      ..Default::default()
    })),
    ..Default::default()
  };
  manual_integration_test!()
    .build(TestMeta { expect_executed: true, snapshot: false, ..Default::default() })
    .run_multiple(
      vec![
        NamedBundlerOptions {
          description: None,
          options: options.clone(),
          snapshot: None,
          config_name: None,
          expect_execution_failure: None,
        },
        NamedBundlerOptions {
          description: Some("cjs".to_string()),
          options: BundlerOptions { format: Some(OutputFormat::Cjs), ..options.clone() },
          snapshot: None,
          config_name: Some("cjs".to_string()),
          expect_execution_failure: None,
        },
        NamedBundlerOptions {
          description: Some("cjs-no-live-bindings".to_string()),
          options: BundlerOptions {
            format: Some(OutputFormat::Cjs),
            external_live_bindings: Some(false),
            ..options
          },
          snapshot: None,
          config_name: Some("cjs-no-live-bindings".to_string()),
          expect_execution_failure: None,
        },
      ],
      vec![super::cross_chunk_reexport_fixture::emit_chunks(&["./seg.js"])],
    )
    .await;
}
