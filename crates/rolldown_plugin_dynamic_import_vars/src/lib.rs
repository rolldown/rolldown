mod ast_visit;
mod dynamic_import_to_glob;
mod parse_pattern;
mod utils;

use std::{borrow::Cow, pin::Pin, sync::Arc};

use ast_visit::DynamicImportVarsVisitConfig;
use derive_more::Debug;
use oxc::{ast::AstBuilder, ast_visit::VisitMut};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookTransformAstArgs, HookTransformAstReturn, HookUsage, Plugin,
  PluginContext,
};
use rolldown_utils::pattern_filter::StringOrRegex;

use crate::ast_visit::DynamicImportVarsVisit;

pub const DYNAMIC_IMPORT_HELPER: &str = "\0rolldown_dynamic_import_helper.js";

pub type ResolverFn = dyn Fn(&str, &str) -> Pin<Box<(dyn Future<Output = anyhow::Result<Option<String>>> + Send)>>
  + Send
  + Sync;

#[derive(Debug, Default)]
pub struct DynamicImportVarsPlugin {
  pub include: Vec<StringOrRegex>,
  pub exclude: Vec<StringOrRegex>,
  #[debug(skip)]
  pub resolver: Option<Arc<ResolverFn>>,
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
      code: include_str!("dynamic-import-helper.js").to_string(),
      ..Default::default()
    }))
  }

  async fn transform_ast(
    &self,
    _ctx: &PluginContext,
    mut args: HookTransformAstArgs<'_>,
  ) -> HookTransformAstReturn {
    let cwd = args.cwd.to_string_lossy();
    if self.filter(args.id, &cwd) {
      return Ok(args.ast);
    }

    let config = DynamicImportVarsVisitConfig::default();

    // TODO: Ignore if includes a marker like "/* @rolldown-ignore */"
    args.ast.program.with_mut(|fields| {
      let source_text = fields.program.source_text;
      let ast_builder = AstBuilder::new(fields.allocator);
      let mut visitor = DynamicImportVarsVisit { ast_builder, source_text, config };

      visitor.visit_program(fields.program);
      if visitor.config.need_helper {
        fields.program.body.push(visitor.import_helper());
      }
    });
    Ok(args.ast)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load | HookUsage::TransformAst
  }
}
