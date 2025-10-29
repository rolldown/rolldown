mod utils;

use std::{
  borrow::Cow,
  pin::Pin,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use cow_utils::CowUtils;
use rolldown_common::{ModuleType, Output, StrOrBytes, side_effects::HookSideEffects};
use rolldown_plugin::{HookRenderChunkOutput, HookTransformOutput, HookUsage, Plugin};
use rolldown_plugin_utils::{
  RenderBuiltUrl, ToOutputFilePathEnv,
  constants::{
    CSSChunkCache, CSSModuleCache, CSSStyles, HTMLProxyResult, PureCSSChunks,
    RemovedPureCSSFilesCache, ViteMetadata,
  },
  css::is_css_request,
  data_to_esm, find_special_query, is_special_query,
};
use rolldown_utils::{url::clean_url, xxhash::xxhash_with_base};
use string_wizard::SourceMapOptions;

pub type IsLegacyFn =
  dyn Fn() -> Pin<Box<dyn Future<Output = anyhow::Result<bool>> + Send>> + Send + Sync;

pub type CSSMinifyFn =
  dyn Fn(String) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send>> + Send + Sync;

#[expect(clippy::struct_excessive_bools)]
#[derive(derive_more::Debug, Default)]
pub struct ViteCSSPostPlugin {
  pub is_lib: bool,
  pub is_ssr: bool,
  pub is_worker: bool,
  pub is_client: bool,
  pub css_code_split: bool,
  pub sourcemap: bool,
  pub assets_dir: String,
  pub url_base: String,
  pub decoded_base: String,
  pub lib_css_filename: Option<String>,
  #[debug(skip)]
  pub is_legacy: Option<Arc<IsLegacyFn>>,
  #[debug(skip)]
  pub css_minify: Option<Arc<CSSMinifyFn>>,
  #[debug(skip)]
  pub render_built_url: Option<Arc<RenderBuiltUrl>>,
  // internal state
  pub has_emitted: AtomicBool,
}

impl Plugin for ViteCSSPostPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Borrowed("builtin:vite-css-post")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
      | HookUsage::Transform
      | HookUsage::RenderStart
      | HookUsage::RenderChunk
      | HookUsage::AugmentChunkHash
      | HookUsage::GenerateBundle
  }

  async fn build_start(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    ctx.meta().insert(Arc::new(CSSStyles::default()));
    ctx.meta().insert(Arc::new(RemovedPureCSSFilesCache::default()));
    Ok(())
  }

  async fn render_start(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    _args: &rolldown_plugin::HookRenderStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    self.has_emitted.store(false, Ordering::Relaxed);
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
      if let Some(ref css_minify) = self.css_minify {
        css = Cow::Owned(css_minify(css.into_owned()).await?);
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

    let ctx = utils::FinalizedContext {
      plugin_ctx: ctx,
      args,
      env: &ToOutputFilePathEnv {
        is_ssr: self.is_ssr,
        host_id: &args.chunk.filename,
        url_base: &self.url_base,
        decoded_base: &self.decoded_base,
        render_built_url: self.render_built_url.as_deref(),
      },
    };

    let mut magic_string = None;
    self.finalize_vite_css_urls(&ctx, &styles, &mut magic_string).await?;
    self.finalize_css_chunk(&ctx, css_chunk, is_pure_css_chunk, &mut magic_string).await?;

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

  async fn augment_chunk_hash(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    chunk: Arc<rolldown_common::RollupRenderedChunk>,
  ) -> rolldown_plugin::HookAugmentChunkHashReturn {
    Ok(ctx.meta().get::<ViteMetadata>().and_then(|vite_metadata| {
      vite_metadata.get(&chunk.filename).and_then(|metadata| {
        (!metadata.imported_css.is_empty()).then(|| {
          let capacity = metadata.imported_css.iter().fold(0, |acc, s| acc + s.len());
          let mut hash = String::with_capacity(capacity);
          for id in metadata.imported_css.iter() {
            hash.push_str(&id);
          }
          hash
        })
      })
    }))
  }

  async fn generate_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    // to avoid emitting duplicate assets for modern build and legacy build
    if let Some(is_legacy_fn) = &self.is_legacy
      && is_legacy_fn().await?
    {
      return Ok(());
    }

    // extract as single css bundle if no codesplit
    self.emit_non_codesplit_css_bundle(ctx, args.bundle).await?;
    // remove empty css chunks and their imports
    self.prune_pure_css_chunks(ctx, args);

    let mut bundle_iter = args.bundle.iter_mut();
    while let Some(Output::Asset(asset)) = bundle_iter.next()
      && asset.filename.ends_with(".css")
    {
      if let StrOrBytes::Str(ref s) = asset.source {
        let Cow::Owned(source) = s.cow_replace(utils::VITE_HASH_UPDATE_MARKER, "") else {
          continue;
        };
        *asset = Arc::new(rolldown_common::OutputAsset {
          names: asset.names.clone(),
          source: StrOrBytes::Str(source),
          filename: asset.filename.clone(),
          original_file_names: asset.original_file_names.clone(),
        });
      }
    }
    Ok(())
  }
}
