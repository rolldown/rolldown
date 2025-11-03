mod ast_utils;
mod ast_visit;
mod utils;

use std::{borrow::Cow, path::PathBuf, sync::Arc};

use arcstr::ArcStr;
use oxc::ast_visit::VisitMut;
use rolldown_common::{Output, side_effects::HookSideEffects};
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookTransformAstArgs, HookTransformAstReturn, HookUsage, Plugin,
  PluginContext,
};
use rolldown_plugin_utils::constants::RemovedPureCSSFilesCache;
use rustc_hash::{FxHashMap, FxHashSet};
use sugar_path::SugarPath as _;

use crate::{
  ast_visit::{DynamicImportCollectVisitor, DynamicImportVisitor},
  utils::AddDeps,
};

use self::ast_visit::BuildImportAnalysisVisitor;

const PRELOAD_HELPER_ID: &str = "\0vite/preload-helper.js";

#[derive(derive_more::Debug)]
#[expect(clippy::struct_excessive_bools)]
pub struct BuildImportAnalysisPlugin {
  pub preload_code: ArcStr,
  pub insert_preload: bool,
  pub render_built_url: bool,
  pub is_relative_base: bool,
  pub is_test_v2: bool,
  // pub sourcemap: bool,
  // pub is_module_preload: bool,
  // #[debug(skip)]
  // pub resolve_dependencies: Option<Arc<ResolveDependenciesFn>>,
}

impl Plugin for BuildImportAnalysisPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:build-import-analysis")
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

  async fn load(&self, _ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
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

  async fn generate_bundle(
    &self,
    ctx: &PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if !args.options.format.is_esm() {
      return Ok(());
    }
    if !self.insert_preload {
      if let Some(removed_pure_css_files) = ctx.meta().get::<RemovedPureCSSFilesCache>()
        && !removed_pure_css_files.inner.is_empty()
      {
        let mut bundle_iter = args.bundle.iter_mut();
        while let Some(Output::Chunk(chunk)) = bundle_iter.next() {
          // TODO: Maybe we should use `chunk.dynamicImports`?
          if utils::DYNAMIC_IMPORT_RE.is_match(&chunk.code) {
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

    let mut bundle_iter = args.bundle.iter_mut();
    // can't use chunk.dynamicImports.length here since some modules e.g.
    // dynamic import to constant json may get inlined.
    while let Some(Output::Chunk(chunk)) = bundle_iter.next()
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

      let mut s = string_wizard::MagicString::new(&chunk.code);

      for import in imports {
        let mut deps = FxHashSet::default();
        let mut has_removed_pure_css_chunks = false;

        let _normalized_file = import.source.map(|url| {
          let file = PathBuf::from(chunk.filename.as_str());
          let file_dir = file.parent().unwrap();
          let normalized_file =
            file_dir.join(url.as_str()).normalize().to_string_lossy().into_owned();

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

        todo!()
      }
    }

    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    if self.is_test_v2 {
      HookUsage::ResolveId | HookUsage::Load | HookUsage::TransformAst | HookUsage::GenerateBundle
    } else {
      HookUsage::ResolveId | HookUsage::Load | HookUsage::TransformAst
    }
  }
}
