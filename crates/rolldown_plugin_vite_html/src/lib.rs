mod html;
mod utils;

use std::{borrow::Cow, path::Path, rc::Rc, sync::Arc};

use cow_utils::CowUtils as _;
use derive_more::Debug;
use oxc::ast_visit::Visit;
use rolldown_common::side_effects::HookSideEffects;
use rolldown_plugin::{HookTransformOutput, HookUsage, LogWithoutPlugin, Plugin};
use rolldown_plugin_utils::{
  AssetUrlResult, RenderBuiltUrl, ToOutputFilePathEnv, UsizeOrFunction,
  constants::HTMLProxyMapItem, partial_encode_url_path,
};
use rolldown_utils::{dashmap::FxDashMap, pattern_filter::normalize_path};
use rustc_hash::{FxHashMap, FxHashSet};
use sugar_path::SugarPath as _;

use crate::utils::html_tag::{AttrValue, HtmlTagDescriptor};

#[derive(Debug, Default)]
pub struct ViteHtmlPlugin {
  pub is_lib: bool,
  pub is_ssr: bool,
  pub url_base: String,
  pub public_dir: String,
  pub decoded_base: String,
  pub module_preload_polyfill: bool,
  #[debug(skip)]
  pub asset_inline_limit: UsizeOrFunction,
  #[debug(skip)]
  pub render_built_url: Option<Arc<RenderBuiltUrl>>,
  html_result_map: FxDashMap<(String, String), (String, bool)>,
}

impl Plugin for ViteHtmlPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-html")
  }

  fn register_hook_usage(&self) -> rolldown_plugin::HookUsage {
    HookUsage::BuildStart | HookUsage::Transform | HookUsage::GenerateBundle
  }

  async fn build_start(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    self.html_result_map.clear();
    Ok(())
  }

  #[expect(clippy::too_many_lines)]
  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !args.id.ends_with(".html") {
      return Ok(None);
    }

    let id = normalize_path(args.id);
    let path = args.id.relative(ctx.cwd());
    let path_lossy = path.to_string_lossy();
    let relative_url_path = normalize_path(&path_lossy);

    let public_path = rolldown_utils::concat_string!("/", relative_url_path);
    let public_base = self.get_base_in_html(&relative_url_path);
    let public_to_relative = |filename: &Path, _: &Path| {
      AssetUrlResult::WithoutRuntime(rolldown_utils::concat_string!(
        &public_base,
        filename.to_string_lossy()
      ))
    };
    let env = ToOutputFilePathEnv {
      is_ssr: self.is_ssr,
      host_id: &relative_url_path,
      url_base: &self.url_base,
      decoded_base: &self.decoded_base,
      render_built_url: self.render_built_url.as_deref(),
    };

    let mut js = String::new();
    let mut inline_module_count = 0usize;
    let mut every_script_is_async = true;
    let mut some_scripts_are_async = false;
    let mut some_scripts_are_defer = false;

    let style_urls = Vec::new();
    let mut script_urls = Vec::new();

    // TODO: Support module_side_effects for module info
    // let mut set_modules = Vec::new();
    let mut overwrite_attrs = Vec::new();
    let mut s = string_wizard::MagicString::new(args.code);

    // TODO: Extract into a function
    {
      let dom = html::parser::parse_html(args.code);
      let mut stack = vec![dom.document];
      while let Some(node) = stack.pop() {
        match &node.data {
          html::sink::NodeData::Element { name, attrs, span } => {
            let mut should_remove = false;
            if &**name == "script" {
              let mut src = None;
              let mut is_async = false;
              let mut is_module = false;
              let mut is_ignored = false;
              for attr in attrs.borrow().iter() {
                match &*attr.name {
                  "src" => {
                    if src.is_none() {
                      src = Some((attr.value.clone(), attr.span));
                    }
                  }
                  "type" if attr.value == "module" => {
                    is_module = true;
                  }
                  "async" => {
                    is_async = true;
                  }
                  "vite-ignore" => {
                    is_ignored = true;
                    s.remove(attr.span.start, attr.span.end);
                  }
                  _ => {}
                }
              }
              if !is_ignored {
                let is_public_file = src.as_ref().is_some_and(|(s, _)| {
                  rolldown_plugin_utils::check_public_file(s, &self.public_dir).is_some()
                });
                if is_public_file && let Some((ref url, span)) = src {
                  overwrite_attrs.push((url[1..].to_owned(), span));
                }
                if is_module {
                  inline_module_count += 1;
                  if let Some((url, _)) = src.as_ref()
                    && !is_public_file
                    && !utils::is_excluded_url(url)
                  {
                    // TODO: Support module_side_effects for module info
                    // set_modules.push(url);
                    // add `<script type="module" src="..."/>` as an import
                    js.push_str(&rolldown_utils::concat_string!(
                      "import ",
                      rolldown_plugin_utils::to_string_literal(url),
                      "\n"
                    ));
                    should_remove = true;
                  } else if let Some(node) = node.children.borrow_mut().pop() {
                    let html::sink::NodeData::Text { contents, .. } = &node.data else {
                      panic!("Expected text node but received: {:#?}", node.data);
                    };
                    self.add_to_html_proxy_cache(
                      &ctx,
                      public_path.clone(),
                      inline_module_count - 1,
                      HTMLProxyMapItem { code: contents.into(), map: None },
                    );
                    js.push_str(&rolldown_utils::concat_string!(
                      "import \"",
                      id,
                      "?html-proxy&index=",
                      itoa::Buffer::new().format(inline_module_count - 1),
                      ".js\"\n"
                    ));
                    should_remove = true;
                  }
                  every_script_is_async = every_script_is_async && is_async;
                  some_scripts_are_async = some_scripts_are_async || is_async;
                  some_scripts_are_defer = some_scripts_are_defer || !is_async;
                } else if let Some((url, _)) = src.as_ref()
                  && !is_public_file
                {
                  if !utils::is_excluded_url(url) {
                    let message = rolldown_utils::concat_string!(
                      "<script src='",
                      url,
                      "'> in '",
                      public_path,
                      "' can't be bundled without type='module' attribute"
                    );
                    ctx.warn(LogWithoutPlugin { message, ..Default::default() });
                  }
                } else if let Some(node) = node.children.borrow_mut().pop() {
                  let html::sink::NodeData::Text { contents, span } = &node.data else {
                    panic!("Expected text node but received: {:#?}", node.data);
                  };
                  if utils::constant::INLINE_IMPORT.is_match(contents) {
                    let allocator = oxc::allocator::Allocator::default();
                    let parser_ret = oxc::parser::Parser::new(
                      &allocator,
                      contents,
                      oxc::span::SourceType::default(),
                    )
                    .parse();
                    if parser_ret.panicked
                      && let Some(err) = parser_ret
                        .errors
                        .iter()
                        .find(|e| e.severity == oxc::diagnostics::Severity::Error)
                    {
                      return Err(anyhow::anyhow!(format!(
                        "Failed to parse inline script in '{}': {:?}",
                        public_path, err.message
                      )));
                    }
                    let mut visitor = utils::ScriptInlineImportVisitor {
                      offset: span.start,
                      script_urls: &mut script_urls,
                    };
                    visitor.visit_program(&parser_ret.program);
                  }
                }
              }
            }

            // Handle attributes like src/href
            if matches!(
              &**name,
              "audio"
                | "embed"
                | "img"
                | "image"
                | "input"
                | "link"
                | "meta"
                | "object"
                | "source"
                | "track"
                | "use"
                | "video"
            ) {
              if let Some(attr) = attrs.borrow().iter().find(|a| &*a.name == "vite-ignore") {
                s.remove(attr.span.start, attr.span.end);
              } else {
                todo!()
              }
            }

            // Handle <tag style="..." />
            if let Some(attr) = attrs.borrow().iter().find(|a| {
              &*a.name == "style" && (a.value.contains("url(") || a.value.contains("image-set("))
            }) {
              self.handle_style_tag_or_attribute(
                &mut s,
                &mut js,
                &id,
                &ctx,
                public_path.clone(),
                &mut inline_module_count,
                true,
                (attr.value.as_str(), attr.span),
              )?;
            }

            // Handle <style>...</style>
            if &**name == "style"
              && let Some(node) = node.children.borrow_mut().pop()
            {
              let html::sink::NodeData::Text { ref contents, span } = node.data else {
                panic!("Expected text node but received: {:#?}", node.data);
              };
              self.handle_style_tag_or_attribute(
                &mut s,
                &mut js,
                &id,
                &ctx,
                public_path.clone(),
                &mut inline_module_count,
                false,
                (contents, span),
              )?;
            }

            if should_remove {
              s.remove(span.start, span.end);
            }
          }
          _ => {}
        }
        for child in node.children.borrow().iter() {
          stack.push(Rc::clone(child));
        }
      }
    }

    for (url, span) in overwrite_attrs {
      let asset_url = env.to_output_file_path(&url, "html", true, public_to_relative).await?;
      utils::overwrite_check_public_file(
        &mut s,
        span.start..span.end,
        partial_encode_url_path(&asset_url.to_asset_url_in_css_or_html()).into_owned(),
      )?;
    }

    if some_scripts_are_async && some_scripts_are_defer {
      let message = rolldown_utils::concat_string!(
        "\nMixed async and defer script modules in ",
        id,
        ", output script will fallback to defer. Every script, including inline ones, need to be marked as async for your output script to be async."
      );
      ctx.warn(LogWithoutPlugin { message, ..Default::default() });
    }

    for (url, range) in script_urls {
      let url = if rolldown_plugin_utils::check_public_file(&url, &self.public_dir).is_some() {
        env
          .to_output_file_path(&url, "html", true, public_to_relative)
          .await?
          .to_asset_url_in_css_or_html()
      } else if !utils::is_excluded_url(&url) {
        self.url_to_built_url(&ctx, &url, &id, None).await?
      } else {
        continue;
      };
      utils::overwrite_check_public_file(
        &mut s,
        range,
        partial_encode_url_path(&url).into_owned(),
      )?;
    }

    let resolved_style_urls = rolldown_utils::futures::block_on_spawn_all(
      style_urls.into_iter().map(async |(url, range): (&str, std::ops::Range<usize>)| {
        (url, range, ctx.resolve(url, Some(&id), None).await)
      }),
    )
    .await;

    for (url, range, resolved) in resolved_style_urls {
      match resolved?.ok() {
        Some(_) => {
          s.remove(range.start, range.end);
        }
        None => {
          ctx.warn(LogWithoutPlugin {
            message: format!("\n{url} doesn't exist at build time, it will remain unchanged to be resolved at runtime"),
            ..Default::default()
          });
          js = js
            .cow_replace(
              &rolldown_utils::concat_string!(
                "import ",
                rolldown_plugin_utils::to_string_literal(url),
                "\n"
              ),
              "",
            )
            .into_owned();
        }
      }
    }

    self
      .html_result_map
      .insert((args.id.to_string(), public_path), (s.to_string(), every_script_is_async));

    if self.module_preload_polyfill && (some_scripts_are_async || some_scripts_are_defer) {
      js = rolldown_utils::concat_string!(
        "import \"",
        utils::constant::MODULE_PRELOAD_POLYFILL,
        "\"\n",
        js
      );
    }

    // TODO: Support module_side_effects for module info
    // for url in set_modules {
    //   match ctx.resolve(&url, Some(args.id), None).await? {
    //     Ok(resolved_id) => match ctx.get_module_info(&resolved_id.id) {
    //       Some(module_info) => module_info.module_side_effects = true,
    //       None => {
    //         if !resolved_id.external.is_external() {
    //           ctx.resolve(specifier, importer, extra_options)
    //         }
    //       },
    //     },
    //     Err(_) => return Err(anyhow::anyhow!("Failed to resolve {url} from {}", args.id)),
    //   }
    // }

    // Force this module to keep from being shared between other entry points.
    // If the resulting chunk is empty, it will be removed in generateBundle.
    Ok(Some(HookTransformOutput {
      code: js.into(),
      side_effects: Some(HookSideEffects::NoTreeshake),
      ..Default::default()
    }))
  }

  #[expect(unused_variables)]
  async fn generate_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    let mut inline_entry_chunk = FxHashSet::default();
    for item in &self.html_result_map {
      let ((id, assets_base), (html, is_async)) = item.pair();

      let mut result = html.to_string();

      let path = id.relative(ctx.cwd());
      let path_lossy = path.to_string_lossy();
      let relative_url_path = normalize_path(&path_lossy);

      let mut can_inline_entry = false;

      let chunk = args.bundle.iter().find_map(|o| match o {
        rolldown_common::Output::Chunk(chunk) => (chunk.is_entry
          && chunk
            .facade_module_id
            .as_ref()
            .is_some_and(|facade_module_id| facade_module_id.resource_id() == id))
        .then_some(chunk),
        rolldown_common::Output::Asset(_) => None,
      });

      // inject chunk asset links
      if let Some(chunk) = chunk {
        // an entry chunk can be inlined if
        //  - it's an ES module (e.g. not generated by the legacy plugin)
        //  - it contains no meaningful code other than import statements
        if args.options.format.is_esm() && utils::is_entirely_import(&chunk.code) {
          can_inline_entry = true;
        }

        // when not inlined, inject <script> for entry and modulepreload its dependencies
        // when inlined, discard entry chunk and inject <script> for everything in post-order
        let imports = utils::get_imported_chunks(chunk, args.bundle);

        let asset_tags = if can_inline_entry {
          let mut tags = Vec::with_capacity(imports.len());
          for imported_chunk in imports {
            let mut tag = HtmlTagDescriptor::new("script");
            let url = match imported_chunk {
              utils::ImportedChunk::External(external) => external.to_string(),
              utils::ImportedChunk::Chunk(chunk) => {
                self
                  .to_output_file_path(&chunk.filename, assets_base, false, &relative_url_path)
                  .await?
              }
            };
            tag.attrs = Some(FxHashMap::from_iter([
              ("type", AttrValue::String("module".to_owned())),
              ("crossorigin", AttrValue::Boolean(true)),
              ("src", AttrValue::String(url)),
            ]));
            tags.push(tag);
          }
          tags
        } else {
          todo!()
        };
        todo!()
      }

      if let Some(s) = Self::handle_inline_css(ctx, &result) {
        result = s.to_string();
      }

      // TODO: applyHtmlTransforms
      // result = await applyHtmlTransforms(..)

      if let Some(s) =
        self.handle_html_asset_url(ctx, html, chunk, assets_base, &relative_url_path).await?
      {
        result = s;
      }

      if let Some(chunk) = chunk
        && can_inline_entry
      {
        inline_entry_chunk.insert(chunk.filename.clone());
      }

      ctx
        .emit_file_async(rolldown_common::EmittedAsset {
          name: None,
          original_file_name: Some(id.to_string()),
          file_name: Some(relative_url_path.into()),
          source: rolldown_common::StrOrBytes::Str(result),
        })
        .await?;
    }

    // all imports from entry have been inlined to html, prevent outputting it
    args.bundle.retain(|o| match o {
      rolldown_common::Output::Chunk(chunk) => !inline_entry_chunk.contains(&chunk.filename),
      rolldown_common::Output::Asset(asset) => true,
    });

    Ok(())
  }
}
