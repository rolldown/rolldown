use std::{
  borrow::Cow,
  sync::{Arc, Mutex},
};

use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_common::Output;
use rolldown_plugin::{HookTransformOutputMap, HookUsage, Plugin};
use rolldown_sourcemap::{SourceMap, empty_sourcemap};
use string_wizard::{MagicString, SourceMapOptions};

/// A transform that returns `map: { mappings: '' }` declares that its output has no mapping back
/// to the module's previous code, so the module's original source must not end up in the emitted
/// sourcemap, even when an earlier transform produced a real map.
/// See: https://github.com/rolldown/rolldown/issues/11176
const SECRET: &str = "SERVER_ONLY_SECRET";

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
    Ok(Some(rolldown_plugin::HookTransformOutput {
      code: Some(magic_string.to_string()),
      map: HookTransformOutputMap::Sourcemap(Box::new(map)),
      ..Default::default()
    }))
  }
}

/// Replaces the module with code that has no relation to the original and returns
/// `map: { mappings: '' }`.
#[derive(Debug)]
struct Replace;

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
    Ok(Some(rolldown_plugin::HookTransformOutput {
      code: Some("export const save = () => 'proxy';\n".to_string()),
      map: HookTransformOutputMap::Sourcemap(Box::new(empty_sourcemap())),
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
}

/// Builds `entry.js` with the `Replace` plugin, preceded by `Earlier` when `with_earlier` is set.
async fn run(with_earlier: bool) {
  let combined_sourcemap = Arc::default();
  let mut plugins = vec![];
  if with_earlier {
    plugins.push(Plugin::new_shared(Earlier));
  }
  plugins.push(Plugin::new_shared(Replace));
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
  Box::pin(run(false)).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_mappings_after_real_map_drops_original_source() {
  Box::pin(run(true)).await;
}
