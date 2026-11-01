use oxc::{
  allocator::{Allocator, TakeIn},
  ast::{
    ast::{self, Expression, Statement},
    builder::AstBuilder,
  },
  ast_visit::{VisitJsMut, walk_js_mut},
  semantic::Scoping,
  span::SPAN,
  syntax::symbol::SymbolFlags,
};
use rustc_hash::FxHashSet;

// An internal AST intrinsic, never an authored identifier or emitted runtime call.
const TEMPORAL_READ: &str = "\0native_temporal_read";

pub fn preserve_temporal_reads<'ast>(
  program: &mut ast::Program<'ast>,
  scoping: &Scoping,
  allocator: &'ast Allocator,
) -> bool {
  let mut symbols: FxHashSet<_> = scoping
    .symbol_ids()
    .filter(|&symbol| {
      scoping.symbol_scope_id(symbol) == scoping.root_scope_id()
        && scoping
          .symbol_flags(symbol)
          .intersects(SymbolFlags::BlockScopedVariable | SymbolFlags::Class | SymbolFlags::Import)
    })
    .collect();
  for statement in &program.body {
    if let Statement::ImportDeclaration(import) = statement
      && let Some(specifiers) = &import.specifiers
    {
      for specifier in specifiers {
        if let ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(namespace) = specifier {
          symbols.remove(&namespace.local.symbol_id());
        }
      }
    }
  }
  struct Preserve<'a, 'ast> {
    scoping: &'a Scoping,
    symbols: FxHashSet<oxc::semantic::SymbolId>,
    builder: AstBuilder<'ast>,
    changed: bool,
  }
  impl<'ast> VisitJsMut<'ast> for Preserve<'_, 'ast> {
    fn visit_expression(&mut self, expression: &mut Expression<'ast>) {
      walk_js_mut::walk_expression(self, expression);
      let Expression::Identifier(identifier) = expression else {
        return;
      };
      let Some(symbol) = identifier
        .reference_id
        .get()
        .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
      else {
        return;
      };
      if !self.symbols.contains(&symbol) {
        return;
      }
      let value = expression.take_in(&self.builder);
      *expression = Expression::new_call_expression(
        SPAN,
        Expression::new_identifier(SPAN, TEMPORAL_READ, &self.builder),
        None,
        [ast::Argument::from(value)],
        false,
        &self.builder,
      );
      self.changed = true;
    }
  }
  let mut preserve = Preserve {
    scoping,
    symbols,
    builder: AstBuilder::new(allocator),
    changed: false,
  };
  preserve.visit_program(program);
  preserve.changed
}

pub fn unwrap_temporal_read(expression: &mut Expression<'_>) {
  if let Expression::CallExpression(call) = expression
    && matches!(&call.callee, Expression::Identifier(identifier) if identifier.name == TEMPORAL_READ)
  {
    *expression = call
      .arguments
      .pop()
      .expect("temporal read argument")
      .into_expression();
  }
}
