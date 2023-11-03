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
use rolldown_common::{SymbolRef, WrapKind};
use rolldown_oxc::BindingIdentifierExt;
use rustc_hash::FxHashMap;
use string_wizard::MagicString;

use super::{
  chunk_graph::ChunkGraph,
  graph::{graph::Graph, linker_info::LinkingInfo},
  module::{Module, NormalModule},
};

#[derive(Debug)]
pub struct AstRenderContext<'r> {
  pub graph: &'r Graph,
  pub module: &'r NormalModule,
  pub linking_info: &'r LinkingInfo,
  pub canonical_names: &'r FxHashMap<SymbolRef, Atom>,
  pub source: &'r mut MagicString<'static>,
  pub chunk_graph: &'r ChunkGraph,
  pub wrap_symbol_name: Option<&'r Atom>,
  pub default_symbol_name: Option<&'r Atom>,
  // Used to hoisted declaration for import module, including import declaration and export declaration which has source imported
  pub first_stmt_start: Option<u32>,
  // Used to determine whether the call result is used.
  pub call_result_un_used: bool,
}

impl<'r> AstRenderContext<'r> {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    graph: &'r Graph,
    canonical_names: &'r FxHashMap<SymbolRef, Atom>,
    source: &'r mut MagicString<'static>,
    chunk_graph: &'r ChunkGraph,
    module: &'r NormalModule,
    linking_info: &'r LinkingInfo,
  ) -> Self {
    let wrap_symbol_name =
      linking_info.wrap_symbol.map(|s| graph.symbols.canonical_name_for(s, canonical_names));
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
      wrap_symbol_name,
      default_symbol_name,
      first_stmt_start: None,
      call_result_un_used: false,
    }
  }
}

#[derive(Debug, Default)]
pub struct RenderKindWrappedEsm {
  pub hoisted_vars: Vec<Atom>,
  pub hoisted_functions: Vec<Span>,
}

#[derive(Debug)]
pub enum RenderKind {
  WrappedEsm(Box<RenderKindWrappedEsm>),
  Cjs,
  Esm,
}

impl RenderKind {
  pub fn from_wrap_kind(kind: &WrapKind) -> Self {
    match kind {
      WrapKind::None => Self::Esm,
      WrapKind::CJS => Self::Cjs,
      WrapKind::ESM => Self::WrappedEsm(Box::default()),
    }
  }
}

#[derive(Debug)]
pub struct AstRenderer<'r> {
  ctx: AstRenderContext<'r>,
  kind: RenderKind,
}

impl<'r> AstRenderer<'r> {
  pub fn new(ctx: AstRenderContext<'r>, kind: RenderKind) -> Self {
    Self { ctx, kind }
  }
}

impl<'r> AstRenderer<'r> {
  pub fn render(&mut self) {
    let program = self.ctx.module.ast.program();
    self.visit_program(program);

    match &mut self.kind {
      RenderKind::WrappedEsm(info) => {
        info.hoisted_functions.iter().for_each(|f| {
          // Improve: multiply functions should separate by "\n"
          self.ctx.source.relocate(f.start, f.end, 0);
          self.ctx.source.append_right(0, "\n");
        });
        if !info.hoisted_vars.is_empty() {
          self.ctx.source.append_right(0, format!("var {};\n", info.hoisted_vars.join(",")));
        }

        if let Some(s) = self.ctx.generate_namespace_variable_declaration() {
          self.ctx.source.append_right(0, s);
        }

        let wrap_ref_name = self.ctx.wrap_symbol_name.unwrap();
        let esm_ref_name = self.ctx.canonical_name_for_runtime("__esm");
        self.ctx.source.append_right(
          0,
          format!(
            "var {wrap_ref_name} = {esm_ref_name}({{\n'{}'() {{\n",
            self.ctx.module.resource_id.prettify(),
          ),
        );
        self.ctx.source.append("\n}\n});");
      }
      RenderKind::Cjs => {
        let wrap_ref_name = self.ctx.wrap_symbol_name.unwrap();
        let prettify_id = self.ctx.module.resource_id.prettify();
        let commonjs_ref_name = self.ctx.canonical_name_for_runtime("__commonJS");
        self.ctx.source.prepend(format!(
          "var {wrap_ref_name} = {commonjs_ref_name}({{\n'{prettify_id}'(exports, module) {{\n",
        ));
        self.ctx.source.append("\n}\n});");
        assert!(!self.ctx.module.is_namespace_referenced());
      }
      RenderKind::Esm => {
        if let Some(s) = self.ctx.generate_namespace_variable_declaration() {
          self.ctx.source.prepend(s);
        }
      }
    }
  }

  fn strip_export_keyword(
    &mut self,
    decl: &oxc::ast::ast::ExportDefaultDeclaration,
  ) -> RenderControl {
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
    RenderControl::Skip
  }

  /// Rewrite statement `require('./foo.js')` to `init_foo()`
  fn try_render_require_statement(&mut self, stmt: &oxc::ast::ast::Statement) -> RenderControl {
    // only direct call is result unused eg `init()`
    let oxc::ast::ast::Statement::ExpressionStatement(expr) = stmt else {
      return RenderControl::Continue;
    };
    let oxc::ast::ast::Expression::CallExpression(call_exp) = &expr.expression else {
      return RenderControl::Continue;
    };
    match call_exp.callee {
      oxc::ast::ast::Expression::Identifier(ref ident) if ident.name == "require" => {
        let Module::Normal(importee) = self.get_importee_by_span(call_exp.span) else {
          return RenderControl::Continue;
        };
        let importee_linking_info = &self.ctx.graph.linking_infos[importee.id];
        let wrap_ref_name = self.canonical_name_for(importee_linking_info.wrap_symbol.unwrap());
        self.ctx.source.update(expr.span.start, expr.span.end, format!("{wrap_ref_name}()"));
        RenderControl::Skip
      }
      _ => RenderControl::Continue,
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
    let Some(symbol_id) = self.ctx.graph.symbols.references_table[self.ctx.module.id]
      [ident.reference_id.get().unwrap()]
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
