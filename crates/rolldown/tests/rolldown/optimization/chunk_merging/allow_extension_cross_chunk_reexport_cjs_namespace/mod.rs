use rolldown::{BundlerOptions, InputItem, OutputFormat};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_cjs_namespace() {
  // The `group` chunk holds the modules of the four emitted AllowExtension entries. Each entry
  // re-exports `foreign` from the `dep.js` chunk. Each entry also re-exports a value from CommonJS
  // that the cross-chunk check does not accept. Thus each entry gets no facade, as without the
  // cross-chunk check, and code outside the bundle does not get `foreign` from it:
  // - `seg_named.js` re-exports `x` from `lib.cjs`. `index.js` loads the entry with `import()`, so
  //   the early check treats the `group` chunk as a chunk that cannot get a new export. A facade
  //   then cannot call an accessor, and the early check rejects `lib.cjs`.
  // - `seg_barrel.js` re-exports `y` from `barrel.js`, which gets `y` through `export *` from a
  //   CommonJS module. The read uses the namespace of `barrel.js`, so the early check rejects
  //   `barrel_lib.cjs`. The `index.js` entry also re-exports `y` this way.
  // - `seg_merged.js` imports `keep` from `merged_lib.cjs` and re-exports `w` from it. Code reads
  //   `keep` outside a re-export, so the early check rejects `merged_lib.cjs`.
  // - `seg_star.js` has `export *` from `star_lib.cjs`, so the linker does not know all its
  //   exports.
  //
  // `index.js` loads `seg_named.js` and `seg_star.js` with `import()`. That loads the `group`
  // chunk and reads the namespace object of the entry. The namespace object of `seg_star.js` gets
  // `z` at run time. `index.js` re-exports `z`, so `star_lib.cjs` is not in the `seg_star.js`
  // chunk. If `seg_star.js` used `lib.cjs`, it would also export `x`. Then the name check keeps
  // the facade of `seg_named.js`, and `import()` loads that facade.
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
