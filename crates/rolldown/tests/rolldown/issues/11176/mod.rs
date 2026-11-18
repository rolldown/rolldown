use std::{
  borrow::Cow,
  path::PathBuf,
  sync::{Arc, Mutex},
};

use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookTransformArgs, HookTransformReturn, HookUsage,
  Plugin, SharedLoadPluginContext, SharedTransformPluginContext,
};
use rolldown_sourcemap::{SourceMap, SourceMapBuilder};

/// The maps that `LoadMapPlugin` sees for `main.js`.
#[derive(Debug, Default)]
struct Maps {
  load: Option<SourceMap>,
  combined: Option<SourceMap>,
}

/// Loads `main.js` with a map that lists a source no token references, then records
/// `this.getCombinedSourcemap()` in the transform hook.
#[derive(Debug)]
struct LoadMapPlugin(Arc<Mutex<Maps>>);

impl Plugin for LoadMapPlugin {
  fn name(&self) -> Cow<'static, str> {
    "load-map".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load | HookUsage::Transform
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if !args.id.ends_with("main.js") {
      return Ok(None);
    }
    let code = std::fs::read_to_string(args.id)?;
    let mut builder = SourceMapBuilder::default();
    let source = builder.add_source_and_content(args.id, &code);
    builder.add_source_and_content("preprocessor-only.js", "unreferenced();\n");
    builder.add_token(0, 0, 0, 0, Some(source), None);
    let map = builder.into_sourcemap().into_owned();
    self.0.lock().unwrap().load = Some(map.clone());
    Ok(Some(HookLoadOutput { code: code.into(), map: Some(map), ..Default::default() }))
  }

  async fn transform(
    &self,
    ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    if args.id.ends_with("main.js") {
      self.0.lock().unwrap().combined = Some(ctx.get_combined_sourcemap());
    }
    Ok(None)
  }
}

// Like Rollup, `getCombinedSourcemap` returns a load map that no transform remapped as is, even
// when a source in it has no token.
#[tokio::test(flavor = "multi_thread")]
async fn combined_sourcemap_keeps_untransformed_load_map() {
  let maps = Arc::new(Mutex::new(Maps::default()));
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      cwd: Some(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/issues/11176"))),
      input: Some(vec![InputItem {
        name: Some("main".to_string()),
        import: "./main.js".to_string(),
      }]),
      sourcemap: Some(SourceMapType::File),
      ..Default::default()
    },
    vec![Plugin::new_shared(LoadMapPlugin(Arc::clone(&maps)))],
  )
  .expect("failed to create bundler");
  bundler.generate().await.expect("build should succeed");

  let maps = maps.lock().unwrap();
  let load = maps.load.as_ref().expect("should load main.js");
  let combined = maps.combined.as_ref().expect("should transform main.js");
  assert_eq!(combined.to_json_string(), load.to_json_string());
}
