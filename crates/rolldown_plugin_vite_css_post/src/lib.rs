mod utils;

use std::{borrow::Cow, path::PathBuf, sync::Arc};

use cow_utils::CowUtils;
use rolldown_common::{EmittedAsset, ModuleType, OutputFormat, side_effects::HookSideEffects};
use rolldown_plugin::{HookRenderChunkOutput, HookTransformOutput, HookUsage, Plugin};
use rolldown_plugin_utils::{
  RenderAssetUrlInJsEnv, RenderAssetUrlInJsEnvConfig, RenderBuiltUrl,
  constants::{
    CSSChunkCache, CSSEntriesCache, CSSModuleCache, CSSStyles, HTMLProxyResult, PureCSSChunks,
    ViteMetadata,
  },
  css::is_css_request,
  data_to_esm, find_special_query, get_chunk_original_name, is_special_query,
};
use rolldown_utils::{url::clean_url, xxhash::xxhash_with_base};
use string_wizard::SourceMapOptions;

#[allow(clippy::struct_excessive_bools)]
#[derive(derive_more::Debug)]
pub struct ViteCssPostPlugin {
  pub is_lib: bool,
  pub is_ssr: bool,
  pub is_worker: bool,
  pub is_legacy: bool,
  pub is_client: bool,
  pub css_minify: bool,
  pub css_code_split: bool,
  pub sourcemap: bool,
  pub url_base: String,
  pub decoded_base: String,
  pub lib_css_filename: Option<String>,
  #[debug(skip)]
  pub render_built_url: Option<Arc<RenderBuiltUrl>>,
}

impl Plugin for ViteCssPostPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Borrowed("builtin:vite-css-post")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform | HookUsage::RenderChunk
  }

  async fn build_start(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    ctx.meta().insert(Arc::new(CSSChunkCache::default()));
    ctx.meta().insert(Arc::new(PureCSSChunks::default()));
    Ok(())
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !is_css_request(args.id) || is_special_query(args.id) {
      return Ok(None);
    }

    // strip bom tag
    let mut css = Cow::Borrowed(args.code.trim_start_matches('\u{feff}'));
    let inline_css = find_special_query(args.id, b"inline-css").is_some();
    let is_html_proxy = find_special_query(args.id, b"html-proxy").is_some();

    if inline_css && is_html_proxy {
      if find_special_query(args.id, b"style-attr").is_some() {
        css = Cow::Owned(css.cow_replace('"', "&quot;").into_owned());
      }
      let Some(index) = utils::extract_index(args.id) else {
        return Err(anyhow::anyhow!("HTML proxy index in '{}' not found", args.id));
      };

      let hash = xxhash_with_base(clean_url(args.id).as_bytes(), 16);
      let cache = ctx.meta().get::<HTMLProxyResult>().expect("HTMLProxyResult missing");
      cache.inner.insert(rolldown_utils::concat_string!(hash, "_", index), css.into_owned());
      return Ok(Some(HookTransformOutput {
        code: Some("export default ''".to_owned()),
        ..Default::default()
      }));
    }

    let css_module_cache = ctx.meta().get::<CSSModuleCache>().expect("CSSModuleCache missing");

    let modules = css_module_cache.inner.get(args.id);
    let inlined = find_special_query(args.id, b"inline").is_some();

    let side_effects = if !inlined && modules.is_none() {
      HookSideEffects::NoTreeshake
    } else {
      HookSideEffects::False
    };

    let code = if inlined {
      if self.css_minify {
        todo!()
      }
      rolldown_utils::concat_string!("export default ", serde_json::to_string(&css)?)
    } else {
      let styles = ctx.meta().get::<CSSStyles>().expect("CSSStyles missing");
      styles.inner.insert(args.id.to_string(), css.into_owned());
      if let Some(modules) = modules {
        let data = serde_json::to_value(&*modules)?;
        data_to_esm(&data, true)
      } else {
        String::new()
      }
    };

    Ok(Some(HookTransformOutput {
      code: Some(code),
      side_effects: Some(side_effects),
      module_type: Some(ModuleType::Js),
      ..Default::default()
    }))
  }

  #[allow(unused_assignments, unused_variables, clippy::too_many_lines)]
  async fn render_chunk(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    // Empty if it's a dynamic chunk with only a CSS import
    // Example -> `import('./style.css')` with no other code
    let is_js_chunk_empty = args.code.is_empty() && !args.chunk.is_entry;
    let styles = ctx.meta().get::<CSSStyles>().expect("CSSStyles missing");
    let mut is_pure_css_chunk = args.chunk.exports.is_empty();
    let mut css_chunk = String::new();
    for module_id in &args.chunk.module_ids {
      let id = module_id.resource_id().as_str();
      if let Some(css) = styles.inner.get(id) {
        // `?transform-only` is used for ?url and shouldn't be included in normal CSS chunks
        if find_special_query(id, b"transform-only").is_some() {
          continue;
        }

        // TODO: implement cssScopeTo
        // https://github.com/vitejs/rolldown-vite/blob/c35ec68d/packages/vite/src/node/plugins/css.ts#L661-L667
        // const cssScopeTo = this.getModuleInfo(id)?.meta?.vite?.cssScopeTo
        // if (cssScopeTo && !isCssScopeToRendered(cssScopeTo, renderedModules)) {
        //   continue
        // }

        if rolldown_plugin_utils::css::is_css_module(id) {
          is_pure_css_chunk = false;
        }

        css_chunk.push_str(css.as_str());
      } else if !is_js_chunk_empty {
        // If the chunk has other JS code, it is not a pure CSS chunk
        is_pure_css_chunk = false;
      }
    }

    let mut magic_string = None;

    self.finalize_vite_css_urls(ctx, args, &styles, &mut magic_string).await?;

    if !css_chunk.is_empty() {
      if is_pure_css_chunk && args.options.format.is_esm_or_cjs() {
        ctx
          .meta()
          .get::<PureCSSChunks>()
          .expect("PureCSSChunks missing")
          .inner
          .insert(args.chunk.filename.clone());
      }

      if self.css_code_split {
        if args.options.format.is_esm_or_cjs() && !args.chunk.filename.contains("-legacy") {
          let css_asset_path = PathBuf::from(args.chunk.name.as_str()).with_extension("css");
          // if facadeModuleId doesn't exist or doesn't have a CSS extension,
          // that means a JS entry file imports a CSS file.
          // in this case, only use the filename for the CSS chunk name like JS chunks.
          let css_asset_name = if args.chunk.is_entry
            && args
              .chunk
              .facade_module_id
              .as_ref()
              .is_none_or(|id| !is_css_request(id.resource_id().as_str()))
          {
            css_asset_path.file_name().map(|v| v.to_string_lossy().into_owned())
          } else {
            Some(css_asset_path.to_string_lossy().into_owned())
          };

          let original_file_name = get_chunk_original_name(
            ctx.cwd(),
            self.is_legacy,
            &args.chunk.name,
            args.chunk.facade_module_id.as_ref(),
          );

          let content = self.resolve_asset_urls_in_css(&css_chunk).await;
          let content = self.finalize_css(content).await;

          let reference_id = ctx
            .emit_file_async(EmittedAsset {
              name: css_asset_name,
              source: content.into(),
              original_file_name,
              ..Default::default()
            })
            .await?;

          ctx
            .meta()
            .get::<ViteMetadata>()
            .expect("ViteMetadata missing")
            .imported_assets
            .insert(ctx.get_file_name(&reference_id)?);

          if args.chunk.is_entry && is_pure_css_chunk {
            ctx
              .meta()
              .get::<CSSEntriesCache>()
              .expect("CSSEntriesCache missing")
              .inner
              .insert(reference_id);
          }
        } else if self.is_client {
          let injection_point = match args.options.format {
            OutputFormat::Esm => {
              if args.code.starts_with("#!") {
                args.code.find('\n').unwrap_or(0)
              } else {
                0
              }
            }
            OutputFormat::Iife | OutputFormat::Umd => {
              let regex = if matches!(args.options.format, OutputFormat::Iife) {
                &utils::RE_IIFE
              } else {
                &utils::RE_UMD
              };
              let Some(m) = regex.find(&args.code) else {
                return Err(anyhow::anyhow!("Injection point for inlined CSS not found"));
              };
              m.end()
            }
            OutputFormat::Cjs => {
              return Err(anyhow::anyhow!("CJS format is not supported for CSS injection"));
            }
          };

          let content = serde_json::to_string(&self.finalize_css(css_chunk).await)?;
          let env = RenderAssetUrlInJsEnv {
            ctx,
            code: &content,
            chunk_filename: &args.chunk.filename,
            config: RenderAssetUrlInJsEnvConfig {
              is_ssr: self.is_ssr,
              is_worker: self.is_worker,
              url_base: &self.url_base,
              decoded_base: &self.decoded_base,
              render_built_url: self.render_built_url.as_deref(),
            },
          };

          let css_string = env.render_asset_url_in_js().await?.unwrap_or(content);
          let inject_code = rolldown_utils::concat_string!(
            "var __vite_style__ = document.createElement('style');__vite_style__.textContent = ",
            css_string,
            ";document.head.appendChild(__vite_style__);"
          );

          magic_string
            .get_or_insert_with(|| string_wizard::MagicString::new(&args.code))
            .append_right(injection_point, inject_code);
        }
      } else {
        let css_asset_name = self.get_css_bundle_name(ctx)?;
        let css_chunk = self.resolve_asset_urls_in_css(&css_chunk).await;
        ctx
          .meta()
          .get::<CSSChunkCache>()
          .expect("CSSChunkCache missing")
          .inner
          .insert(args.chunk.filename.clone(), css_chunk);
      }
    }

    Ok(magic_string.map(|magic_string| HookRenderChunkOutput {
      code: magic_string.to_string(),
      map: self.sourcemap.then(|| {
        magic_string.source_map(SourceMapOptions {
          hires: string_wizard::Hires::Boundary,
          ..Default::default()
        })
      }),
    }))
  }
}
