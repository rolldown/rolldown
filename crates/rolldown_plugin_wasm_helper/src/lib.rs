use std::{borrow::Cow, fs, path::Path};

use rolldown_common::{EmittedAsset, ModuleType, StrOrBytes};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookUsage, Plugin, PluginContext,
};

const WASM_HELPER_ID: &str = "\0vite/wasm-helper.js";

#[derive(Debug, Default)]
pub struct WasmHelperPlugin {
  pub decoded_base: String,
}

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
        code: arcstr::literal!(include_str!("wasm-runtime.js")),
        ..Default::default()
      }));
    }

    if args.id.ends_with(".wasm?init") {
      let file_path = Path::new(&args.id[..args.id.len() - 5]);
      let source = StrOrBytes::Bytes(fs::read(file_path)?, false);

      let referenced_id = ctx
        .emit_file_async(EmittedAsset {
          name: file_path.file_name().map(|x| x.to_string_lossy().to_string()),
          source,
          ..Default::default()
        })
        .await?;

      return Ok(Some(HookLoadOutput {
        code: arcstr::format!(
          r#"import initWasm from "{WASM_HELPER_ID}"; 
          export default opts => initWasm(opts, "{}")"#,
          rolldown_plugin_utils::join_url_segments(
            &self.decoded_base,
            &ctx.get_file_name(&referenced_id)?
          )
        ),
        module_type: Some(ModuleType::Js),
        ..Default::default()
      }));
    }

    Ok(None)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load
  }
}
