use std::{borrow::Cow, fs, path::Path};

use rolldown_common::{AssetSource, EmittedAsset};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, Plugin, PluginContext,
};

const WASM_HELPER_ID: &str = "\0vite/wasm-helper.js";

#[derive(Debug)]
pub struct WasmHelperPlugin {}

impl Plugin for WasmHelperPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:wasm-helper")
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier == WASM_HELPER_ID {
      Ok(Some(HookResolveIdOutput { id: WASM_HELPER_ID.to_string(), ..Default::default() }))
    } else {
      Ok(None)
    }
  }

  async fn load(&self, ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if args.id == WASM_HELPER_ID {
      return Ok(Some(HookLoadOutput {
        code: include_str!("wasm_runtime.js").to_string(),
        ..Default::default()
      }));
    }

    if args.id.ends_with(".wasm?init") {
      let id = args.id.replace("?init", "");
      let file_path = Path::new(&id);
      let reference_id = ctx.emit_file(EmittedAsset {
        name: file_path.file_name().map(|x| x.to_string_lossy().to_string()),
        original_file_name: None,
        source: AssetSource::Buffer(fs::read(file_path)?),
        file_name: None,
      });
      let url = ctx.get_file_name(&reference_id);
      return Ok(Some(HookLoadOutput {
        code: format!(
          r#"import initWasm from "{WASM_HELPER_ID}"; 
          export default opts => initWasm(opts, "{url}")"#
        ),
        ..Default::default()
      }));
    }

    Ok(None)
  }
}
