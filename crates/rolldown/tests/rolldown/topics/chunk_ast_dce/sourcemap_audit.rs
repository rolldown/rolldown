use std::path::Path;

use rolldown::{Bundler, SourceMapType};
use rolldown_common::Output;
use rolldown_testing::test_config::read_test_config;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown");

#[tokio::test(flavor = "multi_thread")]
async fn original_source_positions() {
  let fixtures = [
    "misc/object_shorthand_property",
    "misc/wrapped_esm",
    "sourcemap/imported_cjs_callee",
    "topics/deconflict/complex_params_patterns",
    "topics/deconflict/decl_complex_patterns",
    "topics/deconflict/decl_nested_assign_pattern",
    "topics/deconflict/default_function",
    "topics/deconflict/wrapped_esm_default_function",
    "topics/deconflict/wrapped_esm_export_named_function",
    "tree_shaking/multi-declarator",
    "function/minify/basic",
    "function/module_types/copy/basic",
    "function/module_types/copy/nested_path",
    "function/module_types/asset",
    "sourcemap/issue_6099",
  ];
  let mut captures = Vec::new();
  for fixture in fixtures {
    let cwd = Path::new(FIXTURES).join(fixture);
    let config = read_test_config(&cwd.join("_config.json"));
    let mut variants = vec![("default".to_string(), config.config.clone())];
    variants.extend(
      config
        .config_variants
        .iter()
        .enumerate()
        .map(|(index, variant)| (format!("variant-{index}"), variant.apply(&config.config))),
    );
    for (variant, mut options) in variants {
      options.cwd = Some(cwd.clone());
      options.input.get_or_insert_with(|| {
        vec![rolldown::InputItem { name: Some("main".into()), import: "./main.js".into() }]
      });
      options.sourcemap = Some(SourceMapType::File);
      let output = Bundler::new(options).unwrap().generate().await.unwrap();
      for asset in &output.assets {
        let Output::Chunk(chunk) = asset else { continue };
        let json = chunk.map.as_ref().unwrap().to_json_string();
        let map = oxc_sourcemap::SourceMap::from_json_string(&json).unwrap();
        super::map_assertions::assert_valid_map(&chunk.code, &map);
        super::sourcemap_audit_cases::assert_positions(fixture, &variant, &chunk.code, &map);
        captures.push(serde_json::json!({
          "fixture": fixture,
          "variant": variant,
          "filename": chunk.filename.as_str(),
          "code": chunk.code,
          "map": map.to_json_string(),
        }));
      }
    }
  }
  if let Ok(path) = std::env::var("ROLLDOWN_MAP_AUDIT_OUTPUT") {
    std::fs::write(path, serde_json::to_string_pretty(&captures).unwrap()).unwrap();
  }
}
