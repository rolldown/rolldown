mod impl_visit;
mod render_cjs;
mod render_esm;
mod render_wrapped_esm;
mod utils;
use std::fmt::Debug;

use oxc::{
  ast::Visit,
  span::{Atom, GetSpan, Span},
};
use rolldown_common::{ExportsKind, SymbolRef, WrapKind};
use rolldown_oxc::BindingIdentifierExt;
use rolldown_utils::MagicStringExt;
use rustc_hash::FxHashMap;
use string_wizard::MagicString;

use super::{
  chunk_graph::ChunkGraph,
  linker::linker_info::LinkingInfo,
  module::{Module, NormalModule},
  stages::link_stage::LinkStageOutput,
};

#[derive(Debug)]
pub struct AstRenderContext<'r> {
  pub graph: &'r LinkStageOutput,
  pub module: &'r NormalModule,
  pub linking_info: &'r LinkingInfo,
  pub canonical_names: &'r FxHashMap<SymbolRef, Atom>,
  pub source: &'r mut MagicString<'static>,
  pub chunk_graph: &'r ChunkGraph,
  pub wrap_ref_name: Option<&'r Atom>,
  pub default_ref_name: Option<&'r Atom>,
  // Used to hoisted declaration for import module, including import declaration and export declaration which has source imported
  pub first_stmt_start: Option<u32>,
}

impl<'r> AstRenderContext<'r> {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    graph: &'r LinkStageOutput,
    canonical_names: &'r FxHashMap<SymbolRef, Atom>,
    source: &'r mut MagicString<'static>,
    chunk_graph: &'r ChunkGraph,
    module: &'r NormalModule,
    linking_info: &'r LinkingInfo,
  ) -> Self {
    let wrap_symbol_name =
      linking_info.wrap_ref.map(|s| graph.symbols.canonical_name_for(s, canonical_names));
    let default_symbol_name = module
      .default_export_symbol
      .map(|s| graph.symbols.canonical_name_for((module.id, s).into(), canonical_names));
    Self {
      graph,
      canonical_names,
      source,
      chunk_graph,
      module,
      linking_info,
      wrap_ref_name: wrap_symbol_name,
      default_ref_name: default_symbol_name,
      first_stmt_start: None,
    }
  }
}

#[derive(Debug, Default)]
pub struct WrappedEsmCtx {
  pub hoisted_vars: Vec<Atom>,
  pub hoisted_functions: Vec<Span>,
}

#[derive(Debug)]
pub enum RenderKind {
  WrappedEsm,
  Cjs,
  Esm,
}

impl RenderKind {
  pub fn from_wrap_kind(kind: &WrapKind) -> Self {
    match kind {
      WrapKind::None => Self::Esm,
      WrapKind::Cjs => Self::Cjs,
      WrapKind::Esm => Self::WrappedEsm,
    }
  }
}

#[derive(Debug)]
pub struct AstRenderer<'r> {
  ctx: AstRenderContext<'r>,
  wrapped_esm_ctx: WrappedEsmCtx,
  kind: RenderKind,
  indentor: String,
}

impl<'r> AstRenderer<'r> {
  pub fn new(ctx: AstRenderContext<'r>, kind: RenderKind) -> Self {
    Self {
      kind,
      indentor: ctx.source.guessed_indentor().to_string(),
      ctx,
      wrapped_esm_ctx: WrappedEsmCtx::default(),
    }
  }
}

impl<'r> AstRenderer<'r> {
  pub fn render(&mut self) {
    let program = self.ctx.module.ast.program();
    self.visit_program(program);

    match &mut self.kind {
      RenderKind::WrappedEsm => {
        let mut indent_excludes = vec![];
        self.wrapped_esm_ctx.hoisted_functions.iter().for_each(|f| {
          self.ctx.source.relocate(f.start, f.end, 0);
          self.ctx.source.append_left(f.end, "\n");
          indent_excludes.push([f.start, f.end]);
        });
        self.ctx.source.indent2(&self.indentor, indent_excludes);
        if !self.wrapped_esm_ctx.hoisted_vars.is_empty() {
          self
            .ctx
            .source
            .append_right(0, format!("var {};\n", self.wrapped_esm_ctx.hoisted_vars.join(",")));
        }

        if let Some(s) = self.generate_namespace_variable_declaration() {
          self.ctx.source.append_right(0, s);
        }

        let wrap_ref_name = self.ctx.wrap_ref_name.unwrap();
        let esm_ref_name = self.ctx.canonical_name_for_runtime("__esm");
        self.ctx.source.append_right(
          0,
          format!(
            "var {wrap_ref_name} = {esm_ref_name}({{\n{}'{}'() {{\n",
            self.indentor, self.ctx.module.pretty_path,
          ),
        );
        self.ctx.source.append(format!("\n{}}}\n}});", self.indentor));
      }
      RenderKind::Cjs => {
        let wrap_ref_name = self.ctx.wrap_ref_name.unwrap();
        let prettify_id = &self.ctx.module.pretty_path;
        self.ctx.source.indent2(&self.indentor, Vec::default());
        let commonjs_ref_name = self.ctx.canonical_name_for_runtime("__commonJS");
        self.ctx.source.prepend(format!(
          "var {wrap_ref_name} = {commonjs_ref_name}({{\n{}'{prettify_id}'(exports, module) {{\n",
          self.indentor,
        ));
        self.ctx.source.append(format!("\n{}}}\n}});", self.indentor));
        assert!(!self.ctx.module.is_namespace_referenced());
      }
      RenderKind::Esm => {
        if let Some(s) = self.generate_namespace_variable_declaration() {
          self.ctx.source.prepend(s);
        }
      }
    }
  }

  fn strip_export_keyword(
    &mut self,
    default_decl: &oxc::ast::ast::ExportDefaultDeclaration,
  ) -> RenderControl {
    match &default_decl.declaration {
      oxc::ast::ast::ExportDefaultDeclarationKind::Expression(exp) => {
        let default_ref_name = self.ctx.default_ref_name.expect("Should generated a name");
        self.ctx.source.overwrite(
          default_decl.span.start,
          exp.span().start,
          format!("var {default_ref_name} = "),
        );
        self.visit_expression(exp);
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(decl) => {
        self.ctx.remove_node(Span::new(default_decl.span.start, decl.span.start));
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(decl) => {
        self.ctx.remove_node(Span::new(default_decl.span.start, decl.span.start));
      }
      _ => unreachable!("TypeScript code should be preprocessed"),
    }
    RenderControl::Skip
  }

  fn render_require_expr(&mut self, expr: &oxc::ast::ast::CallExpression) {
    let Module::Normal(importee) = self.ctx.importee_by_span(expr.span) else {
      return;
    };
    let importee_linking_info = &self.ctx.graph.linking_infos[importee.id];
    let wrap_ref_name = self.canonical_name_for(importee_linking_info.wrap_ref.unwrap());
    if importee.exports_kind == ExportsKind::CommonJs {
      self.ctx.source.overwrite(expr.span.start, expr.span.end, format!("{wrap_ref_name}()"));
    } else {
      let ns_name = self.canonical_name_for(importee.namespace_symbol);
      let to_commonjs_ref_name = self.canonical_name_for_runtime("__toCommonJS");
      self.ctx.source.overwrite(
        expr.span.start,
        expr.span.end,
        format!("({wrap_ref_name}(), {to_commonjs_ref_name}({ns_name}))"),
      );
    }
  }

  // TODO(hyf): need to investigate this logic again. https://github.com/rolldown-rs/rolldown/pull/144
  /// Rewrite statement `require('./foo.js')` to `init_foo()`
  fn try_render_require_statement(&mut self, stmt: &oxc::ast::ast::Statement) -> RenderControl {
    // only direct call is result unused eg `init()`
    let oxc::ast::ast::Statement::ExpressionStatement(expr) = stmt else {
      return RenderControl::Continue;
    };
    let oxc::ast::ast::Expression::CallExpression(call_exp) = &expr.expression else {
      return RenderControl::Continue;
    };
    if self.is_global_require(&call_exp.callee) {
      let Module::Normal(importee) = self.ctx.importee_by_span(call_exp.span) else {
        return RenderControl::Continue;
      };
      let importee_linking_info = &self.ctx.graph.linking_infos[importee.id];
      let wrap_ref_name = self.canonical_name_for(importee_linking_info.wrap_ref.unwrap());
      self.ctx.source.update(call_exp.span.start, call_exp.span.end, format!("{wrap_ref_name}()"));
      RenderControl::Skip
    } else {
      RenderControl::Continue
    }
  }

  fn render_binding_identifier(&mut self, ident: &oxc::ast::ast::BindingIdentifier) {
    self.rewrite_symbol(
      (self.ctx.module.id, ident.expect_symbol_id()).into(),
      &ident.name,
      ident.span,
      false,
    );
  }

  fn render_identifier_reference(
    &mut self,
    ident: &'_ oxc::ast::ast::IdentifierReference,
    is_callee: bool,
  ) {
    let Some(symbol_id) = self.ctx.module.scope.symbol_id_for(ident.reference_id.get().unwrap())
    else {
      // This is global identifier references, eg `console.log`. We don't need to rewrite it.
      return;
    };
    self.rewrite_symbol((self.ctx.module.id, symbol_id).into(), &ident.name, ident.span, is_callee);
  }
}

pub enum RenderControl {
  Continue,
  Skip,
}

impl RenderControl {
  pub fn _is_continue(&self) -> bool {
    matches!(self, Self::Continue)
  }

  pub fn is_skip(&self) -> bool {
    matches!(self, Self::Skip)
  }
}
