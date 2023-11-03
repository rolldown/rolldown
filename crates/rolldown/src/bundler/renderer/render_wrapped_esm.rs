use crate::bundler::module::Module;

use super::{AstRenderer, RenderControl, RenderKind};
use oxc::{
  ast::ast::Declaration,
  span::{GetSpan, Span},
};
use rolldown_common::ExportsKind;
use rolldown_oxc::BindingIdentifierExt;
impl<'r> AstRenderer<'r> {
  pub fn render_export_default_declaration_for_wrapped_esm(
    &mut self,
    decl: &oxc::ast::ast::ExportDefaultDeclaration,
  ) -> RenderControl {
    let RenderKind::WrappedEsm(info) = &mut self.kind else { unreachable!() };
    match &decl.declaration {
      oxc::ast::ast::ExportDefaultDeclarationKind::Expression(exp) => {
        let default_symbol_name = self.ctx.default_symbol_name.unwrap();
        info.hoisted_vars.push(default_symbol_name.clone());
        self.ctx.overwrite(decl.span.start, exp.span().start, format!("{default_symbol_name} = "));
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
        self.ctx.remove_node(Span::new(decl.span.start, func.span.start));
        if let Some(id) = &func.id {
          let name =
            self.ctx.canonical_name_for((self.ctx.module.id, id.expect_symbol_id()).into());
          if id.name != name {
            self.ctx.overwrite(id.span.start, id.span.end, name.to_string());
          }
        } else {
          let default_symbol_name = self.ctx.default_symbol_name.unwrap();
          self.ctx.source.append_right(func.params.span.start, format!(" {default_symbol_name}"));
        }
        info.hoisted_functions.push(func.span);
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
        let default_symbol_name = self.ctx.default_symbol_name.unwrap();
        info.hoisted_vars.push(default_symbol_name.clone());
        self.ctx.overwrite(decl.span.start, class.span.start, format!("{default_symbol_name} = "));
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
    let RenderKind::WrappedEsm(info) = &mut self.kind else { unreachable!() };
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
          info.hoisted_vars.extend(names);
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
            self.ctx.overwrite(id.span.start, id.span.end, name.to_string());
          }
          info.hoisted_functions.push(func.span);
        }
        Declaration::ClassDeclaration(class) => {
          let id = class.id.as_ref().unwrap();
          if let Some(name) =
            self.ctx.need_to_rename((self.ctx.module.id, id.expect_symbol_id()).into())
          {
            info.hoisted_vars.push(name.clone());
            self.ctx.overwrite(named_decl.span.start, class.span.start, format!("{name} = "));
            // avoid syntax error
            // export class Foo {} Foo.prop = 123 => var Foo = class Foo {} \n Foo.prop = 123
            self.ctx.source.append_right(class.span.end, "\n");
          }
        }
        _ => {}
      }
    } else if named_decl.source.is_some() {
      match self.ctx.get_importee_by_span(named_decl.span) {
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
