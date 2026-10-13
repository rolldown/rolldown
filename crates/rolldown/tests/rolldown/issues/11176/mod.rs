use std::{
  borrow::Cow,
  sync::{Arc, Mutex},
};

use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_common::Output;
use rolldown_plugin::{HookTransformOutputMap, HookUsage, Plugin};
use rolldown_sourcemap::{SourceMap, SourceMapBuilder, empty_sourcemap};
use string_wizard::{MagicString, SourceMapOptions};

/// A transform that returns `map: { mappings: '' }` declares that its output has no mapping back
/// to the module's previous code, so the module's original source must not end up in the emitted
/// sourcemap, even when an earlier transform produced a real map.
/// See: https://github.com/rolldown/rolldown/issues/11176
const SECRET: &str = "SERVER_ONLY_SECRET";
const SECRET_NAME: &str = "SERVER_ONLY_SECRET_NAME";

fn is_action(id: &str) -> bool {
  id.ends_with("action.js")
}

/// Stands in for any earlier transform with a real map (e.g. TypeScript stripping).
#[derive(Debug)]
struct Earlier;

impl Plugin for Earlier {
  fn name(&self) -> Cow<'static, str> {
    "earlier".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    _ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !is_action(args.id) {
      return Ok(None);
    }
    let mut magic_string = MagicString::new(args.code.as_str());
    magic_string.prepend("/* earlier */\n");
    let map = magic_string.source_map(SourceMapOptions {
      hires: string_wizard::Hires::True,
      include_content: true,
      source: args.id.into(),
    });
    // Name the first mapped segment like a TypeScript transform would name an identifier.
    let mut builder = SourceMapBuilder::default();
    let source = builder.add_source_and_content(args.id, args.code);
    let name = builder.add_name(SECRET_NAME);
    for (index, token) in map.get_tokens().enumerate() {
      builder.add_token(
        token.get_dst_line(),
        token.get_dst_col(),
        token.get_src_line(),
        token.get_src_col(),
        Some(source),
        (index == 0).then_some(name),
      );
    }
    let map = builder.into_sourcemap().into_owned();
    Ok(Some(rolldown_plugin::HookTransformOutput {
      code: Some(magic_string.to_string()),
      map: HookTransformOutputMap::Sourcemap(Box::new(map)),
      ..Default::default()
    }))
  }
}

/// Replaces the module with code that has no relation to the original and returns
/// `map: { mappings: '' }`. With `with_sources`, the map still lists the original module and its
/// code, as some tools emit for empty maps.
#[derive(Debug)]
struct Replace {
  with_sources: bool,
}

impl Plugin for Replace {
  fn name(&self) -> Cow<'static, str> {
    "replace".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    _ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !is_action(args.id) {
      return Ok(None);
    }
    let map = if self.with_sources {
      let mut builder = SourceMapBuilder::default();
      builder.add_source_and_content(args.id, args.code);
      builder.into_sourcemap().into_owned()
    } else {
      empty_sourcemap()
    };
    Ok(Some(rolldown_plugin::HookTransformOutput {
      code: Some("export const save = () => 'proxy';\n".to_string()),
      map: HookTransformOutputMap::Sourcemap(Box::new(map)),
      ..Default::default()
    }))
  }
}

/// Records `this.getCombinedSourcemap()` for the replaced module after all other transforms.
#[derive(Debug, Default)]
struct CombinedSourcemap(Arc<Mutex<Option<SourceMap>>>);

impl Plugin for CombinedSourcemap {
  fn name(&self) -> Cow<'static, str> {
    "combined-sourcemap".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if is_action(args.id) {
      *self.0.lock().unwrap() = Some(ctx.get_combined_sourcemap());
    }
    Ok(None)
  }
}

fn assert_no_leak(map: &SourceMap) {
  let sources = map.get_sources().collect::<Vec<_>>();
  assert!(!sources.iter().any(|source| is_action(source)), "leaked source: {sources:?}");
  assert!(
    !map.get_source_contents().flatten().any(|content| content.contains(SECRET)),
    "leaked sourcesContent: {:?}",
    map.get_source_contents().collect::<Vec<_>>()
  );
  let names = map.get_names().collect::<Vec<_>>();
  assert!(!names.contains(&SECRET_NAME), "leaked name: {names:?}");
}

/// Builds `entry.js` with the `Replace` plugin, preceded by `Earlier` when `with_earlier` is set.
async fn run(with_earlier: bool, replace: Replace) {
  let combined_sourcemap = Arc::default();
  let mut plugins = vec![];
  if with_earlier {
    plugins.push(Plugin::new_shared(Earlier));
  }
  plugins.push(Plugin::new_shared(replace));
  plugins.push(Plugin::new_shared(CombinedSourcemap(Arc::clone(&combined_sourcemap))));
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem { name: Some("entry".into()), import: "./entry.js".into() }]),
      cwd: Some(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/issues/11176").into()),
      sourcemap: Some(SourceMapType::File),
      ..Default::default()
    },
    plugins,
  )
  .expect("failed to create bundler");
  let output = bundler.generate().await.expect("build should succeed");

  let chunk = output
    .assets
    .iter()
    .find_map(|asset| match asset {
      Output::Chunk(chunk) => Some(chunk),
      Output::Asset(_) => None,
    })
    .expect("should emit a chunk");
  assert!(chunk.code.contains("proxy"));
  assert!(!chunk.code.contains(SECRET));
  let map = chunk.map.as_ref().expect("should emit a sourcemap");
  assert_no_leak(map);
  // The entry module keeps its mappings.
  assert!(map.get_sources().any(|source| source.ends_with("entry.js")));

  let combined_sourcemap = combined_sourcemap.lock().unwrap().take();
  assert_no_leak(&combined_sourcemap.expect("should run the transform hook for action.js"));
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_mappings_drops_original_source() {
  Box::pin(run(false, Replace { with_sources: false })).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_mappings_after_real_map_drops_original_source() {
  Box::pin(run(true, Replace { with_sources: false })).await;
}

/// The empty map is the only map in the chain, so `getCombinedSourcemap` has nothing to collapse.
#[tokio::test(flavor = "multi_thread")]
async fn empty_mappings_with_sources_drops_original_source() {
  Box::pin(run(false, Replace { with_sources: true })).await;
}
