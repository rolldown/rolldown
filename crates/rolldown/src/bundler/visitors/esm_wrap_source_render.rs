use oxc::{
  ast::{ast::Declaration, Visit},
  span::{Atom, GetSpan, Span},
};
use rolldown_oxc::BindingIdentifierExt;

use super::RendererContext;

pub struct EsmWrapSourceRender<'ast> {
  ctx: RendererContext<'ast>,
  hoisted_vars: Vec<Atom>,
  hoisted_functions: Vec<Span>,
}

impl<'ast> EsmWrapSourceRender<'ast> {
  pub fn new(ctx: RendererContext<'ast>) -> Self {
    Self { ctx, hoisted_vars: vec![], hoisted_functions: vec![] }
  }

  pub fn apply(&mut self) {
    let program = self.ctx.module.ast.program();
    self.visit_program(program);
    self.hoisted_functions.iter().for_each(|f| {
      // TODO: remove this hack
      // here move end of function to the keep "\n"
      self.ctx.source.relocate(f.start, f.end + 1, 0);
    });
    self.ctx.source.append_right(0, format!("var {};\n", self.hoisted_vars.join(",")));

    let namespace_name = self.ctx.namespace_symbol_name.unwrap();
    let exports: String = self
      .ctx
      .module
      .resolved_exports
      .iter()
      .map(|(exported_name, info)| {
        let canonical_ref = self.ctx.symbols.par_get_canonical_ref(info.local_symbol);
        let canonical_name = self.ctx.final_names.get(&canonical_ref).unwrap();
        format!("  get {exported_name}() {{ return {canonical_name} }}",)
      })
      .collect::<Vec<_>>()
      .join(",\n");
    self.ctx.source.append_right(0, format!("var {namespace_name} = {{\n{exports}\n}};\n",));

    let wrap_symbol_name = self.ctx.wrap_symbol_name.unwrap();
    let esm_runtime_symbol_name = self.ctx.get_runtime_symbol_final_name(&"__esm".into());
    self.ctx.source.append_right(
      0,
      format!(
        "var {wrap_symbol_name} = {esm_runtime_symbol_name}({{\n'{}'() {{\n",
        self.ctx.module.resource_id.prettify(),
      ),
    );
    self.ctx.source.append("\n}\n});");
  }

  fn hoisted_function(&mut self, func: &'ast oxc::ast::ast::Function<'ast>) {
    // deconflict function name
    if let Some(id) = &func.id {
      let name =
        self.ctx.get_symbol_final_name((self.ctx.module.id, id.expect_symbol_id()).into()).unwrap();
      self.ctx.overwrite(id.span.start, id.span.end, name.to_string());
    }
    self.hoisted_functions.push(func.span);
  }
}

impl<'ast> Visit<'ast> for EsmWrapSourceRender<'ast> {
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
      self.visit_declaration(decl);
      match decl {
        Declaration::VariableDeclaration(var_decl) => {
          let names = var_decl
            .declarations
            .iter()
            .filter_map(|decl| match &decl.id.kind {
              oxc::ast::ast::BindingPatternKind::BindingIdentifier(id) => self
                .ctx
                .get_symbol_final_name((self.ctx.module.id, id.symbol_id.get().unwrap()).into()),
              _ => unimplemented!(),
            })
            .cloned();
          self.hoisted_vars.extend(names);
          self
            .ctx
            .remove_node(Span::new(named_decl.span.start, var_decl.declarations[0].span.start));
        }
        Declaration::FunctionDeclaration(func) => {
          self.ctx.remove_node(Span::new(named_decl.span.start, func.span.start));
          self.hoisted_function(func);
        }
        Declaration::ClassDeclaration(class) => {
          let id = class.id.as_ref().unwrap();
          if let Some(name) =
            self.ctx.get_symbol_final_name((self.ctx.module.id, id.expect_symbol_id()).into())
          {
            self.hoisted_vars.push(name.clone());
            self.ctx.overwrite(named_decl.span.start, class.span.start, format!("{name} = "));
          }
        }
        _ => {}
      }
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
        let default_symbol_name = self.ctx.default_symbol_name.unwrap();
        self.hoisted_vars.push(default_symbol_name.clone());
        self.ctx.overwrite(decl.span.start, exp.span().start, format!("{default_symbol_name} = "));
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
        self.ctx.remove_node(Span::new(decl.span.start, func.span.start));
        self.hoisted_function(func);
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
        let default_symbol_name = self.ctx.default_symbol_name.unwrap();
        self.hoisted_vars.push(default_symbol_name.clone());
        self.ctx.overwrite(decl.span.start, class.span.start, format!("{default_symbol_name} = "));
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
