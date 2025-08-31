mod utils;

use std::{
  borrow::Cow,
  path::Path,
  pin::Pin,
  sync::{
    Arc, LazyLock,
    atomic::{AtomicBool, Ordering},
  },
};

use cow_utils::CowUtils;
use regex::{Regex, escape};
use rolldown_common::{ModuleType, Output, StrOrBytes, side_effects::HookSideEffects};
use rolldown_plugin::{HookRenderChunkOutput, HookTransformOutput, HookUsage, Plugin};
use rolldown_plugin_utils::{
  RenderBuiltUrl, ToOutputFilePathEnv,
  constants::{
    CSSChunkCache, CSSModuleCache, CSSStyles, HTMLProxyResult, PureCSSChunks, ViteMetadata,
  },
  css::is_css_request,
  data_to_esm, find_special_query, is_special_query,
};
use rolldown_utils::{url::clean_url, xxhash::xxhash_with_base};
use string_wizard::SourceMapOptions;

pub type CSSMinifyFn =
  dyn Fn(String) -> Pin<Box<(dyn Future<Output = anyhow::Result<String>> + Send)>> + Send + Sync;

#[allow(clippy::struct_excessive_bools)]
#[derive(derive_more::Debug)]
pub struct ViteCssPostPlugin {
  pub is_lib: bool,
  pub is_ssr: bool,
  pub is_worker: bool,
  pub is_legacy: bool,
  pub is_client: bool,
  pub css_code_split: bool,
  pub sourcemap: bool,
  pub assets_dir: String,
  pub url_base: String,
  pub decoded_base: String,
  pub lib_css_filename: Option<String>,
  #[debug(skip)]
  pub css_minify: Option<Arc<CSSMinifyFn>>,
  #[debug(skip)]
  pub render_built_url: Option<Arc<RenderBuiltUrl>>,
  // internal state
  pub has_emitted: AtomicBool,
}

impl Plugin for ViteCssPostPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Borrowed("builtin:vite-css-post")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform | HookUsage::RenderChunk
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
    _chunk: Arc<rolldown_common::RollupRenderedChunk>,
  ) -> rolldown_plugin::HookAugmentChunkHashReturn {
    Ok(ctx.meta().get::<ViteMetadata>().and_then(|vite_metadata| {
      (!vite_metadata.imported_assets.is_empty()).then(|| {
        let capacity = vite_metadata.imported_assets.iter().fold(0, |acc, s| acc + s.len());
        let mut hash = String::with_capacity(capacity);
        for asset in vite_metadata.imported_assets.iter() {
          hash.push_str(&asset);
        }
        hash
      })
    }))
  }

  #[expect(clippy::too_many_lines)]
  async fn generate_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    // to avoid emitting duplicate assets for modern build and legacy build
    if self.is_legacy {
      return Ok(());
    }
    // extract as single css bundle if no codesplit
    if !self.css_code_split && !self.has_emitted.load(Ordering::Relaxed) {
      todo!();
    }
    // remove empty css chunks and their imports
    if let Some(pure_css_chunks) = ctx.meta().get::<PureCSSChunks>()
      && !pure_css_chunks.inner.is_empty()
    {
      let mut pure_css_chunk_names = Vec::with_capacity(pure_css_chunks.inner.len());

      let mut bundle_iter = args.bundle.iter();
      while let Some(Output::Chunk(chunk)) = bundle_iter.next() {
        if pure_css_chunks.inner.contains(chunk.preliminary_filename.as_str()) {
          pure_css_chunk_names.push(chunk.filename.clone());
        }
      }

      // TODO: improve below regex logic
      let empty_chunk_re = LazyLock::new(|| {
        let empty_chunk_files = pure_css_chunk_names
          .iter()
          .filter_map(|file| {
            Path::new(file.as_str()).file_name().and_then(|v| v.to_str().map(escape))
          })
          .collect::<Vec<_>>()
          .join("|");

        Regex::new(&if args.options.format.is_esm() {
          rolldown_utils::concat_string!(
            r#"\\bimport\\s*["'][^"']*(?:"#,
            empty_chunk_files,
            r#")["'];"#
          )
        } else {
          rolldown_utils::concat_string!(
            r#"(\\b|,\\s*)require\\(\\s*["'\`][^"'\`]*(?:"#,
            empty_chunk_files,
            r#")["'\`]\\)(;|,)"#
          )
        })
        .unwrap()
      });

      let bundle_iter = args.bundle.iter_mut();
      for output in bundle_iter {
        if let Output::Chunk(chunk) = output {
          let mut chunk_imports_pure_css_chunk = false;
          let mut new_chunk = (**chunk).clone();
          // remove pure css chunk from other chunk's imports, and also
          // register the emitted CSS files under the importer chunks instead.
          new_chunk.imports = new_chunk
            .imports
            .into_iter()
            .filter(|file| {
              if pure_css_chunk_names.contains(file) {
                // TODO: get special file chunk metadata
                let cache = ctx.meta().get::<ViteMetadata>().expect("ViteMetadata is missing");
                cache.imported_assets.iter().for_each(|file| {
                  ctx
                    .meta()
                    .get::<ViteMetadata>()
                    .expect("ViteMetadata is missing")
                    .imported_assets
                    .insert(file.clone());
                });
                chunk_imports_pure_css_chunk = true;
                return false;
              }
              true
            })
            .collect::<Vec<_>>();
          if chunk_imports_pure_css_chunk {
            new_chunk.code = empty_chunk_re
              .replace_all(&chunk.code, |captures: &regex::Captures<'_>| {
                if args.options.format.is_esm() {
                  return format!("/* empty css {:<width$}*/", "", width = captures.len() - 15);
                }
                if let Some(p2) = captures.get(2)
                  && p2.as_str() == ";"
                {
                  return format!(";/* empty css {:<width$}*/", "", width = captures.len() - 16);
                }
                let p1 = captures.get(1).map_or("", |m| m.as_str());
                format!("{p1}/* empty css {:<width$}*/", "", width = captures.len() - 15 - p1.len())
              })
              .into_owned();
          }
          *chunk = Arc::new(new_chunk);
        }
      }

      // TODO: Verify this change (not set `removedPureCssFilesCache`)
      args.bundle.retain(|output| !match output {
        Output::Chunk(chunk) => pure_css_chunk_names.contains(&chunk.filename),
        Output::Asset(asset) => pure_css_chunk_names
          .contains(&rolldown_utils::concat_string!(asset.filename, ".map").into()),
      });
    }
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
