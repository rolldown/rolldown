use rolldown::{BundlerOptions, InputItem, OutputFormat};
use rolldown_common::{
  CodeSplittingMode, ManualCodeSplittingOptions, MatchGroup, MatchGroupName, MatchGroupTest,
};
use rolldown_testing::{
  integration_test::NamedBundlerOptions, manual_integration_test, test_config::TestMeta,
};
use rolldown_utils::js_regex::HybridRegex;

#[tokio::test(flavor = "multi_thread")]
async fn allow_extension_cross_chunk_reexport_shared_read() {
  // `seg.js` is an emitted AllowExtension entry, and the `group` chunk holds its module,
  // `barrel.js` and the CommonJS modules. The entry re-exports `foreign` from the `dep.js` chunk,
  // so it keeps its facade (issue #11767). The facade and the `app.js` entry re-export the same
  // properties of CommonJS modules. Node reads all properties of a CommonJS module one time, in
  // source order, when it imports the module. So both entries call one accessor for each module in
  // the `group` chunk, and the first call reads the properties of the module:
  // - The getter of `n` counts its reads. Both entries get `1`, in each load order.
  // - The getter of `x` throws. Both entries get `undefined`, and the getter runs one time.
  // - `mut.cjs` writes `m`. After the first read, both entries keep the value of that read. In CJS
  //   output, the first read runs at the first access of an export of `mut.cjs`, so it can see a
  //   write that Node does not see. Node reads `m` when it imports `mut.cjs`.
  // `other.js` re-exports `reads` of `counter.cjs`. The accessor of `counter.cjs` also reads `n`,
  // so the getter of `n` runs one time, as in Node.
  //
  // `seg_barrel.js` is also an emitted entry in the `group` chunk, and it also re-exports
  // `foreign`. It re-exports `y` from `barrel.js`, which gets `y` through `export *` from two
  // CommonJS modules. The output cannot tell which module gives `y`, so the early check rejects
  // the property. Thus the entry gets no facade, as without the cross-chunk check, and code
  // outside the bundle does not get `foreign` from it. `app.js` reads `y` itself.
  let options = BundlerOptions {
    input: Some(vec![
      InputItem { name: Some("app".into()), import: "./app.js".into() },
      InputItem { name: Some("other".into()), import: "./other.js".into() },
      InputItem { name: Some("dep".into()), import: "./dep.js".into() },
    ]),
    code_splitting: Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
      include_dependencies_recursively: Some(false),
      groups: Some(vec![MatchGroup {
        name: MatchGroupName::Static("group".to_string()),
        test: Some(MatchGroupTest::Regex(
          HybridRegex::new(r"(seg\.js|barrel\.js|\.cjs)$").unwrap(),
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
      vec![
        NamedBundlerOptions {
          description: None,
          options: options.clone(),
          snapshot: None,
          config_name: None,
          expect_execution_failure: None,
        },
        // In CJS output, the exports of `app.js` are getters that call the accessor.
        NamedBundlerOptions {
          description: Some("cjs".to_string()),
          options: BundlerOptions { format: Some(OutputFormat::Cjs), ..options },
          snapshot: None,
          config_name: Some("cjs".to_string()),
          expect_execution_failure: None,
        },
      ],
      vec![super::cross_chunk_reexport_fixture::emit_chunks(&["./seg.js", "./seg_barrel.js"])],
    )
    .await;
}
