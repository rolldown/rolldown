use crate::bundler::module::Module;

use super::{AstRenderer, RenderControl};
use oxc::{
  ast::ast::Declaration,
  span::{Atom, GetSpan, Span},
};
use rolldown_common::ExportsKind;
use rolldown_oxc::BindingIdentifierExt;
use rolldown_utils::MagicStringExt;
impl<'r> AstRenderer<'r> {
  pub fn render_export_default_declaration_for_wrapped_esm(
    &mut self,
    decl: &oxc::ast::ast::ExportDefaultDeclaration,
  ) -> RenderControl {
    match &decl.declaration {
      oxc::ast::ast::ExportDefaultDeclarationKind::Expression(exp) => {
        let default_ref_name = self.ctx.default_ref_name.unwrap();
        self.wrapped_esm_ctx.hoisted_vars.push(default_ref_name.clone());
        self.ctx.source.overwrite(
          decl.span.start,
          exp.span().start,
          format!("{default_ref_name} = "),
        );
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
        self.ctx.remove_node(Span::new(decl.span.start, func.span.start));
        if func.id.is_none() {
          let default_symbol_name = self.ctx.default_ref_name.unwrap();
          self.ctx.source.append_right(func.params.span.start, format!(" {default_symbol_name}"));
        }
        self.hoisted_function_declaration(func);
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
        self.ctx.remove_node(Span::new(decl.span.start, class.span.start));
        let default_symbol_name = self.ctx.default_ref_name.unwrap();
        self.hoisted_class_declaration(class, default_symbol_name);
      }
      _ => {}
    }
    RenderControl::Continue
  }

  pub fn render_export_named_declaration_for_wrapped_esm(
    &mut self,
    named_decl: &oxc::ast::ast::ExportNamedDeclaration,
  ) -> RenderControl {
    if let Some(decl) = &named_decl.declaration {
      match decl {
        Declaration::VariableDeclaration(var_decl) => {
          self.hoisted_variable_declaration(&var_decl.declarations, named_decl.span.start);
        }
        Declaration::FunctionDeclaration(func) => {
          self.ctx.remove_node(Span::new(named_decl.span.start, func.span.start));
          self.hoisted_function_declaration(func);
        }
        Declaration::ClassDeclaration(class) => {
          self.ctx.remove_node(Span::new(named_decl.span.start, class.span.start));
          let id = class.id.as_ref().unwrap();
          let name =
            self.ctx.canonical_name_for((self.ctx.module.id, id.expect_symbol_id()).into());
          self.hoisted_class_declaration(class, name);
        }
        _ => {}
      }
      return RenderControl::Continue;
    } else if named_decl.source.is_some() {
      match self.ctx.importee_by_span(named_decl.span) {
        Module::Normal(importee) => {
          if importee.exports_kind == ExportsKind::CommonJs {
            self.ctx.hoisted_module_declaration(
              named_decl.span.start,
              self.ctx.generate_import_commonjs_module(
                &self.ctx.graph.linking_infos[importee.id],
                Some(named_decl.span),
              ),
            );
          }
        }
        Module::External(_) => {}
      }
      self.ctx.remove_node(named_decl.span);
    } else {
      self.ctx.remove_node(named_decl.span);
    }
    RenderControl::Skip
  }

  pub fn render_top_level_declaration_for_wrapped_esm(
    &mut self,
    decl: &oxc::ast::ast::Declaration,
  ) {
    match &decl {
      oxc::ast::ast::Declaration::VariableDeclaration(var_decl) => {
        self.hoisted_variable_declaration(&var_decl.declarations, var_decl.span.start);
      }
      oxc::ast::ast::Declaration::FunctionDeclaration(func) => {
        self.hoisted_function_declaration(func);
      }
      oxc::ast::ast::Declaration::ClassDeclaration(class) => {
        if let Some(id) = &class.id {
          let name =
            self.ctx.canonical_name_for((self.ctx.module.id, id.expect_symbol_id()).into());
          self.hoisted_class_declaration(class, name);
        }
      }
      _ => {}
    }
  }

  fn hoisted_variable_declaration<'a>(
    &mut self,
    declarations: &oxc::allocator::Vec<'a, oxc::ast::ast::VariableDeclarator<'a>>,
    decl_start: u32,
  ) {
    let names = declarations
      .iter()
      .map(|decl| match &decl.id.kind {
        oxc::ast::ast::BindingPatternKind::BindingIdentifier(id) => {
          self.ctx.canonical_name_for((self.ctx.module.id, id.symbol_id.get().unwrap()).into())
        }
        _ => unimplemented!(),
      })
      .cloned();
    self.wrapped_esm_ctx.hoisted_vars.extend(names);
    self.ctx.remove_node(Span::new(decl_start, declarations[0].span.start));
  }

  fn hoisted_function_declaration(&mut self, func: &oxc::ast::ast::Function) {
    // binding_identifier will rename at visit children
    self.wrapped_esm_ctx.hoisted_functions.push(func.span);
  }

  fn hoisted_class_declaration(&mut self, class: &oxc::ast::ast::Class, name: &Atom) {
    self.wrapped_esm_ctx.hoisted_vars.push(name.clone());
    self.ctx.source.append_left(class.span.start, format!("{name} = "));
    // avoid syntax error
    // export class Foo {} Foo.prop = 123 => var Foo = class Foo {} \n Foo.prop = 123
    self.ctx.source.append_right(class.span.end, "\n");
  }
}
