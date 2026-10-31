//! PoC (`poc/chunk-ast-minify`): code that oxc's DCE moves across modules (single-use inlining)
//! out of modules whose text came from a plugin `transform` with a sourcemap. In the chunk AST
//! DCE `global` mode, the moved code must map through the *source* module's `sourcemap_chain`.
//! Run with `ROLLDOWN_CHUNK_AST_DCE=global` and compare the visualized sourcemap with the default.
use std::{
  borrow::Cow,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
};

use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_common::Output;
use rolldown_plugin::{
  HookTransformArgs, HookTransformOutput, HookTransformReturn, HookUsage, Plugin,
  SharedTransformPluginContext,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};
use string_wizard::{MagicString, SourceMapOptions};

/// Prepends two comment lines to every fixture module, with an exact sourcemap.
#[derive(Debug)]
struct ShiftLinesPlugin {
  transformed: Arc<AtomicUsize>,
}

impl Plugin for ShiftLinesPlugin {
  fn name(&self) -> Cow<'static, str> {
    "shift-lines".into()
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    if !args.id.ends_with(".js") || args.id.starts_with('\0') {
      return Ok(None);
    }
    self.transformed.fetch_add(1, Ordering::Relaxed);
    let mut magic_string = MagicString::new(args.code.as_str());
    magic_string.prepend("// shifted by a plugin\n// shifted by a plugin\n");
    Ok(Some(HookTransformOutput {
      code: Some(magic_string.to_string()),
      map: magic_string
        .source_map(SourceMapOptions {
          hires: string_wizard::Hires::True,
          include_content: true,
          source: Arc::from(args.id),
        })
        .into(),
      ..Default::default()
    }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn composed_source_positions() {
  for full in [false, true] {
    let transformed = Arc::new(AtomicUsize::new(0));
    let output = Bundler::with_plugins(
      BundlerOptions {
        cwd: Some(
          concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/rolldown/topics/chunk_ast_dce/plugin_sourcemap_chain"
          )
          .into(),
        ),
        input: Some(vec![InputItem { name: Some("entry".into()), import: "./entry.js".into() }]),
        sourcemap: Some(SourceMapType::File),
        minify: full.then(|| true.into()),
        ..Default::default()
      },
      vec![Plugin::new_shared(ShiftLinesPlugin { transformed: Arc::clone(&transformed) })],
    )
    .unwrap()
    .generate()
    .await
    .unwrap();
    assert_eq!(
      transformed.load(Ordering::Relaxed),
      3,
      "all fixture producers must pass through the transform chain"
    );
    for asset in &output.assets {
      let Output::Chunk(chunk) = asset else { continue };
      let json = chunk.map.as_ref().unwrap().to_json_string();
      let map = oxc_sourcemap::SourceMap::from_json_string(&json).unwrap();
      let map = &map;
      super::map_assertions::assert_valid_map(&chunk.code, map);
      if std::env::var("ROLLDOWN_CHUNK_AST_DCE").is_ok_and(|mode| mode == "global") {
        super::map_assertions::assert_unmapped(
          &chunk.code,
          map,
          if full { "export{" } else { "export {" },
        );
      }
      if std::env::var("ROLLDOWN_CHUNK_AST_DCE").is_ok_and(|mode| mode == "global") {
        let marker = "#ifdef USE_MAP";
        let start = chunk.code.find(marker).unwrap();
        let quote = chunk.code[..start].rfind(['\'', '"', '`']).unwrap();
        super::map_assertions::assert_mapping_at(
          &chunk.code,
          map,
          quote,
          "shader.js",
          include_str!("shader.js"),
          include_str!("shader.js").find('`').unwrap(),
        );
      }
      let literal = if full { "`helper`" } else { "\"helper\"" };
      for (generated, original) in
        [("name:", "name:"), (literal, "'helper',"), ("make:", "make:"), ("Symbol(", "Symbol(")]
      {
        super::map_assertions::assert_mapping(
          &chunk.code,
          map,
          generated,
          "lib.js",
          include_str!("lib.js"),
          original,
        );
      }
      if !full {
        super::map_assertions::assert_mapping(
          &chunk.code,
          map,
          "ShaderChunk =",
          "entry.js",
          include_str!("entry.js"),
          "ShaderChunk =",
        );
      }
      super::map_assertions::assert_mapping(
        &chunk.code,
        map,
        "shader:",
        "entry.js",
        include_str!("entry.js"),
        "shader, helper",
      );
    }
    if let Ok(path) = std::env::var("ROLLDOWN_PLUGIN_MAP_AUDIT_OUTPUT") {
      let chunks: Vec<_> = output.assets.iter().filter_map(|asset| {
      let Output::Chunk(chunk) = asset else { return None };
      Some(serde_json::json!({ "code": chunk.code, "map": chunk.map.as_ref().unwrap().to_json_string() }))
    }).collect();
      std::fs::write(format!("{path}.{full}.json"), serde_json::to_string_pretty(&chunks).unwrap())
        .unwrap();
    }
  }
}

#[derive(Debug)]
struct EmptyMapPlugin;

impl Plugin for EmptyMapPlugin {
  fn name(&self) -> Cow<'static, str> {
    "empty-map".into()
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    if !args.id.ends_with("dep2.js") {
      return Ok(None);
    }
    Ok(Some(HookTransformOutput {
      code: Some("export default 'dep2-generated'".into()),
      map: rolldown_sourcemap::empty_sourcemap().into(),
      ..Default::default()
    }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_producer_map_terminates_foreign_mapping() {
  for full in [false, true] {
    let output = Bundler::with_plugins(
      BundlerOptions {
        cwd: Some(
          concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/rolldown/tests/fixtures/misc/transform-empty-mappings"
          )
          .into(),
        ),
        input: Some(vec![InputItem { name: Some("entry".into()), import: "./main.js".into() }]),
        sourcemap: Some(SourceMapType::File),
        minify: full.then(|| true.into()),
        ..Default::default()
      },
      vec![Plugin::new_shared(EmptyMapPlugin)],
    )
    .unwrap()
    .generate()
    .await
    .unwrap();
    for asset in &output.assets {
      let Output::Chunk(chunk) = asset else { continue };
      let json = chunk.map.as_ref().unwrap().to_json_string();
      let map = oxc_sourcemap::SourceMap::from_json_string(&json).unwrap();
      super::map_assertions::assert_valid_map(&chunk.code, &map);
      if std::env::var("ROLLDOWN_CHUNK_AST_DCE").is_ok_and(|mode| mode == "global") {
        super::map_assertions::assert_unmapped(&chunk.code, &map, "dep2-generated");
        let lookup = map.generate_lookup_table();
        for marker in ["dep1-test", "dep3-test"] {
          let prefix = &chunk.code[..chunk.code.find(marker).unwrap()];
          let line = u32::try_from(prefix.bytes().filter(|b| *b == b'\n').count()).unwrap();
          let column = u32::try_from(prefix.rsplit('\n').next().unwrap().len()).unwrap();
          let token = map.lookup_source_view_token(&lookup, line, column).unwrap();
          assert!(token.get_source().unwrap().ends_with(if marker == "dep1-test" {
            "dep1.js"
          } else {
            "dep3.js"
          }));
        }
      }
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn plugin_sourcemap_chain() {
  manual_integration_test!()
    .build(TestMeta { visualize_sourcemap: true, expect_executed: false, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        sourcemap: Some(SourceMapType::File),
        ..Default::default()
      },
      vec![Plugin::new_shared(ShiftLinesPlugin { transformed: Arc::default() })],
    )
    .await;
}
