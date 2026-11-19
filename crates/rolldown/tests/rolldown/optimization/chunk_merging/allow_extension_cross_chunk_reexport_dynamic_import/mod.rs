use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_dynamic_import() {
  // The six emitted AllowExtension entries re-export `foreign` from the `dep.js` chunk, so each
  // entry keeps its facade for code outside the bundle (issue #11767). The test loads the facades
  // directly. An `import()` in the bundle loads the `group` chunk and reads the namespace object
  // of the entry, as it does when the facade is removed. The facade gives a different value:
  // - `seg_dynamic.js` has `export *` from a CommonJS module, and `seg_barrel.js` has the same
  //   through `barrel.js`. The namespace object gets the names of a CommonJS module at run time,
  //   so `import()` gets `x`. A facade does not re-export the names that `export *` gets from a
  //   CommonJS module.
  // - `seg_getter.js` re-exports `boom` from a CommonJS module that throws when code reads
  //   `boom`. The facade and the getter of `boom` in the namespace object share one read in a
  //   `try` block. The facade reads `boom` when it loads, and the namespace object reads it only
  //   on access. Thus the facade loads and `import()` resolves, and both give `undefined`, as in
  //   Node.
  // - `reader.js` reads the namespace object of `seg_namespace.js` as a value, and `import()`
  //   returns the same object.
  // `seg_external.js` has `export *` from an external module. Only its facade re-exports the
  // names of that module. The namespace object does not get them (a known gap), so the test
  // checks them only on the facade. `seg_static.js` is not an `import()` target.
  manual_integration_test!()
    .build(TestMeta { expect_executed: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: Some("index".into()), import: "./index.js".into() }]),
        external: Some(vec!["node:path".to_string()].into()),
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
        "./seg_dynamic.js",
        "./seg_barrel.js",
        "./seg_static.js",
        "./seg_external.js",
        "./seg_getter.js",
        "./seg_namespace.js",
      ])],
    )
    .await;
}
