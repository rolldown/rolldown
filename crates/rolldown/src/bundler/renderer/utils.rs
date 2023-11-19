use std::borrow::Cow;

use oxc::span::{Atom, Span};
use rolldown_common::SymbolRef;
use string_wizard::UpdateOptions;

use crate::bundler::{
  linker::linker_info::LinkingInfo,
  module::{Module, NormalModule},
};

use super::{AstRenderContext, AstRenderer};
impl<'r> AstRenderContext<'r> {
  pub fn canonical_name_for(&self, symbol: SymbolRef) -> &'r Atom {
    self.graph.symbols.canonical_name_for(symbol, self.canonical_names)
  }

  pub fn need_to_rename(&self, symbol: SymbolRef) -> Option<&Atom> {
    let canonical_ref = self.graph.symbols.par_canonical_ref_for(symbol);
    self.canonical_names.get(&canonical_ref)
  }

  pub fn canonical_name_for_runtime(&self, name: &str) -> &Atom {
    let symbol = self.graph.runtime.resolve_symbol(name);
    self.canonical_name_for(symbol)
  }

  pub fn importee_by_span(&self, span: Span) -> &Module {
    &self.graph.modules[self.module.importee_id_by_span(span)]
  }

  pub fn remove_node(&mut self, span: Span) {
    self.source.remove(span.start, span.end);
  }

  pub fn hoisted_module_declaration(&mut self, decl_start: u32, content: String) {
    let start = self.first_stmt_start.unwrap_or(decl_start);
    self.source.append_left(start, content);
  }

  pub fn generate_import_commonjs_module(
    &self,
    importee: &NormalModule,
    importee_linking_info: &LinkingInfo,
    with_declaration: bool,
  ) -> String {
    let wrap_ref_name = self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
    let to_esm_ref_name = self.canonical_name_for_runtime("__toESM");
    let code = format!(
      "{to_esm_ref_name}({wrap_ref_name}(){})",
      if self.module.module_type.is_esm() { ", 1" } else { "" }
    );
    if with_declaration {
      let symbol_ref =
        self.linking_info.local_symbol_for_import_cjs.get(&importee.id).copied().unwrap_or_else(
          || {
            panic!(
              "Cannot find local symbol for importee: {:?} with importer {:?} {:?}",
              importee.resource_id, self.module.resource_id, self.module.exports_kind
            )
          },
        );
      let final_name = self.canonical_name_for(symbol_ref);
      format!("var {final_name} = {code};\n")
    } else {
      code
    }
  }
}

impl<'r> AstRenderer<'r> {
  pub fn is_global_require(&self, callee: &oxc::ast::ast::Expression<'_>) -> bool {
    matches!(callee, oxc::ast::ast::Expression::Identifier(ident) if
      ident.name == "require"
      && self.ctx.module.scope.is_unresolved(ident.reference_id.get().unwrap())
    )
  }

  pub fn overwrite(&mut self, start: u32, end: u32, content: String) {
    self.ctx.source.update_with(
      start,
      end,
      content,
      UpdateOptions { overwrite: true, ..Default::default() },
    );
  }

  pub fn generate_namespace_variable_declaration(&mut self) -> Option<String> {
    if self.ctx.module.is_namespace_referenced() {
      let ns_name = self.ctx.canonical_name_for(self.ctx.module.namespace_symbol);
      let exports: String = self
        .ctx
        .linking_info
        .sorted_exports()
        .map(|(exported_name, resolved_export)| {
          let canonical_ref =
            self.ctx.graph.symbols.par_canonical_ref_for(resolved_export.symbol_ref);
          let symbol = self.ctx.graph.symbols.get(canonical_ref);
          let return_expr = if let Some(ns_alias) = &symbol.namespace_alias {
            let canonical_ns_name = &self.ctx.canonical_names[&ns_alias.namespace_ref];
            format!("{canonical_ns_name}.{exported_name}",)
          } else {
            let canonical_name = self.ctx.canonical_names.get(&canonical_ref).unwrap();
            format!("{canonical_name}",)
          };
          format!("{}get {exported_name}() {{ return {return_expr} }}", self.indentor)
        })
        .collect::<Vec<_>>()
        .join(",\n");
      Some(format!("var {ns_name} = {{\n{exports}\n}};\n",))
    } else {
      None
    }
  }

  pub fn remove_node(&mut self, span: Span) {
    self.ctx.source.remove(span.start, span.end);
  }

  pub fn rewrite_symbol(
    &mut self,
    symbol_ref: SymbolRef,
    original_name: &Atom,
    pos: Span,
    is_callee: bool,
  ) {
    let canonical_ref = self.ctx.graph.symbols.par_canonical_ref_for(symbol_ref);
    let symbol = self.ctx.graph.symbols.get(canonical_ref);
    let rendered_symbol = if let Some(ns_alias) = &symbol.namespace_alias {
      // If import symbol from commonjs, the reference of the symbol is not resolved,
      // Here need write it to property access. eg `import { a } from 'cjs'; console.log(a)` => `console.log(import_a.a)`
      // Note: we should rewrite call expression to indirect call, eg `import { a } from 'cjs'; console.log(a())` => `console.log((0, import_a.a)())`
      let canonical_ns_name = self.canonical_name_for(ns_alias.namespace_ref);
      let content = format!("{canonical_ns_name}.{}", ns_alias.property_name);
      Cow::Owned(content)
    } else if let Some(name) = self.need_to_rename(symbol_ref) {
      Cow::Borrowed(name.as_str())
    } else {
      return;
    };

    if original_name != &rendered_symbol {
      self.ctx.source.update(
        pos.start,
        pos.end,
        if is_callee { format!("(0, {rendered_symbol})",) } else { rendered_symbol.into_owned() },
      );
    }
  }

  pub fn canonical_name_for(&self, symbol: SymbolRef) -> &'r Atom {
    self.ctx.graph.symbols.canonical_name_for(symbol, self.ctx.canonical_names)
  }

  pub fn canonical_name_for_runtime(&self, name: &str) -> &Atom {
    let symbol = self.ctx.graph.runtime.resolve_symbol(&Atom::new_inline(name));
    self.canonical_name_for(symbol)
  }

  pub fn need_to_rename(&self, symbol: SymbolRef) -> Option<&Atom> {
    let canonical_ref = self.ctx.graph.symbols.par_canonical_ref_for(symbol);
    self.ctx.canonical_names.get(&canonical_ref)
  }

  pub fn hoisted_module_declaration(&mut self, decl_start: u32, content: String) {
    let start = self.ctx.first_stmt_start.unwrap_or(decl_start);
    self.ctx.source.append_left(start, content);
  }
}
