use std::{
  borrow::Cow,
  ops::Deref,
  path::{Path, PathBuf},
  sync::{Arc, LazyLock, atomic::Ordering},
};

use arcstr::ArcStr;
use regex::Regex;
use rolldown_common::{
  AssetFilenamesOutputOption, EmittedAsset, Output, OutputChunk, OutputFormat,
  RollupPreRenderedAsset,
};
use rolldown_plugin::{HookRenderChunkArgs, PluginContext};
use rolldown_plugin_utils::{
  AssetUrlItem, AssetUrlIter, AssetUrlResult, PublicAssetUrlCache, RenderAssetUrlInJsEnv,
  ToOutputFilePathEnv,
  constants::{
    CSSBundleName, CSSChunkCache, CSSEntriesCache, CSSStyles, PureCSSChunks, ViteMetadata,
  },
  create_to_import_meta_url_based_relative_runtime,
  css::is_css_request,
  get_chunk_original_name,
  uri::encode_uri_path,
};
use rolldown_utils::{futures::block_on_spawn_all, indexmap::FxIndexSet, url::clean_url};
use rustc_hash::{FxHashMap, FxHashSet};
use string_wizard::MagicString;
use sugar_path::SugarPath;

use crate::ViteCssPostPlugin;

pub const VITE_HASH_UPDATE_MARKER: &str = "/*$vite$:1*/";
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

static AT_IMPORT_RE: LazyLock<Regex> = std::sync::LazyLock::new(|| {
  Regex::new(r#"@import(?:\s*(?:url\([^)]*\)|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')[^;]*|[^;]*);"#)
    .unwrap()
});

static AT_CHARSET_RE: LazyLock<Regex> = std::sync::LazyLock::new(|| {
  Regex::new(r#"@charset(?:\s*(?:"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*').*?|[^;]*);"#).unwrap()
});

static MULTI_LINE_COMMENTS_RE: LazyLock<Regex> =
  std::sync::LazyLock::new(|| Regex::new(r"\/\*[^*]*\*+(?:[^/*][^*]*\*+)*\/").unwrap());

pub fn extract_index(id: &str) -> Option<&str> {
  let s = id.split_once("&index=")?.1;
  let end = s.as_bytes().iter().take_while(|b| b.is_ascii_digit()).count();
  (end > 0).then_some(&s[..end])
}

#[derive(Debug, Default)]
pub struct UrlEmitTasks {
  pub range: (usize, usize),
  pub replacement: String,
}

pub struct FinalizedContext<'a, 'b, 'c> {
  pub plugin_ctx: &'a PluginContext,
  pub env: &'a ToOutputFilePathEnv<'b>,
  pub args: &'a HookRenderChunkArgs<'c>,
}

impl Deref for FinalizedContext<'_, '_, '_> {
  type Target = PluginContext;

  fn deref(&self) -> &Self::Target {
    self.plugin_ctx
  }
}

impl ViteCssPostPlugin {
  pub async fn finalize_vite_css_urls<'a>(
    &self,
    ctx: &FinalizedContext<'a, '_, '_>,
    css_styles: &CSSStyles,
    magic_string: &mut Option<MagicString<'a>>,
  ) -> anyhow::Result<()> {
    let mut css_url_iter = ctx.args.code.match_indices("__VITE_CSS_URL__").peekable();
    if css_url_iter.peek().is_some() {
      let vite_metadata = ctx.meta().get::<ViteMetadata>().expect("ViteMetadata missing");
      let tasks = css_url_iter.map(async |(index, _)| {
        let start = index + "__VITE_CSS_URL__".len();
        let Some(pos) = ctx.args.code[start..].find("__") else {
          return Err(anyhow::anyhow!(
            "Invalid __VITE_CSS_URL__ in '{}', expected '__VITE_CSS_URL__<base64>__'",
            ctx.args.chunk.name
          ));
        };

        let id = unsafe {
          String::from_utf8_unchecked(
            base64_simd::STANDARD
              .decode_to_vec(&ctx.args.code[start..start + pos])
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
            style.to_owned(),
            &css_asset_name,
            &ctx.args.options.asset_filenames,
          )
          .await?;
        let content = self.finalize_css(content).await?;

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

        let url = ctx
          .env
          .to_output_file_path(
            &filename,
            "js",
            false,
            create_to_import_meta_url_based_relative_runtime(ctx.options().format, self.is_worker),
          )
          .await?;

        Ok(UrlEmitTasks { range: (index, index + pos + 2), replacement: url.to_asset_url_in_js()? })
      });

      let magic_string =
        magic_string.get_or_insert_with(|| string_wizard::MagicString::new(&ctx.args.code));
      for task in block_on_spawn_all(tasks).await {
        let UrlEmitTasks { range: (start, end), replacement } = task?;
        magic_string.update(start, end, replacement);
      }
    }
    Ok(())
  }

  #[expect(clippy::too_many_lines)]
  pub async fn finalize_css_chunk<'a>(
    &self,
    ctx: &FinalizedContext<'a, '_, '_>,
    css_chunk: String,
    is_pure_css_chunk: bool,
    magic_string: &mut Option<MagicString<'a>>,
  ) -> anyhow::Result<()> {
    if css_chunk.is_empty() {
      return Ok(());
    }

    if is_pure_css_chunk && ctx.args.options.format.is_esm_or_cjs() {
      ctx
        .meta()
        .get::<PureCSSChunks>()
        .expect("PureCSSChunks missing")
        .inner
        .insert(ctx.args.chunk.filename.clone());
    }

    if self.css_code_split {
      if ctx.args.options.format.is_esm_or_cjs() && !ctx.args.chunk.filename.contains("-legacy") {
        let css_asset_path = PathBuf::from(ctx.args.chunk.name.as_str()).with_extension("css");
        // if facadeModuleId doesn't exist or doesn't have a CSS extension,
        // that means a JS entry file imports a CSS file.
        // in this case, only use the filename for the CSS chunk name like JS chunks.
        let css_asset_name = if ctx.args.chunk.is_entry
          && ctx
            .args
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
          &ctx.args.chunk.name,
          ctx.args.chunk.facade_module_id.as_ref(),
        );

        let content = self
          .resolve_asset_urls_in_css(
            ctx,
            css_chunk,
            &css_asset_name,
            &ctx.args.options.asset_filenames,
          )
          .await?;
        let content = self.finalize_css(content).await?;

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

        if ctx.args.chunk.is_entry && is_pure_css_chunk {
          ctx
            .meta()
            .get::<CSSEntriesCache>()
            .expect("CSSEntriesCache missing")
            .inner
            .insert(reference_id);
        }
      } else if self.is_client {
        let injection_point = match ctx.args.options.format {
          OutputFormat::Esm => {
            if ctx.args.code.starts_with("#!") {
              ctx.args.code.find('\n').unwrap_or(0)
            } else {
              0
            }
          }
          OutputFormat::Iife | OutputFormat::Umd => {
            let regex = if matches!(ctx.args.options.format, OutputFormat::Iife) {
              &RE_IIFE
            } else {
              &RE_UMD
            };
            let Some(m) = regex.find(&ctx.args.code) else {
              return Err(anyhow::anyhow!("Injection point for inlined CSS not found"));
            };
            m.end()
          }
          OutputFormat::Cjs => {
            return Err(anyhow::anyhow!("CJS format is not supported for CSS injection"));
          }
        };

        let content = serde_json::to_string(&self.finalize_css(css_chunk).await?)?;
        let env =
          RenderAssetUrlInJsEnv { ctx, env: ctx.env, code: &content, is_worker: self.is_worker };

        let css_string = env.render_asset_url_in_js().await?.unwrap_or(content);
        let inject_code = rolldown_utils::concat_string!(
          "var __vite_style__ = document.createElement('style');__vite_style__.textContent = ",
          css_string,
          ";document.head.appendChild(__vite_style__);"
        );

        magic_string
          .get_or_insert_with(|| string_wizard::MagicString::new(&ctx.args.code))
          .append_right(injection_point, inject_code);
      }
    } else {
      ctx.meta().get::<CSSChunkCache>().expect("CSSChunkCache missing").inner.insert(
        ctx.args.chunk.filename.clone(),
        self
          .resolve_asset_urls_in_css(
            ctx,
            css_chunk,
            &self.get_css_bundle_name(ctx)?,
            &ctx.args.options.asset_filenames,
          )
          .await?,
      );
    }

    Ok(())
  }

  pub async fn resolve_asset_urls_in_css(
    &self,
    ctx: &FinalizedContext<'_, '_, '_>,
    css_chunk: String,
    css_asset_name: &str,
    css_file_names: &AssetFilenamesOutputOption,
  ) -> anyhow::Result<String> {
    let css_asset_dirname = if self.url_base.is_empty() || self.url_base == "./" {
      Some(self.get_css_asset_dir_name(css_asset_name, css_file_names).await?)
    } else {
      None
    };

    let to_relative = |filename: &Path, _host_id: &Path| {
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
            is_ssr: self.is_ssr,
            host_id: &ctx.args.chunk.filename,
            url_base: &self.url_base,
            decoded_base: &self.decoded_base,
            render_built_url: self.render_built_url.as_deref(),
          };

          s.update(
            range.start,
            range.end,
            encode_uri_path(
              env
                .to_output_file_path(&filename, "css", false, to_relative)
                .await?
                .to_asset_url_in_css_or_html(),
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
            is_ssr: self.is_ssr,
            host_id: &ctx.args.chunk.filename,
            url_base: &self.url_base,
            decoded_base: &self.decoded_base,
            render_built_url: self.render_built_url.as_deref(),
          };

          let relative_path = ctx.cwd().relative(css_asset_dirname.as_ref().unwrap());
          let relative_path = relative_path.to_slash_lossy();

          s.update(
            range.start,
            range.end,
            encode_uri_path(
              env
                .to_output_file_path(&public_url, "css", true, |_: &Path, _: &Path| {
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

  pub async fn finalize_css(&self, mut content: String) -> anyhow::Result<String> {
    // hoist external @imports and @charset to the top of the CSS chunk per spec (#1845 and #6333)
    if content.contains("@import") || content.contains("@charset") {
      content = Self::hoist_at_rules(&content);
    }
    // TODO: Maybe we should use internal lightningcss minify
    if let Some(css_minify) = &self.css_minify {
      content = (css_minify)(content).await?;
    }
    // inject an additional string to generate a different hash for https://github.com/vitejs/vite/issues/18038
    //
    // pre-5.4.3, we generated CSS link tags without crossorigin attribute and generated an hash without
    // this string
    // in 5.4.3, we added crossorigin attribute to the generated CSS link tags but that made chromium browsers
    // to block the CSSs from loading due to chromium's weird behavior
    // (https://www.hacksoft.io/blog/handle-images-cors-error-in-chrome, https://issues.chromium.org/issues/40381978)
    // to avoid that happening, we inject an additional string so that a different hash is generated
    // for the same CSS content
    Ok(rolldown_utils::concat_string!(content, VITE_HASH_UPDATE_MARKER))
  }

  pub fn hoist_at_rules(css: &str) -> String {
    let mut css_without_comments = css.to_owned();
    let bytes = unsafe { css_without_comments.as_bytes_mut() };
    for matched in MULTI_LINE_COMMENTS_RE.find_iter(css) {
      bytes[matched.range()].fill(b' ');
    }

    let mut s = string_wizard::MagicString::new(css);
    for matched in AT_IMPORT_RE.find_iter(&css_without_comments) {
      s.remove(matched.start(), matched.end());
      s.append_left(0, matched.as_str());
    }

    let mut found_charset = false;
    for matched in AT_CHARSET_RE.find_iter(&css_without_comments) {
      s.remove(matched.start(), matched.end());
      if !found_charset {
        s.prepend(matched.as_str());
        found_charset = true;
      }
    }

    s.to_string()
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

  pub async fn emit_non_codesplit_css_bundle(
    &self,
    ctx: &PluginContext,
    bundle: &mut [Output],
  ) -> anyhow::Result<()> {
    if !self.css_code_split && !self.has_emitted.load(Ordering::Relaxed) {
      fn collect(
        ctx: &PluginContext,
        chunk: &OutputChunk,
        bundle: &FxHashMap<ArcStr, Arc<OutputChunk>>,
        collected: &mut FxHashSet<ArcStr>,
        dynamic_imports: &mut FxIndexSet<ArcStr>,
        extracted_css: &mut String,
      ) {
        if collected.contains(&chunk.filename) {
          return;
        }
        collected.insert(chunk.filename.clone());
        // First collect all styles from the synchronous imports (lowest priority)
        chunk.imports.iter().for_each(|name| {
          if let Some(chunk) = bundle.get(name) {
            collect(ctx, chunk, bundle, collected, dynamic_imports, extracted_css);
          }
        });
        // Save dynamic imports in deterministic order to add the styles later (to have the highest priority)
        chunk.dynamic_imports.iter().for_each(|name| {
          dynamic_imports.insert(name.clone());
        });
        // Then collect the styles of the current chunk (might overwrite some styles from previous imports)
        if let Some(css_chunk) = ctx
          .meta()
          .get::<CSSChunkCache>()
          .expect("CSSChunkCache is missing")
          .inner
          .get(&Into::<ArcStr>::into(&chunk.preliminary_filename))
        {
          extracted_css.push_str(&css_chunk);
        }
      }

      let chunks = bundle
        .iter()
        .filter_map(|output| match output {
          Output::Chunk(chunk) => Some((chunk.filename.clone(), Arc::clone(chunk))),
          Output::Asset(_) => None,
        })
        .collect::<FxHashMap<_, _>>();

      let mut extracted_css = String::new();
      let mut collected = FxHashSet::default();
      let mut dynamic_imports = FxIndexSet::default();
      // The bundle is guaranteed to be deterministic, if not then we have a bug in rollup.
      // So we use it to ensure a deterministic order of styles
      let mut bundle_iter = bundle.iter();
      while let Some(Output::Chunk(chunk)) = bundle_iter.next()
        && chunk.is_entry
      {
        collect(ctx, chunk, &chunks, &mut collected, &mut dynamic_imports, &mut extracted_css);
      }
      // Now collect the dynamic chunks, this is done last to have the styles overwrite the previous ones
      while let imports = std::mem::take(&mut dynamic_imports)
        && !imports.is_empty()
      {
        for name in imports {
          if let Some(chunk) = chunks.get(&name) {
            collect(ctx, chunk, &chunks, &mut collected, &mut dynamic_imports, &mut extracted_css);
          }
        }
      }

      if !extracted_css.is_empty() {
        self.has_emitted.store(true, Ordering::Relaxed);
        ctx
          .emit_file_async(rolldown_common::EmittedAsset {
            name: Some(self.get_css_bundle_name(ctx)?),
            // this file is an implicit entry point, use `style.css` as the original file name
            // this name is also used as a key in the manifest
            original_file_name: Some("style.css".to_owned()),
            source: self.finalize_css(extracted_css).await?.into(),
            ..Default::default()
          })
          .await?;
      }
    }
    Ok(())
  }

  pub fn prune_pure_css_chunks(
    &self,
    ctx: &PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) {
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
            Path::new(file.as_str()).file_name().and_then(|v| v.to_str().map(regex::escape))
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

      for output in args.bundle.iter_mut() {
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
                let len = captures.get(0).unwrap().len();
                if args.options.format.is_esm() {
                  return format!("/* empty css {:<width$}*/", "", width = len.saturating_sub(15));
                }
                if let Some(p2) = captures.get(2)
                  && p2.as_str() == ";"
                {
                  return format!(";/* empty css {:<width$}*/", "", width = len.saturating_sub(16));
                }
                let p1 = captures.get(1).map_or("", |m| m.as_str());
                format!(
                  "{p1}/* empty css {:<width$}*/",
                  "",
                  width = len.saturating_sub(15 + p1.len())
                )
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
  }
}
