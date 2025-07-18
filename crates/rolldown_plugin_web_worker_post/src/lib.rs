mod ast_visitor;

use std::borrow::Cow;

use oxc::ast_visit::VisitMut;
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_plugin::{HookUsage, Plugin};

use crate::ast_visitor::WebWorkerPostVisitor;

#[derive(Debug)]
pub struct WebWorkerPostPlugin;

impl Plugin for WebWorkerPostPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:web-worker-post")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::TransformAst
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
}
