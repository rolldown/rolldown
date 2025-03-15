// Since we have customized the output `entry_filenames`, 
// the `compiled_entries` in the following context is incorrect.
//
// /crates/rolldown_testing/src/integration_test.rs#L471-L484
//
// if test_script.exists() {
//   node_command.arg(test_script);
// } else {
//   let compiled_entries = bundler
//     .options()
//     .input
//     .iter()
//     .map(|item| {
//       let name = item.name.clone().expect("inputs must have `name` in `_config.json`");
//       let ext = "js";
//       format!("{name}.{ext}",)
//     })
//     .map(|name| dist_folder.join(name))
//     .collect::<Vec<_>>();
//  ...
//  }