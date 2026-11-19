//! A build that is not incremental composes each run of consecutive transform maps before the
//! link stage, and drops the sourcemap chains after every module is rendered. An incremental
//! build keeps the chains as they are. Both builds must emit the same maps.
//!
//! `main.js` is loaded with a map and then goes through three transforms, so its chain is
//! `Load`, `Transform`, `Transform`, `Transform`. `dep.js` has three `Transform` elements.

use std::{borrow::Cow, path::PathBuf, sync::Arc};

use rolldown::{Bundler, BundlerOptions, ExperimentalOptions, InputItem, SourceMapType};
use rolldown_common::Output;
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookTransformArgs, HookTransformOutput,
  HookTransformReturn, HookUsage, Plugin, SharedLoadPluginContext, SharedTransformPluginContext,
};
use rolldown_sourcemap::{OwnedSourceMap, SourceMap};

fn fixture_dir() -> PathBuf {
  PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/topics/sourcemap_chain"))
}

fn is_fixture_module(id: &str) -> bool {
  id.ends_with("main.js") || id.ends_with("dep.js")
}

/// Prepends `line` to `code`, and returns the new code with a map back to `code`.
fn prepend(code: &str, id: &str, line: String) -> (String, SourceMap) {
  let mut magic_string = string_wizard::MagicString::new(code);
  magic_string.prepend(line);
  let map = magic_string.source_map(string_wizard::SourceMapOptions {
    include_content: true,
    source: Arc::from(id),
    hires: string_wizard::Hires::Boundary,
  });
  (magic_string.to_string(), map)
}

/// Loads `main.js` with a map, so the chain of `main.js` starts with a `Load` element.
#[derive(Debug)]
struct LoadWithMap;

impl Plugin for LoadWithMap {
  fn name(&self) -> Cow<'static, str> {
    "load-with-map".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if !args.id.ends_with("main.js") {
      return Ok(None);
    }
    let code = std::fs::read_to_string(args.id)?;
    let (code, map) = prepend(&code, args.id, "globalThis.loaded = true;\n".to_string());
    Ok(Some(HookLoadOutput { code: code.into(), map: Some(map), ..Default::default() }))
  }
}

/// Prepends a line to each fixture module and returns a map.
#[derive(Debug)]
struct PrependTransform(&'static str);

impl Plugin for PrependTransform {
  fn name(&self) -> Cow<'static, str> {
    self.0.into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    if !is_fixture_module(args.id) {
      return Ok(None);
    }
    let (code, map) = prepend(args.code, args.id, format!("globalThis.{} = true;\n", self.0));
    Ok(Some(HookTransformOutput { code: Some(code), map: map.into(), ..Default::default() }))
  }
}

fn bundler(incremental_build: bool) -> Bundler {
  Bundler::with_plugins(
    BundlerOptions {
      cwd: Some(fixture_dir()),
      input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
      sourcemap: Some(SourceMapType::File),
      experimental: Some(ExperimentalOptions {
        incremental_build: Some(incremental_build),
        ..Default::default()
      }),
      ..Default::default()
    },
    vec![
      Plugin::new_shared(LoadWithMap),
      Plugin::new_shared(PrependTransform("first")),
      Plugin::new_shared(PrependTransform("second")),
      Plugin::new_shared(PrependTransform("third")),
    ],
  )
  .expect("failed to create bundler")
}

/// The code and the map JSON of the only chunk.
async fn generate(bundler: &mut Bundler) -> (String, String) {
  let output = bundler.generate().await.expect("build should succeed");
  let chunks: Vec<_> = output
    .assets
    .iter()
    .filter_map(|output| match output {
      Output::Chunk(chunk) => Some(chunk),
      Output::Asset(_) => None,
    })
    .collect();
  assert_eq!(chunks.len(), 1, "should emit one chunk");
  let map = chunks[0].map.as_ref().expect("should emit a map");
  (chunks[0].code.clone(), map.to_json_string())
}

/// 0-based `(line, column)` of the only occurrence of `needle`.
fn position_of(text: &str, needle: &str) -> (u32, u32) {
  let mut matches = text.match_indices(needle);
  let (offset, _) = matches.next().unwrap_or_else(|| panic!("{needle:?} not found in:\n{text}"));
  assert!(matches.next().is_none(), "{needle:?} is not unique in:\n{text}");
  let before = &text[..offset];
  let line = before.matches('\n').count();
  let column = before.len() - before.rfind('\n').map_or(0, |newline| newline + 1);
  (u32::try_from(line).unwrap(), u32::try_from(column).unwrap())
}

/// The string literal `literal` in `code` must map to the same literal in the original `file`.
fn assert_maps_to(code: &str, map_json: &str, literal: &str, file: &str) {
  let map = OwnedSourceMap::from_json_string(map_json).unwrap();
  let map = map.as_source_map();
  let (line, column) = position_of(code, &format!("\"{literal}\""));
  let token = map
    .lookup_token(&map.generate_lookup_table(), line, column)
    .filter(|token| (token.get_dst_line(), token.get_dst_col()) == (line, column))
    .unwrap_or_else(|| panic!("{literal:?} at {line}:{column} has no token of its own"));
  let source = token.get_source_id().and_then(|id| map.get_source(id));
  assert!(source.is_some_and(|source| source.ends_with(file)), "{literal:?} maps to {source:?}");
  let content = std::fs::read_to_string(fixture_dir().join(file)).unwrap();
  assert_eq!(
    (token.get_src_line(), token.get_src_col()),
    position_of(&content, &format!("'{literal}'")),
    "{literal:?} maps to the wrong position in {file}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn collapsed_chains_give_the_same_maps_as_full_chains() {
  let mut collapsing = bundler(false);
  let first = Box::pin(generate(&mut collapsing)).await;
  // The first build dropped its chains. The second build scans the modules again.
  let second = Box::pin(generate(&mut collapsing)).await;
  let full = Box::pin(generate(&mut bundler(true))).await;

  assert_eq!(first, full);
  assert_eq!(second, full);
  let (code, map_json) = &full;
  assert_maps_to(code, map_json, "main-anchor", "main.js");
  assert_maps_to(code, map_json, "dep-anchor", "dep.js");
}
