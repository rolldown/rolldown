use std::{
  borrow::Cow,
  path::{Path, PathBuf},
  sync::{Arc, LazyLock},
};

use regex::Regex;
use rolldown_common::{
  AssetFilenamesOutputOption, EmittedAsset, OutputFormat, RollupPreRenderedAsset,
};
use rolldown_plugin::{HookRenderChunkArgs, PluginContext};
use rolldown_plugin_utils::{
  AssetUrlItem, AssetUrlIter, AssetUrlResult, PublicAssetUrlCache, RenderAssetUrlInJsEnv,
  RenderAssetUrlInJsEnvConfig, RenderBuiltUrlConfig, ToOutputFilePathEnv,
  constants::{
    CSSBundleName, CSSChunkCache, CSSEntriesCache, CSSStyles, PureCSSChunks, ViteMetadata,
  },
  create_to_import_meta_url_based_relative_runtime,
  css::is_css_request,
  get_chunk_original_name,
  uri::encode_uri_path,
};
use rolldown_utils::{futures::block_on_spawn_all, url::clean_url};
use string_wizard::MagicString;
use sugar_path::SugarPath;

use crate::ViteCssPostPlugin;

pub const DEFAULT_CSS_BUNDLE_NAME: &str = "style.css";

// TODO: improve below logic
pub static RE_UMD: LazyLock<Regex> = std::sync::LazyLock::new(|| {
  Regex::new(r#"\}\)\((?:this,\s*)?function\([^()]*\)\s*\{(?:\s*"use strict";)?"#).unwrap()
});

pub static RE_IIFE: LazyLock<Regex> = std::sync::LazyLock::new(|| {
  Regex::new(
    r#"(?:(?:const|var)\s+\S+\s*=\s*|^|\n)\(?function\([^()]*\)\s*\{(?:\s*"use strict";)?"#,
  )
  .unwrap()
});

pub fn extract_index(id: &str) -> Option<&str> {
  let s = id.split_once("&index=")?.1;
  let end = s.as_bytes().iter().take_while(|b| b.is_ascii_digit()).count();
  (end > 0).then_some(&s[..end])
}

#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct UrlEmitTasks {
  pub range: (usize, usize),
  pub replacement: String,
}

impl ViteCssPostPlugin {
  pub async fn finalize_vite_css_urls<'a>(
    &self,
    ctx: &PluginContext,
    args: &'a HookRenderChunkArgs<'_>,
    css_styles: &CSSStyles,
    magic_string: &mut Option<MagicString<'a>>,
  ) -> anyhow::Result<()> {
    let mut css_url_iter = args.code.match_indices("__VITE_CSS_URL__").peekable();
    if css_url_iter.peek().is_some() {
      let vite_metadata = ctx.meta().get::<ViteMetadata>().expect("ViteMetadata missing");
      let env = ToOutputFilePathEnv {
        url_base: &self.url_base,
        decoded_base: &self.decoded_base,
        render_built_url: self.render_built_url.as_deref(),
        render_built_url_config: RenderBuiltUrlConfig {
          r#type: "asset",
          is_ssr: self.is_ssr,
          host_id: &args.chunk.filename,
          host_type: "js",
        },
      };
      let tasks = css_url_iter.map(async |(index, _)| {
        let start = index + "__VITE_CSS_URL__".len();
        let Some(pos) = args.code[start..].find("__") else {
          return Err(anyhow::anyhow!(
            "Invalid __VITE_CSS_URL__ in '{}', expected '__VITE_CSS_URL__<base64>__'",
            args.chunk.name
          ));
        };

        let id = unsafe {
          String::from_utf8_unchecked(
            base64_simd::STANDARD
              .decode_to_vec(&args.code[start..start + pos])
              .map_err(|_| anyhow::anyhow!("Invalid base64 in '__VITE_CSS_URL__'"))?,
          )
        };

        let Some(style) = css_styles.inner.get(&id) else {
          return Err(anyhow::anyhow!("CSS content for  '{}' was not found", id));
        };

        let original_file_name = clean_url(&id).to_string();
        let css_asset_path = PathBuf::from(&original_file_name).with_extension("css");
        let css_asset_name =
          css_asset_path.file_name().map(|v| v.to_string_lossy().into_owned()).unwrap();

        let content = self
          .resolve_asset_urls_in_css(
            ctx,
            args,
            style.to_owned(),
            &css_asset_name,
            &args.options.asset_filenames,
          )
          .await?;
        let content = self.finalize_css(content).await;

        let reference_id = ctx
          .emit_file_async(EmittedAsset {
            name: Some(css_asset_name),
            source: content.into(),
            original_file_name: Some(original_file_name),
            ..Default::default()
          })
          .await?;

        let filename = ctx.get_file_name(&reference_id)?;
        vite_metadata.imported_assets.insert(clean_url(&reference_id).into());

        let url = env
          .to_output_file_path(
            &filename,
            create_to_import_meta_url_based_relative_runtime(ctx.options().format, self.is_worker),
          )
          .await?;

        Ok(UrlEmitTasks { range: (index, index + pos + 2), replacement: url.to_asset_url_in_js()? })
      });

      let magic_string =
        magic_string.get_or_insert_with(|| string_wizard::MagicString::new(&args.code));
      for task in block_on_spawn_all(tasks).await {
        let UrlEmitTasks { range: (start, end), replacement } = task?;
        magic_string.update(start, end, replacement);
      }
    }
    Ok(())
  }

  #[allow(clippy::too_many_lines)]
  pub async fn finalize_css_chunk<'a>(
    &self,
    ctx: &PluginContext,
    args: &'a HookRenderChunkArgs<'_>,
    css_chunk: String,
    is_pure_css_chunk: bool,
    magic_string: &mut Option<MagicString<'a>>,
  ) -> anyhow::Result<()> {
    if css_chunk.is_empty() {
      return Ok(());
    }

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
          css_asset_path.file_name().map(|v| v.to_string_lossy().into_owned()).unwrap()
        } else {
          css_asset_path.to_string_lossy().into_owned()
        };

        let original_file_name = get_chunk_original_name(
          ctx.cwd(),
          self.is_legacy,
          &args.chunk.name,
          args.chunk.facade_module_id.as_ref(),
        );

        let content = self
          .resolve_asset_urls_in_css(
            ctx,
            args,
            css_chunk,
            &css_asset_name,
            &args.options.asset_filenames,
          )
          .await?;
        let content = self.finalize_css(content).await;

        let reference_id = ctx
          .emit_file_async(EmittedAsset {
            name: Some(css_asset_name),
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
            let regex =
              if matches!(args.options.format, OutputFormat::Iife) { &RE_IIFE } else { &RE_UMD };
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
      ctx.meta().get::<CSSChunkCache>().expect("CSSChunkCache missing").inner.insert(
        args.chunk.filename.clone(),
        self
          .resolve_asset_urls_in_css(
            ctx,
            args,
            css_chunk,
            &self.get_css_bundle_name(ctx)?,
            &args.options.asset_filenames,
          )
          .await?,
      );
    }

    Ok(())
  }

  #[allow(clippy::unused_async, unused_variables)]
  pub async fn resolve_asset_urls_in_css(
    &self,
    ctx: &PluginContext,
    args: &HookRenderChunkArgs<'_>,
    css_chunk: String,
    css_asset_name: &str,
    css_file_names: &AssetFilenamesOutputOption,
  ) -> anyhow::Result<String> {
    let css_asset_dirname = if self.url_base.is_empty() || self.url_base == "./" {
      Some(self.get_css_asset_dir_name(css_asset_name, css_file_names).await?)
    } else {
      None
    };

    let to_relative = |filename: &Path, host_id: &Path| {
      let relative_path = filename.relative(css_asset_dirname.as_ref().unwrap());
      let relative_path = relative_path.to_slash_lossy();
      if relative_path.starts_with('.') {
        AssetUrlResult::WithoutRuntime(relative_path.into_owned())
      } else {
        AssetUrlResult::WithoutRuntime(rolldown_utils::concat_string!("./", relative_path))
      }
    };

    let mut magic_string = None;
    for item in AssetUrlIter::from(css_chunk.as_str()).into_asset_url_iter() {
      let s = magic_string.get_or_insert_with(|| string_wizard::MagicString::new(&css_chunk));
      match item {
        AssetUrlItem::Asset((range, reference_id, postfix)) => {
          let filename = ctx.get_file_name(reference_id)?;
          let filename = if let Some(postfix) = postfix {
            Cow::Owned(rolldown_utils::concat_string!(filename, postfix))
          } else {
            Cow::Borrowed(filename.as_str())
          };

          let vite_meta_data = ctx.meta().get::<ViteMetadata>().unwrap_or_else(|| {
            let value = Arc::new(ViteMetadata::default());
            ctx.meta().insert(Arc::<ViteMetadata>::clone(&value));
            value
          });
          vite_meta_data.imported_assets.insert(clean_url(&filename).into());

          let env = ToOutputFilePathEnv {
            url_base: &self.url_base,
            decoded_base: &self.decoded_base,
            render_built_url: self.render_built_url.as_deref(),
            render_built_url_config: RenderBuiltUrlConfig {
              r#type: "asset",
              is_ssr: self.is_ssr,
              host_id: &args.chunk.filename,
              host_type: "css",
            },
          };

          s.update(
            range.start,
            range.end,
            encode_uri_path(
              env.to_output_file_path(&filename, to_relative).await?.to_asset_url_in_css_or_html(),
            ),
          );
        }
        AssetUrlItem::PublicAsset((range, hash)) => {
          let cache = ctx
            .meta()
            .get::<PublicAssetUrlCache>()
            .ok_or_else(|| anyhow::anyhow!("PublicAssetUrlCache missing"))?;

          let public_url = cache
            .0
            .get(hash)
            .ok_or_else(|| {
              anyhow::anyhow!("Can't find the cache of {}", &css_chunk[range.clone()])
            })?
            .to_string();

          let env = ToOutputFilePathEnv {
            url_base: &self.url_base,
            decoded_base: &self.decoded_base,
            render_built_url: self.render_built_url.as_deref(),
            render_built_url_config: RenderBuiltUrlConfig {
              r#type: "public",
              is_ssr: self.is_ssr,
              host_id: &args.chunk.filename,
              host_type: "css",
            },
          };

          let relative_path = ctx.cwd().relative(css_asset_dirname.as_ref().unwrap());
          let relative_path = relative_path.to_slash_lossy();

          s.update(
            range.start,
            range.end,
            encode_uri_path(
              env
                .to_output_file_path(&public_url, |filename: &Path, host_id: &Path| {
                  AssetUrlResult::WithoutRuntime(rolldown_utils::concat_string!(
                    relative_path,
                    public_url
                  ))
                })
                .await?
                .to_asset_url_in_css_or_html(),
            ),
          );
        }
      }
    }

    Ok(if let Some(magic_string) = magic_string { magic_string.to_string() } else { css_chunk })
  }

  #[allow(clippy::unused_async)]
  pub async fn finalize_css(&self, _content: String) -> String {
    todo!()
  }

  pub async fn get_css_asset_dir_name(
    &self,
    css_asset_name: &str,
    css_file_names: &AssetFilenamesOutputOption,
  ) -> anyhow::Result<String> {
    match css_file_names {
      AssetFilenamesOutputOption::String(css_file_names) => {
        let assets_dir = if css_file_names.is_empty() {
          Path::new(&self.assets_dir)
        } else {
          Path::new(&css_file_names).parent().unwrap()
        };
        Ok(
          assets_dir
            .join(Path::new(css_asset_name).parent().unwrap())
            .to_slash_lossy()
            .into_owned(),
        )
      }
      AssetFilenamesOutputOption::Fn(css_file_names_fn) => {
        (css_file_names_fn)(&RollupPreRenderedAsset {
          names: vec![css_asset_name.into()],
          original_file_names: Vec::new(),
          source: "/* vite internal call, ignore */".to_owned().into(),
        })
        .await
      }
    }
  }

  pub fn get_css_bundle_name(&self, ctx: &PluginContext) -> anyhow::Result<String> {
    Ok(if let Some(css_asset_name) = ctx.meta().get::<CSSBundleName>() {
      css_asset_name.0.clone()
    } else {
      let css_bundle_name = if self.is_lib {
        if let Some(lib_css_filename) = &self.lib_css_filename {
          lib_css_filename.to_owned()
        } else {
          let mut base_dir = ctx.cwd().to_owned();
          loop {
            let pkg_path = base_dir.join("package.json");
            if pkg_path.is_file() {
              let json = std::fs::read_to_string(&pkg_path)?;
              let json = json.trim_start_matches("\u{feff}");
              let raw_json = serde_json::from_str::<serde_json::Value>(json)?;
              if let Some(json_object) = raw_json.as_object() {
                break json_object
                  .get("name")
                  .and_then(|field| field.as_str())
                  .map(ToString::to_string)
                  .ok_or_else(|| anyhow::anyhow!("Name in package.json is required if option 'build.lib.cssFileName' is not provided."))?;
              }
            }
            base_dir = match base_dir.parent() {
              Some(next) => next.to_path_buf(),
              None => {
                return Err(anyhow::anyhow!(
                  "Didn't find the nearest package.json when determining the library CSS bundle name.",
                ));
              }
            };
          }
        }
      } else {
        DEFAULT_CSS_BUNDLE_NAME.to_owned()
      };
      ctx.meta().insert(Arc::new(CSSBundleName(css_bundle_name.clone())));
      css_bundle_name
    })
  }
}
