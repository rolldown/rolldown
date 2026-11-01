use oxc::{
  allocator::{self, TakeIn},
  ast::ast::{self, Expression, Statement},
  ast_visit::{VisitJs, VisitJsMut, walk_js_mut},
  span::SPAN,
};
use rolldown_common::EcmaModuleAstUsage;
use rolldown_ecmascript_utils::StatementFactoryExt;
use rustc_hash::FxHashSet;

use super::ScopeHoistingFinalizer;

const ASYNC_BODY: &str = "\0native_async_body";

pub(super) fn is_async_body_marker(statement: &Statement<'_>) -> bool {
  matches!(statement, Statement::ExpressionStatement(statement)
    if matches!(&statement.expression, Expression::Identifier(id) if id.name == ASYNC_BODY))
}

impl<'ast> ScopeHoistingFinalizer<'_, 'ast> {
  pub(super) fn async_body_marker(&self) -> Statement<'ast> {
    Statement::new_expression_statement(
      SPAN,
      Expression::new_identifier(SPAN, ASYNC_BODY, self),
      self,
    )
  }

  pub(super) fn async_pending_name(&self, program: &ast::Program<'ast>) -> String {
    struct Names(FxHashSet<String>);
    impl<'ast> VisitJs<'ast> for Names {
      fn visit_binding_identifier(&mut self, id: &ast::BindingIdentifier<'ast>) {
        self.0.insert(id.name.to_string());
      }
      fn visit_identifier_reference(&mut self, id: &ast::IdentifierReference<'ast>) {
        self.0.insert(id.name.to_string());
      }
    }
    let mut names = Names(
      self
        .ctx
        .chunk
        .canonical_names
        .values()
        .map(ToString::to_string)
        .collect(),
    );
    names.visit_program(program);
    for index in 0.. {
      let name = format!("__asyncDependencies{index}");
      if !names.0.contains(&name) {
        return name;
      }
    }
    unreachable!()
  }

  pub(super) fn schedule_async_body(
    &self,
    mut imports: allocator::Vec<'ast, Statement<'ast>>,
    body: allocator::Vec<'ast, Statement<'ast>>,
    pending: &str,
  ) -> allocator::Vec<'ast, Statement<'ast>> {
    let pending = ast::Str::from_str_in(pending, self);
    struct Start<'a, 'me, 'ast> {
      finalizer: &'a ScopeHoistingFinalizer<'me, 'ast>,
      pending: ast::Str<'ast>,
    }
    impl<'ast> VisitJsMut<'ast> for Start<'_, '_, 'ast> {
      fn visit_expression(&mut self, expression: &mut Expression<'ast>) {
        walk_js_mut::walk_expression(self, expression);
        if let Expression::AwaitExpression(awaited) = expression {
          let result = awaited.argument.take_in(self.finalizer);
          *expression = Expression::new_call_expression(
            SPAN,
            self
              .finalizer
              .finalized_expr_for_runtime_symbol("__startAsync"),
            None,
            [
              ast::Argument::from(Expression::new_identifier(
                SPAN,
                self.pending,
                self.finalizer,
              )),
              ast::Argument::from(result),
            ],
            false,
            self.finalizer,
          );
        }
      }
    }
    let mut start = Start {
      finalizer: self,
      pending,
    };
    for statement in &mut imports {
      start.visit_statement(statement);
    }
    imports.insert(
      0,
      Statement::new_var_decl(
        &pending,
        Expression::new_array_expression(SPAN, [], self),
        self,
      ),
    );
    let execute = Expression::new_arrow_function_expression(
      SPAN,
      self
        .ctx
        .module
        .ast_usage
        .contains(EcmaModuleAstUsage::TopLevelAwait),
      None,
      ast::FormalParameters::boxed(
        SPAN,
        ast::FormalParameterKind::ArrowFormalParameters,
        [],
        None,
        self,
      ),
      None,
      ast::ArrowFunctionBody::FunctionBody(ast::FunctionBody::boxed(SPAN, [], body, self)),
      self,
    );
    imports.push(Statement::new_return_statement(
      SPAN,
      Some(Expression::new_call_expression(
        SPAN,
        self.finalized_expr_for_runtime_symbol("__finishAsync"),
        None,
        [
          ast::Argument::from(Expression::new_identifier(SPAN, pending, self)),
          ast::Argument::from(execute),
        ],
        false,
        self,
      )),
      self,
    ));
    imports
  }
}
