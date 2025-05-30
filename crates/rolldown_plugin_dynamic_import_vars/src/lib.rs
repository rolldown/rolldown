mod ast_visit;
mod dynamic_import_to_glob;

use std::{borrow::Cow, path::Path, pin::Pin, sync::Arc};

use ast_visit::DynamicImportVarsVisitConfig;
use derive_more::Debug;
use oxc::{ast::AstBuilder, ast_visit::VisitMut};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookTransformAstArgs, HookTransformAstReturn, HookUsage, Plugin,
  PluginContext,
};
use rolldown_utils::{
  futures::block_on_spawn_all,
  pattern_filter::{StringOrRegex, filter as pattern_filter},
};
use sugar_path::SugarPath;

pub const DYNAMIC_IMPORT_HELPER: &str = "\0rolldown_dynamic_import_helper.js";

pub type ResolverFn = dyn Fn(String, String) -> Pin<Box<(dyn Future<Output = anyhow::Result<Option<String>>> + Send)>>
  + Send
  + Sync;

#[derive(Debug, Default)]
pub struct DynamicImportVarsPlugin {
  pub include: Vec<StringOrRegex>,
  pub exclude: Vec<StringOrRegex>,
  #[debug(skip)]
  pub resolver: Option<Arc<ResolverFn>>,
}

impl DynamicImportVarsPlugin {
  fn filter(&self, id: &str, cwd: &Path) -> bool {
    if self.include.is_empty() && self.exclude.is_empty() {
      return true;
    }

    let exclude = (!self.exclude.is_empty()).then_some(self.exclude.as_slice());
    let include = (!self.include.is_empty()).then_some(self.include.as_slice());
    pattern_filter(exclude, include, id, &cwd.to_string_lossy()).inner()
  }
}

impl Plugin for DynamicImportVarsPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:dynamic-import-vars")
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    Ok((args.specifier == DYNAMIC_IMPORT_HELPER).then_some(HookResolveIdOutput {
      id: arcstr::literal!(DYNAMIC_IMPORT_HELPER),
      ..Default::default()
    }))
  }

  async fn load(&self, _ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    Ok((args.id == DYNAMIC_IMPORT_HELPER).then_some(HookLoadOutput {
      code: arcstr::literal!(include_str!("dynamic-import-helper.js")),
      ..Default::default()
    }))
  }

  async fn transform_ast(
    &self,
    _ctx: &PluginContext,
    mut args: HookTransformAstArgs<'_>,
  ) -> HookTransformAstReturn {
    if !self.filter(args.id, args.cwd) {
      return Ok(args.ast);
    }

    let mut config = None;

    // TODO: Ignore if includes a marker like "/* @rolldown-ignore */"
    args.ast.program.with_mut(|fields| {
      let source_text = fields.source.as_str();
      let ast_builder = AstBuilder::new(fields.allocator);
      let mut visitor = ast_visit::DynamicImportVarsVisit {
        ast_builder,
        source_text,
        config: DynamicImportVarsVisitConfig {
          async_enabled: self.resolver.is_some(),
          ..Default::default()
        },
      };

      visitor.visit_program(fields.program);

      if visitor.config.async_enabled && !visitor.config.async_imports.is_empty() {
        visitor.config.current = 0;
        config = Some(visitor.config);
      } else if visitor.config.need_helper {
        fields.program.body.push(visitor.import_helper());
      }
    });

    if let Some(mut config) = config {
      let resolver = self.resolver.as_ref().unwrap();
      let iter = config.async_imports.into_iter().map(async |source| {
        let importer = args.id.as_path().parent().unwrap();
        resolver(source.unwrap(), args.id.to_string()).await.ok()?.and_then(|id| {
          let id = id.relative(importer);
          let id = id.to_slash_lossy();
          if id.is_empty() {
            None
          } else if id.as_bytes()[0] != b'.' {
            Some(rolldown_utils::concat_string!("./", id))
          } else {
            Some(id.into_owned())
          }
        })
      });

      config.async_imports = block_on_spawn_all(iter).await;

      args.ast.program.with_mut(|fields| {
        let source_text = fields.source.as_str();
        let ast_builder = AstBuilder::new(fields.allocator);
        let mut visitor = ast_visit::DynamicImportVarsVisit { ast_builder, source_text, config };

        visitor.visit_program(fields.program);

        if visitor.config.need_helper {
          fields.program.body.push(visitor.import_helper());
        }
      });
    }

    Ok(args.ast)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load | HookUsage::TransformAst
  }
}
