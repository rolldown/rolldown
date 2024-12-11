use std::{borrow::Cow, fs, path::Path};

use rolldown_common::{EmittedAsset, ModuleType, StrOrBytes};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, Plugin, PluginContext,
};
use rolldown_utils::concat_string;

const WASM_HELPER_ID: &str = "\0vite/wasm-helper.js";

#[derive(Debug)]
pub struct WasmHelperPlugin {}

impl Plugin for WasmHelperPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:wasm-helper")
  }

  async fn resolve_id(
    &self,
    ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier == WASM_HELPER_ID {
      Ok(Some(HookResolveIdOutput { id: WASM_HELPER_ID.to_string(), ..Default::default() }))
    } else if args.specifier.ends_with(".wasm?init") {
      let id = args.specifier.replace("?init", "");
      let resolved_id = ctx.resolve(&id, args.importer, None).await??;
      Ok(Some(HookResolveIdOutput {
        id: concat_string!(resolved_id.id, "?init"),
        ..Default::default()
      }))
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
        source: StrOrBytes::Bytes(fs::read(file_path)?),
        file_name: None,
      });
      let url = ctx.get_file_name(&reference_id);
      return Ok(Some(HookLoadOutput {
        code: format!(
          r#"import initWasm from "{WASM_HELPER_ID}"; 
          export default opts => initWasm(opts, "{url}")"#
        ),
        module_type: Some(ModuleType::Js),
        ..Default::default()
      }));
    }

    Ok(None)
  }
}
