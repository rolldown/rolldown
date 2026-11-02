use std::borrow::Cow;

use rolldown::{Bundler, BundlerOptions, InputItem};
use rolldown_common::{ExperimentalOptions, Output};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookUsage, Plugin, SharedLoadPluginContext,
};

/// Loads every `.wasm` file as JS without setting a module type, like `unwasm` does.
#[derive(Debug)]
struct WasmLoader;

impl Plugin for WasmLoader {
  fn name(&self) -> Cow<'static, str> {
    "wasm-loader".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if !args.id.ends_with(".wasm") {
      return Ok(None);
    }
    Ok(Some(HookLoadOutput {
      code: "export default 'loaded by plugin'".into(),
      ..Default::default()
    }))
  }
}

// https://github.com/rolldown/rolldown/issues/11325
#[tokio::test(flavor = "multi_thread")]
async fn new_url_asset_loaded_by_plugin() {
  let cwd =
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/plugin/new_url_asset_loaded_by_plugin");
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
      cwd: Some(cwd.into()),
      experimental: Some(ExperimentalOptions {
        resolve_new_url_to_asset: Some(true),
        ..Default::default()
      }),
      ..Default::default()
    },
    vec![Plugin::new_shared(WasmLoader)],
  )
  .unwrap();
  let output = bundler.generate().await.unwrap();

  let asset = output
    .assets
    .iter()
    .find_map(|output| match output {
      Output::Asset(asset) if asset.filename.ends_with(".wasm") => Some(asset),
      _ => None,
    })
    .expect("the wasm file was not emitted as an asset");
  assert_eq!(asset.source.as_bytes(), std::fs::read(format!("{cwd}/m.wasm")).unwrap());

  let chunk = output
    .assets
    .iter()
    .find_map(|output| match output {
      Output::Chunk(chunk) => Some(chunk),
      Output::Asset(_) => None,
    })
    .unwrap();
  assert!(
    chunk.code.contains(&format!("new URL(\"{}\", import.meta.url)", asset.filename)),
    "`new URL()` was not rewritten to the asset:\n{}",
    chunk.code
  );
}
