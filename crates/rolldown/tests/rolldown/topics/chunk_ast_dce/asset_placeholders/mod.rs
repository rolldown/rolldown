use rolldown::{BundlerOptions, InputItem, ModuleType};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

async fn run(name: &str, asset: bool, full_minify: bool) {
  manual_integration_test!()
    .build(TestMeta { snapshot: false, config_name: Some(name.into()), ..Default::default() })
    .run(BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("main".into()),
        import: if asset { "./asset.js" } else { "./copy.js" }.into(),
      }]),
      module_types: Some(
        std::iter::once((
          if asset { ".txt" } else { ".mjs" }.into(),
          if asset { ModuleType::Asset } else { ModuleType::Copy },
        ))
        .collect(),
      ),
      minify: full_minify.then(|| true.into()),
      dir: Some(format!("dist/{name}")),
      entry_filenames: Some("entries/[name].js".to_string().into()),
      asset_filenames: Some("assets/[name][extname]".to_string().into()),
      ..Default::default()
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn asset_and_copy_placeholders() {
  // The harness scans the fixture directory before building, so even distinct output directories
  // must not be removed by another variant during that scan.
  for (name, asset, full_minify) in [
    ("asset_full_minify", true, true),
    ("asset_dce_only", true, false),
    ("copy_full_minify", false, true),
    ("copy_dce_only", false, false),
  ] {
    run(name, asset, full_minify).await;
  }
}
