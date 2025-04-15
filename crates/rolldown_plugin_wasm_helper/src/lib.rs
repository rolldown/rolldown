use std::{borrow::Cow, fs, path::Path};

use rolldown_common::{EmittedAsset, ModuleType, StrOrBytes};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, Plugin, PluginContext,
};

const WASM_HELPER_ID: &str = "\0vite/wasm-helper.js";

#[derive(Debug)]
pub struct WasmHelperPlugin;

impl Plugin for WasmHelperPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:wasm-helper")
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    Ok((args.specifier == WASM_HELPER_ID).then_some(HookResolveIdOutput {
      id: arcstr::literal!(WASM_HELPER_ID),
      ..Default::default()
    }))
  }

  async fn load(&self, ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if args.id == WASM_HELPER_ID {
      return Ok(Some(HookLoadOutput {
        code: include_str!("wasm-runtime.js").to_string(),
        ..Default::default()
      }));
    }

    if args.id.ends_with(".wasm?init") {
      let file_path = Path::new(&args.id[..args.id.len() - 5]);
      let source = StrOrBytes::Bytes(fs::read(file_path)?);
      let name = file_path.file_name().map(|x| x.to_string_lossy().to_string());

      let id = ctx.emit_file_async(EmittedAsset { name, source, ..Default::default() }).await?;
      return Ok(Some(HookLoadOutput {
        code: format!(
          r#"import initWasm from "{WASM_HELPER_ID}"; 
          export default opts => initWasm(opts, "{}")"#,
          ctx.get_file_name(&id)?
        ),
        module_type: Some(ModuleType::Js),
        ..Default::default()
      }));
    }

    Ok(None)
  }
}
