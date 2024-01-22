use oxc::span::{GetSpan, Span};
use rolldown_common::ExportsKind;

use crate::bundler::module::Module;

use super::{AstRenderer, RenderControl};

impl<'r> AstRenderer<'r> {
  pub fn render_export_named_declaration_for_esm(
    &mut self,
    named_decl: &oxc::ast::ast::ExportNamedDeclaration,
  ) -> RenderControl {
    if let Some(decl) = &named_decl.declaration {
      self.ctx.remove_node(Span::new(named_decl.span.start, decl.span().start));
      RenderControl::Continue
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
      RenderControl::Skip
    } else {
      self.ctx.remove_node(named_decl.span);
      RenderControl::Skip
    }
  }
}
