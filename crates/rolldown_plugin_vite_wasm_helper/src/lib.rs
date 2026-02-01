use std::{
  borrow::Cow,
  fs,
  path::{Path, PathBuf},
};

use rolldown_common::{EmittedAsset, ModuleType, StrOrBytes};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookUsage, Plugin, PluginContext,
};
use rolldown_plugin_utils::{FileToUrlEnv, UsizeOrFunction};

const WASM_HELPER_ID: &str = "\0vite/wasm-helper.js";

#[derive(derive_more::Debug)]
pub struct ViteWasmHelperPluginV2Config {
  pub root: PathBuf,
  pub is_lib: bool,
  pub public_dir: String,
  #[debug(skip)]
  pub asset_inline_limit: UsizeOrFunction,
}

#[derive(Debug, Default)]
pub struct ViteWasmHelperPlugin {
  pub decoded_base: String,
  pub v2: Option<ViteWasmHelperPluginV2Config>,
}

impl Plugin for ViteWasmHelperPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-wasm-helper")
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

  async fn load(
    &self,
    ctx: rolldown_plugin::SharedLoadPluginContext,
    args: &HookLoadArgs<'_>,
  ) -> HookLoadReturn {
    if args.id == WASM_HELPER_ID {
      return Ok(Some(HookLoadOutput {
        code: arcstr::literal!(include_str!("wasm-runtime.js")),
        ..Default::default()
      }));
    }

    if args.id.ends_with(".wasm?init") {
      let url = if let Some(v2) = &self.v2 {
        let env = FileToUrlEnv {
          ctx: &ctx,
          root: &v2.root,
          is_lib: v2.is_lib,
          public_dir: &v2.public_dir,
          asset_inline_limit: &v2.asset_inline_limit,
        };
        env.file_to_url(args.id).await?
      } else {
        let file_path = Path::new(&args.id[..args.id.len() - 5]);
        let source = StrOrBytes::Bytes(fs::read(file_path)?);

        let referenced_id = ctx
          .emit_file_async(EmittedAsset {
            name: file_path.file_name().map(|x| x.to_string_lossy().to_string()),
            source,
            ..Default::default()
          })
          .await?;
        let filename = ctx.get_file_name(&referenced_id)?;
        rolldown_plugin_utils::join_url_segments(&self.decoded_base, &filename).into_owned()
      };

      return Ok(Some(HookLoadOutput {
        code: arcstr::format!(
          r#"import initWasm from "{WASM_HELPER_ID}"; 
          export default opts => initWasm(opts, "{url}")"#
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
