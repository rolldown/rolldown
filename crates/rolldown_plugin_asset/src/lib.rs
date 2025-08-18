mod utils;

use std::{borrow::Cow, sync::Arc};

use derive_more::Debug;
use rolldown_common::{ModuleType, Output, side_effects::HookSideEffects};
use rolldown_plugin::{HookUsage, Plugin};
use rolldown_plugin_utils::{
  AssetCache, FileToUrlEnv, PublicAssetUrlCache, RenderAssetUrlInJsEnv,
  RenderAssetUrlInJsEnvConfig, RenderBuiltUrl, UsizeOrFunction, check_public_file,
  find_special_query,
};
use rolldown_utils::{dashmap::FxDashSet, pattern_filter::StringOrRegex, url::clean_url};
use serde_json::Value;

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default)]
pub struct AssetPlugin {
  pub is_lib: bool,
  pub is_ssr: bool,
  pub is_worker: bool,
  pub is_skip_assets: bool,
  pub url_base: String,
  pub public_dir: String,
  pub decoded_base: String,
  pub assets_include: Vec<StringOrRegex>,
  #[debug(skip)]
  pub asset_inline_limit: UsizeOrFunction,
  #[debug(skip)]
  pub render_built_url: Option<Arc<RenderBuiltUrl>>,
  pub handled_asset_ids: FxDashSet<String>,
}

impl Plugin for AssetPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:asset")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
      | HookUsage::ResolveId
      | HookUsage::Load
      | HookUsage::RenderChunk
      | HookUsage::GenerateBundle
  }

  async fn build_start(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    ctx.meta().insert(Arc::new(AssetCache::default()));
    ctx.meta().insert(Arc::new(PublicAssetUrlCache::default()));
    Ok(())
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if self.check_invalid_assets(ctx.cwd(), args.specifier).is() {
      return Ok(None);
    }
    Ok(check_public_file(clean_url(args.specifier), &self.public_dir).map(|_| {
      rolldown_plugin::HookResolveIdOutput { id: args.specifier.into(), ..Default::default() }
    }))
  }

  async fn load(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if args.id.starts_with('\0') {
      return Ok(None);
    }

    if find_special_query(args.id, b"raw").is_some() {
      let path = match check_public_file(args.id, &self.public_dir) {
        Some(f) => Cow::Owned(f.to_string_lossy().into_owned()),
        None => Cow::Borrowed(clean_url(args.id)),
      };

      ctx.add_watch_file(&path);

      let content = std::fs::read_to_string(path.as_ref())?;
      let code = arcstr::format!("export default {}", serde_json::to_string(&content)?);
      return Ok(Some(rolldown_plugin::HookLoadOutput {
        code,
        module_type: Some(ModuleType::Js),
        ..Default::default()
      }));
    }

    match self.check_invalid_assets(ctx.cwd(), args.id) {
      utils::InvalidAsset::True => return Ok(None),
      utils::InvalidAsset::False => {}
      utils::InvalidAsset::Special => {
        self.handled_asset_ids.insert(args.id.to_string());
      }
    }

    let id = rolldown_plugin_utils::remove_special_query(args.id, b"url");
    let env = FileToUrlEnv {
      ctx,
      root: ctx.cwd(),
      is_lib: self.is_lib,
      public_dir: &self.public_dir,
      asset_inline_limit: &self.asset_inline_limit,
    };

    let side_effects = if ctx.get_module_info(&id).is_some_and(|v| v.is_entry) {
      HookSideEffects::NoTreeshake
    } else {
      HookSideEffects::False
    };

    let url = rolldown_plugin_utils::uri::encode_uri_path(env.file_to_url(&id).await?);
    let code = arcstr::format!("export default {}", serde_json::to_string(&Value::String(url))?);
    Ok(Some(rolldown_plugin::HookLoadOutput {
      code,
      side_effects: Some(side_effects),
      module_type: Some(ModuleType::Js),
      ..Default::default()
    }))
  }

  async fn render_chunk(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    let env = RenderAssetUrlInJsEnv {
      ctx,
      code: &args.code,
      chunk_filename: &args.chunk.filename,
      config: RenderAssetUrlInJsEnvConfig {
        is_ssr: self.is_ssr,
        is_worker: self.is_worker,
        url_base: &self.url_base,
        decoded_base: &self.decoded_base,
        render_built_url: self.render_built_url.as_deref(),
      },
    };

    // TODO: consider using `MagicString` later
    Ok(
      env
        .render_asset_url_in_js()
        .await?
        .map(|code| rolldown_plugin::HookRenderChunkOutput { code, map: None }),
    )
  }

  async fn generate_bundle(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    let mut deleted_files = vec![0u8; args.bundle.len().div_ceil(8)];
    for (index, file) in args.bundle.iter().enumerate() {
      match file {
        Output::Chunk(chunk) => {
          if chunk.is_entry
            && chunk.module_ids.len() == 1
            && self.handled_asset_ids.contains(&*chunk.module_ids[0])
          {
            deleted_files[index / 8] |= 1 << (index & 7);
          }
        }
        Output::Asset(asset) => {
          if self.is_skip_assets
            && !asset.filename.ends_with("ssr-manifest.json")
            && !asset.filename.ends_with(".js.map")
            && !asset.filename.ends_with(".cjs.map")
            && !asset.filename.ends_with(".mjs.map")
          {
            deleted_files[index / 8] |= 1 << (index & 7);
          }
        }
      }
    }
    for (i, e) in deleted_files.into_iter().rev().enumerate() {
      'outer: for j in (0..8).rev() {
        if e & (1 << j) != 0 {
          let index = i * 8 + j;
          if let Output::Chunk(item) = &args.bundle[index] {
            for file in args.bundle.iter() {
              if let Output::Chunk(chunk) = file {
                if chunk.imports.contains(&item.filename)
                  || chunk.dynamic_imports.contains(&item.filename)
                {
                  continue 'outer;
                }
              }
            }
          }
          args.bundle.remove(index);
        }
      }
    }
    Ok(())
  }
}
