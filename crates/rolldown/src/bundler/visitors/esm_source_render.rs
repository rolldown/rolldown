use super::RendererContext;
use oxc::{
  ast::Visit,
  span::{GetSpan, Span},
};

pub struct EsmSourceRender<'ast> {
  ctx: RendererContext<'ast>,
}

impl<'ast> EsmSourceRender<'ast> {
  pub fn new(ctx: RendererContext<'ast>) -> Self {
    Self { ctx }
  }

  pub fn apply(&mut self) {
    let module = self.ctx.module;
    let program = module.ast.program();
    self.visit_program(program);

    if let Some(namespace_name) = self.ctx.namespace_symbol_name {
      let exports: String = module
        .resolved_exports
        .iter()
        .map(|(exported_name, info)| {
          let canonical_ref = self.ctx.symbols.par_get_canonical_ref(info.local_symbol);
          let canonical_name = self.ctx.final_names.get(&canonical_ref).unwrap();
          format!("  get {exported_name}() {{ return {canonical_name} }}",)
        })
        .collect::<Vec<_>>()
        .join(",\n");
      self.ctx.source.append(format!("\nvar {namespace_name} = {{\n{exports}\n}};\n",));
    }
  }
}

impl<'ast> Visit<'ast> for EsmSourceRender<'ast> {
  fn visit_binding_identifier(&mut self, ident: &'ast oxc::ast::ast::BindingIdentifier) {
    self.ctx.visit_binding_identifier(ident);
  }

  fn visit_identifier_reference(&mut self, ident: &'ast oxc::ast::ast::IdentifierReference) {
    self.ctx.visit_identifier_reference(ident);
  }

  fn visit_import_declaration(&mut self, decl: &'ast oxc::ast::ast::ImportDeclaration<'ast>) {
    self.ctx.visit_import_declaration(decl);
  }

  fn visit_export_named_declaration(
    &mut self,
    named_decl: &'ast oxc::ast::ast::ExportNamedDeclaration<'ast>,
  ) {
    if let Some(decl) = &named_decl.declaration {
      self.ctx.remove_node(Span::new(named_decl.span.start, decl.span().start));
      self.visit_declaration(decl);
    } else {
      self.ctx.remove_node(named_decl.span);
    }
  }

  fn visit_export_all_declaration(
    &mut self,
    decl: &'ast oxc::ast::ast::ExportAllDeclaration<'ast>,
  ) {
    self.ctx.visit_export_all_declaration(decl);
  }

  fn visit_export_default_declaration(
    &mut self,
    decl: &'ast oxc::ast::ast::ExportDefaultDeclaration<'ast>,
  ) {
    match &decl.declaration {
      oxc::ast::ast::ExportDefaultDeclarationKind::Expression(exp) => {
        if let Some(name) = self.ctx.default_symbol_name {
          self.ctx.overwrite(decl.span.start, exp.span().start, format!("var {name} = "));
        }
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(decl) => {
        self.ctx.remove_node(Span::new(decl.span.start, decl.span.start));
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(decl) => {
        self.ctx.remove_node(Span::new(decl.span.start, decl.span.start));
      }
      _ => {}
    }
  }

  fn visit_import_expression(&mut self, expr: &oxc::ast::ast::ImportExpression<'ast>) {
    self.ctx.visit_import_expression(expr);
  }

  fn visit_call_expression(&mut self, expr: &'ast oxc::ast::ast::CallExpression<'ast>) {
    self.ctx.visit_call_expression(expr);
    for arg in &expr.arguments {
      self.visit_argument(arg);
    }
    self.visit_expression(&expr.callee);
  }
}
