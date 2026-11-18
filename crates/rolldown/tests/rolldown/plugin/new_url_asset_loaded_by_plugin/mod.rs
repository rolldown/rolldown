use std::borrow::Cow;

use rolldown::{BundleOutput, Bundler, BundlerOptions, InputItem};
use rolldown_common::{ExperimentalOptions, ModuleType, Output};
use rolldown_plugin::{
  __inner::SharedPluginable, HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs,
  HookResolveIdOutput, HookResolveIdReturn, HookUsage, Plugin, PluginContext,
  SharedLoadPluginContext,
};

const CWD: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/plugin/new_url_asset_loaded_by_plugin");

/// Loads every `.wasm` file as JS, like `unwasm` does.
#[derive(Debug)]
struct WasmLoader(Option<ModuleType>);

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
      module_type: self.0.clone(),
      ..Default::default()
    }))
  }
}

/// Resolves `./m.wasm` to an id with no file on disk.
#[derive(Debug)]
struct ResolveWasmTo(String);

impl Plugin for ResolveWasmTo {
  fn name(&self) -> Cow<'static, str> {
    "resolve-wasm-to".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    Ok(
      (args.specifier == "./m.wasm")
        .then(|| HookResolveIdOutput { id: self.0.as_str().into(), ..Default::default() }),
    )
  }
}

async fn generate(plugins: Vec<SharedPluginable>) -> BundleOutput {
  Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
      cwd: Some(CWD.into()),
      experimental: Some(ExperimentalOptions {
        resolve_new_url_to_asset: Some(true),
        ..Default::default()
      }),
      ..Default::default()
    },
    plugins,
  )
  .unwrap()
  .generate()
  .await
  .unwrap()
}

fn chunk_code(output: &BundleOutput) -> &str {
  output
    .assets
    .iter()
    .find_map(|output| match output {
      Output::Chunk(chunk) => Some(chunk.code.as_str()),
      Output::Asset(_) => None,
    })
    .unwrap()
}

async fn assert_asset_emitted(module_type: Option<ModuleType>) {
  let output = generate(vec![Plugin::new_shared(WasmLoader(module_type))]).await;
  let asset = output
    .assets
    .iter()
    .find_map(|output| match output {
      Output::Asset(asset) if asset.filename.ends_with(".wasm") => Some(asset),
      _ => None,
    })
    .expect("the wasm file was not emitted as an asset");
  assert_eq!(asset.source.as_bytes(), std::fs::read(format!("{CWD}/m.wasm")).unwrap());

  let code = chunk_code(&output);
  assert!(
    code.contains(&format!("new URL(\"{}\", import.meta.url)", asset.filename)),
    "`new URL()` was not rewritten to the asset:\n{code}"
  );
}

// https://github.com/rolldown/rolldown/issues/11325
#[tokio::test(flavor = "multi_thread")]
async fn new_url_asset_loaded_by_plugin() {
  assert_asset_emitted(None).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn new_url_asset_loaded_by_plugin_with_module_type() {
  assert_asset_emitted(Some(ModuleType::Js)).await;
}

// A target with no file on disk has nothing to emit, so its URL is left as written.
#[tokio::test(flavor = "multi_thread")]
async fn new_url_target_without_file_loaded_by_plugin() {
  for id in ["virtual:m.wasm".to_string(), format!("{CWD}/missing.wasm")] {
    let output = generate(vec![
      Plugin::new_shared(ResolveWasmTo(id)),
      Plugin::new_shared(WasmLoader(Some(ModuleType::Js))),
    ])
    .await;
    let code = chunk_code(&output);
    assert!(code.contains("new URL(\"./m.wasm\", import.meta.url)"), "{code}");
  }
}
