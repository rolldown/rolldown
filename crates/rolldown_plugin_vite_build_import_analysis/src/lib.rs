mod ast_utils;
mod ast_visit;
mod utils;

use std::{
  borrow::Cow,
  path::{Path, PathBuf},
  sync::Arc,
};

use arcstr::ArcStr;
use itertools::Itertools as _;
use oxc::ast_visit::VisitMut;
use rolldown_common::{Output, side_effects::HookSideEffects};
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookRenderChunkOutput, HookResolveIdArgs,
  HookResolveIdOutput, HookResolveIdReturn, HookTransformAstArgs, HookTransformAstReturn,
  HookUsage, Plugin, PluginContext, SharedLoadPluginContext,
};
use rolldown_plugin_utils::{
  AssetUrlResult, ModulePreload, RenderBuiltUrl, ToOutputFilePathEnv,
  constants::RemovedPureCSSFilesCache, to_string_literal,
};
use rustc_hash::{FxHashMap, FxHashSet};
use sugar_path::SugarPath as _;

use crate::{
  ast_visit::{DynamicImportCollectVisitor, DynamicImportVisitor},
  utils::{AddDeps, FileDeps},
};

use self::ast_visit::BuildImportAnalysisVisitor;

const IS_MODERN_FLAG: &str = "__VITE_IS_MODERN__";
const PRELOAD_HELPER_ID: &str = "\0vite/preload-helper.js";

#[derive(derive_more::Debug)]
pub struct ViteBuildImportAnalysisPluginV2 {
  pub is_ssr: bool,
  pub url_base: String,
  pub decoded_base: String,
  pub module_preload: ModulePreload,
  #[debug(skip)]
  pub render_built_url: Option<Arc<RenderBuiltUrl>>,
}

#[derive(derive_more::Debug)]
pub struct ViteBuildImportAnalysisPlugin {
  pub preload_code: ArcStr,
  pub insert_preload: bool,
  pub render_built_url: bool,
  pub is_relative_base: bool,
  pub v2: Option<ViteBuildImportAnalysisPluginV2>,
}

impl Plugin for ViteBuildImportAnalysisPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-build-import-analysis")
  }

  fn register_hook_usage(&self) -> HookUsage {
    if self.v2.is_some() {
      HookUsage::ResolveId
        | HookUsage::Load
        | HookUsage::TransformAst
        | HookUsage::RenderChunk
        | HookUsage::GenerateBundle
    } else {
      HookUsage::ResolveId | HookUsage::Load | HookUsage::TransformAst
    }
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    Ok(
      (args.specifier == PRELOAD_HELPER_ID)
        .then_some(HookResolveIdOutput { id: args.specifier.into(), ..Default::default() }),
    )
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    Ok((args.id == PRELOAD_HELPER_ID).then_some(HookLoadOutput {
      code: self.preload_code.clone(),
      side_effects: Some(HookSideEffects::False),
      ..Default::default()
    }))
  }

  async fn transform_ast(
    &self,
    ctx: &PluginContext,
    args: HookTransformAstArgs<'_>,
  ) -> HookTransformAstReturn {
    let mut ast = args.ast;
    ast.program.with_mut(|fields| {
      let builder = AstSnippet::new(fields.allocator);
      let mut visitor = BuildImportAnalysisVisitor::new(
        builder,
        self.insert_preload,
        self.render_built_url,
        self.is_relative_base,
        ctx.options().format.is_esm(),
      );
      visitor.visit_program(fields.program);
    });
    Ok(ast)
  }

  async fn render_chunk(
    &self,
    _ctx: &PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    if args.code.contains(IS_MODERN_FLAG) {
      let is_modern = args.options.format.is_esm();
      let replacement = if is_modern { "true" } else { "false" };

      let mut code = args.code.clone();
      for (index, _) in args.code.match_indices(IS_MODERN_FLAG) {
        let bytes = unsafe { code.as_bytes_mut() };
        let replacement_bytes = replacement.as_bytes();
        bytes[index..index + replacement_bytes.len()].copy_from_slice(replacement_bytes);
        bytes[index + replacement_bytes.len()..index + IS_MODERN_FLAG.len()].fill(b' ');
      }

      Ok(Some(HookRenderChunkOutput { code, map: None }))
    } else {
      Ok(None)
    }
  }

  #[expect(clippy::too_many_lines)]
  async fn generate_bundle(
    &self,
    ctx: &PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if !args.options.format.is_esm() {
      return Ok(());
    }

    let Some(ViteBuildImportAnalysisPluginV2 {
      is_ssr,
      ref url_base,
      ref decoded_base,
      ref module_preload,
      ref render_built_url,
    }) = self.v2
    else {
      return Ok(());
    };

    if !self.insert_preload {
      if let Some(removed_pure_css_files) = ctx.meta().get::<RemovedPureCSSFilesCache>()
        && !removed_pure_css_files.inner.is_empty()
      {
        for output in args.bundle.iter_mut() {
          // TODO: Maybe we should use `chunk.dynamicImports`?
          if let Output::Chunk(chunk) = output
            && utils::DYNAMIC_IMPORT_RE.is_match(&chunk.code)
          {
            let allocator = oxc::allocator::Allocator::default();
            let mut parser_ret = oxc::parser::Parser::new(
              &allocator,
              chunk.code.as_ref(),
              oxc::span::SourceType::default(),
            )
            .parse();
            if parser_ret.panicked
              && let Some(err) =
                parser_ret.errors.iter().find(|e| e.severity == oxc::diagnostics::Severity::Error)
            {
              return Err(anyhow::anyhow!(format!(
                "Failed to parse code in '{}': {:?}",
                chunk.filename, err.message
              )));
            }

            let mut s = None;
            let mut visitor = DynamicImportVisitor {
              s: &mut s,
              code: &chunk.code,
              removed_pure_css_files: &removed_pure_css_files,
              chunk_filename_dir: PathBuf::from(chunk.filename.as_str())
                .parent()
                .unwrap()
                .to_path_buf(),
            };

            visitor.visit_program(&mut parser_ret.program);

            if let Some(s) = s {
              *chunk = Arc::new(rolldown_common::OutputChunk {
                code: s.to_string(),
                ..chunk.as_ref().clone()
              });
            }
          }
        }
      }
      return Ok(());
    }

    let bundle = args
      .bundle
      .iter()
      .map(|output| (output.filename().to_string(), output.clone()))
      .collect::<FxHashMap<_, _>>();

    // can't use chunk.dynamicImports.length here since some modules e.g.
    // dynamic import to constant json may get inlined.
    for output in args.bundle.iter_mut() {
      if let Output::Chunk(chunk) = output
        && chunk.code.contains("__VITE_PRELOAD__")
      {
        let mut imports = Vec::new();

        {
          let allocator = oxc::allocator::Allocator::default();
          let mut parser_ret = oxc::parser::Parser::new(
            &allocator,
            chunk.code.as_ref(),
            oxc::span::SourceType::default(),
          )
          .parse();
          if parser_ret.panicked
            && let Some(err) =
              parser_ret.errors.iter().find(|e| e.severity == oxc::diagnostics::Severity::Error)
          {
            return Err(anyhow::anyhow!(format!(
              "Failed to parse code in '{}': {:?}",
              chunk.filename, err.message
            )));
          }

          let mut visitor = DynamicImportCollectVisitor { imports: &mut imports };

          visitor.visit_program(&mut parser_ret.program);
        }

        let imports_len = imports.len();
        let mut rewrote_marker_start_pos = FxHashSet::default();
        let mut file_deps = FileDeps(Vec::with_capacity(imports.len()));
        let mut s = string_wizard::MagicString::new(&chunk.code);

        for import in imports {
          let mut deps = FxHashSet::default();
          let mut has_removed_pure_css_chunks = false;

          let normalized_file = import.source.map(|url| {
            let file = PathBuf::from(chunk.filename.as_str());
            let file_dir = file.parent().unwrap();
            let normalized_file =
              file_dir.join(url.as_str()).normalize().to_slash_lossy().into_owned();

            let mut collector = AddDeps {
              s: &mut s,
              ctx,
              deps: &mut deps,
              owner_filename: chunk.filename.to_string(),
              analyzed: FxHashSet::default(),
              has_removed_pure_css_chunks: &mut has_removed_pure_css_chunks,
              expr_range: import.start..import.end,
            };

            collector.add_deps(&bundle, &normalized_file);
            normalized_file
          });

          let mut marker_start = utils::find_marker_pos(&chunk.code, import.end);

          if marker_start.is_none() && imports_len == 1 {
            marker_start = utils::find_marker_pos(&chunk.code, 0);
          }

          if let Some(marker_start) = marker_start
            && marker_start > 0
          {
            // the dep list includes the main chunk, so only need to reload when there are actual other deps.
            let mut deps_arr = if deps.len() > 1 ||
              // main chunk is removed
              (has_removed_pure_css_chunks && !deps.is_empty())
            {
              if module_preload.is_false() {
                // CSS deps use the same mechanism as module preloads, so even if disabled,
                // we still need to pass these deps to the preload helper in dynamic imports.
                deps.into_iter().filter(|dep| dep.ends_with(".css")).collect()
              } else {
                deps.into_iter().collect()
              }
            } else {
              vec![]
            };

            if let Some(resolve_dependencies) =
              module_preload.options().and_then(|v| v.resolve_dependencies.as_ref())
              && let Some(normalized_file) = normalized_file
            {
              // We can't let the user remove css deps as these aren't really preloads, they are just using
              // the same mechanism as module preloads for this chunk
              let mut css_deps = vec![];
              let mut other_deps = vec![];
              for dep in deps_arr.drain(..) {
                if dep.ends_with(".css") {
                  css_deps.push(dep);
                } else {
                  other_deps.push(dep);
                }
              }
              deps_arr.clear();
              deps_arr.extend(
                resolve_dependencies(&normalized_file, other_deps, &chunk.filename, "js").await?,
              );
              deps_arr.extend(css_deps);
            }

            let mut render_deps = Vec::with_capacity(deps_arr.len());
            if render_built_url.is_some() {
              let env = ToOutputFilePathEnv {
                is_ssr,
                host_id: &chunk.filename,
                url_base,
                decoded_base,
                render_built_url: render_built_url.as_deref(),
              };
              for dep in deps_arr {
                let result = env
                  .to_output_file_path(&dep, "js", false, |filename: &Path, importer: &Path| {
                    let path = filename.relative(importer.parent().unwrap());
                    let file = path.to_slash_lossy();
                    if file.starts_with('.') {
                      AssetUrlResult::WithoutRuntime(file.into_owned())
                    } else {
                      AssetUrlResult::WithoutRuntime(format!("./{file}"))
                    }
                  })
                  .await?;
                render_deps.push(match result {
                  AssetUrlResult::WithRuntime(s) => file_deps.add_file_deps(s, true),
                  AssetUrlResult::WithoutRuntime(s) => file_deps.add_file_deps(s, false),
                });
              }
            } else {
              for dep in deps_arr {
                // Don't include the assets dir if the default asset file names
                // are used, the path will be reconstructed by the import preload helper
                render_deps.push(if self.is_relative_base {
                  let path = dep.relative(chunk.filename.as_path().parent().unwrap());
                  let file = path.to_slash_lossy();
                  file_deps.add_file_deps(
                    if file.starts_with('.') { file.into_owned() } else { format!("./{file}") },
                    false,
                  )
                } else {
                  file_deps.add_file_deps(dep, false)
                });
              }
            }

            #[expect(clippy::cast_possible_truncation)]
            s.update(
              marker_start as u32,
              (marker_start + 16) as u32, // __VITE_PRELOAD__
              if render_deps.is_empty() {
                "[]".to_string()
              } else {
                format!(
                  "__vite__mapDeps([{}])",
                  render_deps.into_iter().map(|u| u.to_string()).join(",")
                )
              },
            )
            .expect("update should not fail in build import analysis plugin");

            rewrote_marker_start_pos.insert(marker_start);
          }
        }

        if !file_deps.0.is_empty() {
          let map_deps_code = format!(
            "const __vite__mapDeps=(i,m=__vite__mapDeps,d=(m.f||(m.f=[{}])))=>i.map(i=>d[i]);\n",
            file_deps
              .0
              .into_iter()
              .map(|(s, is_runtime)| if is_runtime { s } else { to_string_literal(&s) })
              .join(",")
          );
          // inject extra code at the top or next line of hashbang
          if chunk.code.starts_with("#!") {
            #[expect(clippy::cast_possible_truncation)]
            s.prepend_left(
              chunk.code.find('\n').map(|pos| pos + 1).unwrap_or_default() as u32,
              map_deps_code,
            );
          } else {
            s.prepend(map_deps_code);
          }
        }

        // there may still be markers due to inlined dynamic imports, remove
        // all the markers regardless
        for (start, _) in chunk.code.match_indices("__VITE_PRELOAD__") {
          if !rewrote_marker_start_pos.contains(&start) {
            #[expect(clippy::cast_possible_truncation)]
            s.update(start as u32, (start + 16) as u32, "void 0")
              .expect("update should not fail in build import analysis plugin");
          }
        }

        if s.has_changed() {
          *chunk = Arc::new(rolldown_common::OutputChunk {
            code: s.to_string(),
            ..chunk.as_ref().clone()
          });
          // TODO: update sourcemap
        }
      }
    }

    Ok(())
  }
}
