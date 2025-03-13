use oxc::{
  allocator::Allocator,
  ast::{AstKind, ast::Program},
  ast_visit::{Visit, walk::walk_program},
  parser::{ParseOptions, Parser},
  semantic::{Semantic, SemanticBuilder, SymbolId},
  span::{SourceType, Span},
};

pub fn extract_toplevel_item_span(
  source: &str,
  path: &str,
  toplevel_item_name: &str,
) -> Option<Span> {
  let allocator = Allocator::default();
  let source_type = SourceType::from_path(path).unwrap();
  let parser = Parser::new(&allocator, source, source_type)
    .with_options(ParseOptions { allow_return_outside_function: true, ..ParseOptions::default() });
  let ret = parser.parse();

  let semantic =
    SemanticBuilder::new().with_scope_tree_child_ids(true).build(&ret.program).semantic;
  let mut visitor = ExtractTargetSpan::new(toplevel_item_name, &semantic);
  visitor.visit_program(&ret.program);
  visitor.ret_span
}

struct ExtractTargetSpan<'a> {
  ret_span: Option<Span>,
  visit_path: Vec<AstKind<'a>>,
  symbol_id: Option<SymbolId>,
}

impl<'a> ExtractTargetSpan<'a> {
  fn new(toplevel_item_name: &str, semantic: &Semantic<'a>) -> Self {
    let symbol_id =
      semantic.scoping().find_binding(semantic.scoping().root_scope_id(), toplevel_item_name);
    Self { ret_span: None, visit_path: vec![], symbol_id }
  }
}

impl<'a> Visit<'a> for ExtractTargetSpan<'a> {
  fn visit_program(&mut self, it: &Program<'a>) {
    if self.symbol_id.is_none() {
      return;
    }
    walk_program(self, it);
  }
  fn enter_node(&mut self, kind: AstKind<'a>) {
    self.visit_path.push(kind);
  }

  fn leave_node(&mut self, _: AstKind<'_>) {
    self.visit_path.pop();
  }

  fn visit_binding_identifier(&mut self, it: &oxc::ast::ast::BindingIdentifier<'a>) {
    let symbol_id = it.symbol_id();
    if symbol_id == self.symbol_id.unwrap() {
      for p in self.visit_path.iter().rev() {
        match p {
          AstKind::VariableDeclaration(decl) => {
            self.ret_span = Some(decl.span);
            break;
          }
          _ => {}
        }
      }
    }
  }
}
