mod ast_utils;
mod ast_visit;

use std::borrow::Cow;

use arcstr::ArcStr;
use oxc::ast_visit::VisitMut;
use rolldown_common::side_effects::HookSideEffects;
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookTransformAstArgs, HookTransformAstReturn, HookUsage, Plugin,
  PluginContext,
};

use self::ast_visit::BuildImportAnalysisVisitor;

const PRELOAD_HELPER_ID: &str = "\0vite/preload-helper.js";

#[derive(Debug)]
#[expect(clippy::struct_excessive_bools)]
pub struct BuildImportAnalysisPlugin {
  pub preload_code: ArcStr,
  pub insert_preload: bool,
  pub render_built_url: bool,
  pub is_relative_base: bool,
  pub is_test_v2: bool,
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
    _ctx: &PluginContext,
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
      );
      visitor.visit_program(fields.program);
    });
    Ok(ast)
  }

  async fn generate_bundle(
    &self,
    _ctx: &PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if !args.options.format.is_esm() {
      return Ok(());
    }
    todo!()
  }

  fn register_hook_usage(&self) -> HookUsage {
    if self.is_test_v2 {
      HookUsage::ResolveId | HookUsage::Load | HookUsage::TransformAst | HookUsage::GenerateBundle
    } else {
      HookUsage::ResolveId | HookUsage::Load | HookUsage::TransformAst
    }
  }
}
