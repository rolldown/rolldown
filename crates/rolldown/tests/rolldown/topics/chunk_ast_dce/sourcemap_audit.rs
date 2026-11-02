use std::path::Path;

use rolldown::{Bundler, SourceMapType};
use rolldown_common::Output;
use rolldown_common::RawMinifyOptions;
use rolldown_testing::test_config::read_test_config;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown");

#[tokio::test(flavor = "multi_thread")]
async fn changed_snapshot_maps_preserve_sources_and_generated_facades() {
  let fixtures = [
    "cjs_compat/mix-cjs-esm",
    "function/inline_common_chunks/sourcemap",
    "sourcemap/issue_10070",
    "sourcemap/live_binding",
    "topics/import_meta_url_dirname_filename_polyfill/node_cjs",
  ];
  let mut captures = Vec::new();
  for fixture in fixtures {
    for full in [false, true] {
      let cwd = Path::new(FIXTURES).join(fixture);
      let mut options = read_test_config(&cwd.join("_config.json")).config;
      options.cwd = Some(cwd.clone());
      options.input.get_or_insert_with(|| {
        vec![rolldown::InputItem { name: Some("main".into()), import: "./main.js".into() }]
      });
      options.sourcemap = Some(SourceMapType::File);
      options.minify = Some(if full {
        RawMinifyOptions::Bool(true)
      } else {
        RawMinifyOptions::DeadCodeEliminationOnly
      });
      let output = Bundler::new(options).unwrap().generate().await.unwrap();
      for asset in &output.assets {
        let Output::Chunk(chunk) = asset else { continue };
        // Generated-only facades have an empty map with no original-source attribution.
        let json = chunk.map.as_ref().unwrap().to_json_string();
        let map = oxc_sourcemap::SourceMap::from_json_string(&json).unwrap();
        super::map_assertions::assert_valid_map(&chunk.code, &map);
        assert!(
          output.assets.iter().any(|asset| matches!(asset, Output::Asset(asset)
            if asset.filename.as_str() == format!("{}.map", chunk.filename))),
          "emitted sourceMappingURL must resolve: {fixture}/{}",
          chunk.filename
        );
        if (fixture == "sourcemap/live_binding" && chunk.name.as_str() == "main2")
          || chunk.module_ids.iter().all(|id| id.as_str().starts_with('\0'))
        {
          assert!(map.get_source_view_tokens().all(|token| token.get_source_id().is_none()));
          let lookup = map.generate_lookup_table();
          for (byte, _) in chunk.code.char_indices() {
            let (line, column) = super::map_assertions::position(&chunk.code, byte);
            assert!(
              map
                .lookup_source_view_token(&lookup, line, column)
                .is_none_or(|token| token.get_source_id().is_none())
            );
          }
        } else {
          assert!(
            map.get_source_view_tokens().any(|token| token.get_source_id().is_some()),
            "expected mapped source: {fixture}/{}/{full}: {}",
            chunk.filename,
            chunk.code
          );
          for source in map.get_sources() {
            assert!(!source.contains("<chunk>"), "virtual space must not leak: {source}");
          }
          if fixture == "cjs_compat/mix-cjs-esm" {
            let source = std::fs::read_to_string(cwd.join("esm-import-cjs-require.js")).unwrap();
            super::map_assertions::assert_mapping(
              &chunk.code,
              &map,
              "equal(",
              "esm-import-cjs-require.js",
              &source,
              "equal(",
            );
          }
          if fixture == "topics/import_meta_url_dirname_filename_polyfill/node_cjs" {
            let source = std::fs::read_to_string(cwd.join("main.js")).unwrap();
            super::map_assertions::assert_mapping(
              &chunk.code,
              &map,
              "pathToFileURL(",
              "main.js",
              &source,
              "pathToFileURL(",
            );
          }
          if fixture == "sourcemap/issue_10070" {
            let source = std::fs::read_to_string(cwd.join("app.js")).unwrap();
            super::map_assertions::assert_mapping(
              &chunk.code,
              &map,
              "globalThis.side",
              "app.js",
              &source,
              "globalThis.side",
            );
          }
          if fixture == "function/inline_common_chunks/sourcemap" {
            let source = std::fs::read_to_string(cwd.join("shared.js")).unwrap();
            let anchor = chunk.code.find("S body").unwrap();
            let quoted = chunk.code[..anchor].char_indices().last().unwrap().0;
            super::map_assertions::assert_mapping_at(
              &chunk.code,
              &map,
              quoted,
              "shared.js",
              &source,
              source.find("'S body'").unwrap(),
            );
          }
          if !full && fixture == "sourcemap/live_binding" && chunk.name.as_str() == "main1" {
            let source = std::fs::read_to_string(cwd.join("main1.js")).unwrap();
            super::map_assertions::assert_mapping(
              &chunk.code,
              &map,
              "function ",
              "main1.js",
              &source,
              "function ",
            );
          }
          if fixture == "sourcemap/live_binding" && chunk.name.as_str() == "shared" {
            let source = std::fs::read_to_string(cwd.join("shared.js")).unwrap();
            super::map_assertions::assert_mapping(
              &chunk.code,
              &map,
              "console.trace",
              "shared.js",
              &source,
              "console.trace",
            );
          }
        }
        captures.push(serde_json::json!({
          "fixture": fixture, "full": full, "filename": chunk.filename.as_str(),
          "code": chunk.code, "map": json,
        }));
      }
    }
  }
  if let Ok(path) = std::env::var("ROLLDOWN_CHANGED_MAP_AUDIT_OUTPUT") {
    std::fs::write(path, serde_json::to_string_pretty(&captures).unwrap()).unwrap();
  }
}

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
