use oxc::{
  ast::{ast::Declaration, Visit},
  formatter::{Formatter, FormatterOptions, Gen},
  span::{Atom, GetSpan, Span},
};
use rolldown_oxc::BindingIdentifierExt;

use super::RendererContext;

pub struct EsmWrapSourceRender<'ast> {
  ctx: RendererContext<'ast>,
  hoisted_vars: Vec<Atom>,
  hoisted_functions: Vec<String>,
}

impl<'ast> EsmWrapSourceRender<'ast> {
  pub fn new(ctx: RendererContext<'ast>) -> Self {
    Self { ctx, hoisted_vars: vec![], hoisted_functions: vec![] }
  }

  pub fn apply(&mut self) {
    let program = self.ctx.module.ast.program();
    self.visit_program(program);
    let namespace_name = self.ctx.namespace_symbol_name.unwrap();
    let wrap_symbol_name = self.ctx.wrap_symbol_name.unwrap();
    let esm_runtime_symbol_name = self.ctx.get_runtime_symbol_final_name(&"__esm".into());
    self.ctx.source.prepend(format!(
      "var {wrap_symbol_name} = {esm_runtime_symbol_name}({{\n'{}'() {{\n",
      self.ctx.module.resource_id.prettify(),
    ));
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
    self.ctx.source.append("\n}\n});");
    self.ctx.source.prepend(format!("\nvar {namespace_name} = {{\n{exports}\n}};\n",));
    self.ctx.source.prepend(format!("var {};\n", self.hoisted_vars.join(",")));
    self.ctx.source.prepend(format!("{}\n", self.hoisted_functions.join("\n")));
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
          // hoisted function declaration
          // TODO update symbol name with magic string move
          self.ctx.remove_node(Span::new(named_decl.span.start, named_decl.span.end));
          #[allow(clippy::eq_op)]
          let mut formatter =
            Formatter::new((func.span.end - func.span.end) as usize, FormatterOptions::default());
          func.gen(&mut formatter);
          self.hoisted_functions.push(formatter.into_code());
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
      oxc::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(fn_decl) => {
        // hoisted function declaration
        // TODO update symbol name with magic string move
        self.ctx.remove_node(decl.span);
        #[allow(clippy::eq_op)]
        let mut formatter = Formatter::new(
          (fn_decl.span.end - fn_decl.span.end) as usize,
          FormatterOptions::default(),
        );
        fn_decl.gen(&mut formatter);
        self.hoisted_functions.push(formatter.into_code());
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
}
