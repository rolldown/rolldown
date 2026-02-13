mod ast_visitor;

use std::borrow::Cow;

use oxc::ast_visit::VisitMut;
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_plugin::{Plugin, PluginHookMeta, PluginOrder, RegisterHook};

use crate::ast_visitor::WebWorkerPostVisitor;

#[derive(Debug)]
pub struct ViteWebWorkerPostPlugin;

#[RegisterHook]
impl Plugin for ViteWebWorkerPostPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-web-worker-post")
  }

  async fn transform_ast(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    mut args: rolldown_plugin::HookTransformAstArgs<'_>,
  ) -> rolldown_plugin::HookTransformAstReturn {
    args.ast.program.with_mut(|fields| {
      let ast_snippet = AstSnippet::new(fields.allocator);
      let mut visitor = WebWorkerPostVisitor::new(ast_snippet);
      visitor.visit_program(fields.program);
    });
    Ok(args.ast)
  }

  fn transform_ast_meta(&self) -> Option<PluginHookMeta> {
    Some(PluginHookMeta { order: Some(PluginOrder::Post) })
  }
}
