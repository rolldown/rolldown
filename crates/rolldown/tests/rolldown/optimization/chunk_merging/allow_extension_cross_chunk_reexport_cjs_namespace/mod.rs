use rolldown::{BundlerOptions, InputItem, OutputFormat};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_cjs_namespace() {
  // The `group` chunk holds the modules of the three emitted AllowExtension entries. Each entry
  // re-exports `foreign` from the `dep.js` chunk, so it keeps its facade (issue #11767). Two
  // facades also re-export a property of a namespace object in another chunk:
  // - `seg_named.js` re-exports `x` from a CommonJS module. The namespace is in the `group` chunk.
  // - `seg_barrel.js` re-exports `y` from `barrel.js`, which gets `y` through `export *` from a
  //   CommonJS module. The namespace of `barrel.js` is in the `barrel.js` chunk.
  // In CJS output, the read of such a property must get the namespace through the `require()`
  // binding of the other chunk. The `index.js` entry also re-exports `y` this way.
  //
  // `seg_merged.js` imports `keep` from `merged_lib.cjs` and re-exports `w` from it. The two
  // import records get one namespace after the link pass (`merge_cjs_namespace`), but the
  // `group` chunk exports the namespace of the second record. `index.js` re-exports `keep`, so
  // `merged_lib.cjs` is not in the `seg_merged.js` chunk.
  //
  // `index.js` loads `seg_named.js` and `seg_star.js` with `import()`. That loads the `group`
  // chunk and reads the namespace object of the entry, as it does when the facade is removed.
  // `seg_star.js` has `export *` from `star_lib.cjs`, so its namespace object gets `z` at run
  // time. `index.js` re-exports `z`, so `star_lib.cjs` is not in the `seg_star.js` chunk. If
  // `seg_star.js` used `lib.cjs`, it would also export `x`. Then the name check keeps the facade
  // of `seg_named.js`, and `import()` loads that facade.
  manual_integration_test!()
    .build(TestMeta { expect_executed: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: Some("index".into()), import: "./index.js".into() }]),
        format: Some(OutputFormat::Cjs),
        code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
          include_dependencies_recursively: Some(false),
          groups: Some(vec![MatchGroup {
            name: MatchGroupName::Static("group".to_string()),
            test: Some(MatchGroupTest::Regex(HybridRegex::new("seg_").unwrap())),
            ..Default::default()
          }]),
          ..Default::default()
        })),
        ..Default::default()
      },
      vec![super::cross_chunk_reexport_fixture::emit_chunks(&[
        "./seg_named.js",
        "./seg_barrel.js",
        "./seg_star.js",
        "./seg_merged.js",
      ])],
    )
    .await;
}
