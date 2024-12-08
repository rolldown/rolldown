use rolldown::{BundleOutput, Bundler, BundlerOptions, InputItem};
use rolldown_testing::abs_file_dir;

#[tokio::test(flavor = "multi_thread")]
async fn test_rebuild_basic() {
  // prepare tmp dir to modify source
  let cwd = abs_file_dir!();
  let tmp = cwd.join("dist");
  std::fs::create_dir_all(&tmp).unwrap();
  for file in ["entry.js", "dep.js"] {
    std::fs::copy(cwd.join(file), tmp.join(file)).unwrap();
  }

  let options = BundlerOptions {
    input: Some(vec![InputItem {
      name: Some("entry".to_string()),
      import: "./entry.js".to_string(),
    }]),
    cwd: Some(tmp.clone()),
    ..Default::default()
  };
  let mut bundler = Bundler::new(options);
  bundler.set_rebuild_enabled(true);

  let mut snapshot_output = String::new();

  // build
  let output = bundler.write().await.unwrap();
  snapshot_output.push_str("# build\n\n");
  format_bundle_output(&mut snapshot_output, &output);

  // edit
  let js_file = tmp.join("dep.js");
  std::fs::write(
    &js_file,
    std::fs::read_to_string(&js_file).unwrap().replace("[dep]", "[dep-edit]"),
  )
  .unwrap();

  // rebuild
  let output = bundler.write().await.unwrap();
  snapshot_output.push_str("# rebuild\n\n");
  format_bundle_output(&mut snapshot_output, &output);

  let mut settings = insta::Settings::clone_current();
  settings.set_snapshot_path(cwd);
  settings.set_prepend_module_to_snapshot(false);
  settings.remove_input_file();
  settings.set_omit_expression(true);
  settings.bind(|| {
    insta::assert_snapshot!("artifacts", snapshot_output);
  });
}

fn format_bundle_output(snapshot: &mut String, output: &BundleOutput) {
  output.assets.iter().for_each(|asset| match asset {
    rolldown_common::Output::Asset(_) => {}
    rolldown_common::Output::Chunk(chunk) => {
      snapshot.push_str(&format!("- {}\n\n```js\n{}\n```\n\n", chunk.name, chunk.code));
    }
  });
}
