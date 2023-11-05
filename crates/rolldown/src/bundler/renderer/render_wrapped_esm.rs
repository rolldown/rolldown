use crate::bundler::module::Module;

use super::{AstRenderer, RenderControl};
use oxc::{
  ast::ast::Declaration,
  span::{GetSpan, Span},
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
        if let Some(ident) = &func.id {
          self.render_binding_identifier(ident);
        } else {
          let default_symbol_name = self.ctx.default_ref_name.unwrap();
          self.ctx.source.append_right(func.params.span.start, format!(" {default_symbol_name}"));
        }
        self.wrapped_esm_ctx.hoisted_functions.push(func.span);
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
        let default_symbol_name = self.ctx.default_ref_name.unwrap();
        self.wrapped_esm_ctx.hoisted_vars.push(default_symbol_name.clone());
        self.ctx.source.overwrite(
          decl.span.start,
          class.span.start,
          format!("{default_symbol_name} = "),
        );
        // avoid syntax error
        // export default class Foo {} Foo.prop = 123 => var Foo = class Foo {} \n Foo.prop = 123
        self.ctx.source.append_right(class.span.end, "\n");
      }
      _ => {}
    }
    RenderControl::Skip
  }

  pub fn render_export_named_declaration_for_wrapped_esm(
    &mut self,
    named_decl: &oxc::ast::ast::ExportNamedDeclaration,
  ) -> RenderControl {
    if let Some(decl) = &named_decl.declaration {
      match decl {
        Declaration::VariableDeclaration(var_decl) => {
          let names = var_decl
            .declarations
            .iter()
            .map(|decl| match &decl.id.kind {
              oxc::ast::ast::BindingPatternKind::BindingIdentifier(id) => self
                .ctx
                .canonical_name_for((self.ctx.module.id, id.symbol_id.get().unwrap()).into()),
              _ => unimplemented!(),
            })
            .cloned();
          self.wrapped_esm_ctx.hoisted_vars.extend(names);
          self
            .ctx
            .remove_node(Span::new(named_decl.span.start, var_decl.declarations[0].span.start));
        }
        Declaration::FunctionDeclaration(func) => {
          self.ctx.remove_node(Span::new(named_decl.span.start, func.span.start));
          let id = func.id.as_ref().unwrap();
          let name =
            self.ctx.canonical_name_for((self.ctx.module.id, id.expect_symbol_id()).into());
          if id.name != name {
            self.ctx.source.overwrite(id.span.start, id.span.end, name.to_string());
          }
          self.wrapped_esm_ctx.hoisted_functions.push(func.span);
        }
        Declaration::ClassDeclaration(class) => {
          let id = class.id.as_ref().unwrap();
          if let Some(name) =
            self.ctx.need_to_rename((self.ctx.module.id, id.expect_symbol_id()).into())
          {
            self.wrapped_esm_ctx.hoisted_vars.push(name.clone());
            self.ctx.source.overwrite(
              named_decl.span.start,
              class.span.start,
              format!("{name} = "),
            );
            // avoid syntax error
            // export class Foo {} Foo.prop = 123 => var Foo = class Foo {} \n Foo.prop = 123
            self.ctx.source.append_right(class.span.end, "\n");
          }
        }
        _ => {}
      }
    } else if named_decl.source.is_some() {
      match self.ctx.importee_by_span(named_decl.span) {
        Module::Normal(importee) => {
          if importee.exports_kind == ExportsKind::CommonJs {
            self.ctx.hoisted_module_declaration(
              named_decl.span.start,
              self.ctx.generate_import_commonjs_module(
                importee,
                &self.ctx.graph.linking_infos[importee.id],
                true,
              ),
            );
          }
        }
        Module::External(_) => {} // TODO
      }
      self.ctx.remove_node(named_decl.span);
    } else {
      self.ctx.remove_node(named_decl.span);
    }
    RenderControl::Skip
  }
}
