mod ast_utils;
mod ast_visit;

use std::borrow::Cow;

use arcstr::ArcStr;
use oxc::ast_visit::VisitMut;
use rolldown_common::side_effects::HookSideEffects;
use rolldown_ecmascript_utils::AstFactory;
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookTransformAstArgs, HookTransformAstReturn, HookUsage, Plugin,
  PluginContext, SharedLoadPluginContext,
};

use self::ast_visit::BuildImportAnalysisVisitor;

const PRELOAD_HELPER_ID: &str = "\0vite/preload-helper.js";

#[derive(derive_more::Debug)]
pub struct ViteBuildImportAnalysisPlugin {
  pub preload_code: ArcStr,
  pub insert_preload: bool,
  pub render_built_url: bool,
  pub is_relative_base: bool,
}

impl Plugin for ViteBuildImportAnalysisPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-build-import-analysis")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load | HookUsage::TransformAst
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
      let ast_factory = AstFactory::new(fields.allocator);
      let mut visitor = BuildImportAnalysisVisitor::new(
        ast_factory,
        self.insert_preload,
        self.render_built_url,
        self.is_relative_base,
        ctx.options().format.is_esm(),
      );
      visitor.visit_program(fields.program);
    });
    Ok(ast)
  }
}
