use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_mixed_entry() {
  // The `group` chunk holds the modules of eight emitted AllowExtension entries. Each ESM entry
  // re-exports `foreign` from the `dep.js` chunk with a different name. `seg_pure.js` has only
  // ESM exports, so it keeps its facade (issue #11767). Each other entry has an export that can
  // come from CommonJS, or an `export *` with names that the linker does not know. Thus the chunk
  // optimizer removes the facade of each other entry, as it does without the cross-chunk check.
  // Each other entry tests one condition:
  // - `seg_star.js`: `export *` from a CommonJS module that has no names that the scanner finds.
  // - `seg_barrel.js`: the same `export *` through an ESM module.
  // - `seg_external.js`: `export *` from an external module.
  // - `seg_namespace.js`: the namespace of a CommonJS module.
  // - `seg_named.js`: a named re-export from a CommonJS module.
  // - `seg_commonjs.cjs`: a CommonJS entry that re-exports `required.js` from another chunk.
  // `seg_external_named.js` re-exports `sep` from `node:path`. In CJS output, `sep` is a property
  // of the namespace of `node:path`, so the chunk optimizer also removes this facade. In ESM
  // output, the entry keeps its facade.
  // The `cjs` group holds the CommonJS module and the ESM module that re-exports it. Without that
  // group, the chunk optimizer keeps the facade of `seg_barrel.js` for a different reason. The
  // test checks the facade files and the exports in ESM and CJS output.
  let options = BundlerOptions {
    input: Some(vec![InputItem { name: Some("index".into()), import: "./index.js".into() }]),
    external: Some(vec!["node:path".to_string()].into()),
    code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
      include_dependencies_recursively: Some(false),
      groups: Some(vec![
        MatchGroup {
          name: MatchGroupName::Static("group".to_string()),
          test: Some(MatchGroupTest::Regex(HybridRegex::new("seg_").unwrap())),
          ..Default::default()
        },
        MatchGroup {
          name: MatchGroupName::Static("cjs".to_string()),
          test: Some(MatchGroupTest::Regex(HybridRegex::new("cjs_").unwrap())),
          ..Default::default()
        },
      ]),
      ..Default::default()
    })),
    ..Default::default()
  };
  manual_integration_test!()
    .build(TestMeta { expect_executed: true, snapshot: false, ..Default::default() })
    .run_multiple(
      super::cross_chunk_reexport_fixture::es_and_cjs(options),
      vec![super::cross_chunk_reexport_fixture::emit_chunks(&[
        "./seg_pure.js",
        "./seg_star.js",
        "./seg_barrel.js",
        "./seg_external.js",
        "./seg_external_named.js",
        "./seg_namespace.js",
        "./seg_named.js",
        "./seg_commonjs.cjs",
      ])],
    )
    .await;
}
