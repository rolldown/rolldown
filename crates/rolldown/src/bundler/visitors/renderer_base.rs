use oxc::span::{Atom, GetSpan, Span};
use rolldown_common::{ExportsKind, SymbolRef};
use rolldown_oxc::BindingIdentifierExt;
use rustc_hash::FxHashMap;
use string_wizard::{MagicString, UpdateOptions};

use super::super::{
  chunk_graph::ChunkGraph,
  graph::{graph::Graph, linker::LinkingInfo},
  module::{Module, NormalModule},
};

/// Different renderers share some common logic, so we extract them into this struct.
pub struct RendererBase<'ast> {
  pub graph: &'ast Graph,
  pub module: &'ast NormalModule,
  pub linking_info: &'ast LinkingInfo,
  pub canonical_names: &'ast FxHashMap<SymbolRef, Atom>,
  pub source: &'ast mut MagicString<'static>,
  pub chunk_graph: &'ast ChunkGraph,
  pub wrap_symbol_name: Option<&'ast Atom>,
  pub default_symbol_name: Option<&'ast Atom>,
  // Used to hoisted import declaration before the first statement
  pub first_stmt_start: Option<u32>,
}

impl<'ast> RendererBase<'ast> {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    graph: &'ast Graph,
    canonical_names: &'ast FxHashMap<SymbolRef, Atom>,
    source: &'ast mut MagicString<'static>,
    chunk_graph: &'ast ChunkGraph,
    module: &'ast NormalModule,
    linking_info: &'ast LinkingInfo,
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
    }
  }

  pub fn overwrite(&mut self, start: u32, end: u32, content: String) {
    self.source.update_with(
      start,
      end,
      content,
      UpdateOptions { overwrite: true, ..Default::default() },
    );
  }

  pub fn remove_node(&mut self, span: Span) {
    self.source.remove(span.start, span.end);
  }

  pub fn rename_symbol(&mut self, span: Span, name: String) {
    self.overwrite(span.start, span.end, name);
  }

  pub fn canonical_name_for(&self, symbol: SymbolRef) -> &'ast Atom {
    &self.graph.symbols.canonical_name_for(symbol, &self.canonical_names)
  }

  pub fn canonical_name_for_runtime(&self, name: &Atom) -> &Atom {
    let symbol = self.graph.runtime.resolve_symbol(name);
    self.canonical_name_for(symbol)
  }

  pub fn need_to_rename(&self, symbol: SymbolRef) -> Option<&Atom> {
    let canonical_ref = self.graph.symbols.par_canonical_ref_for(symbol);
    self.canonical_names.get(&canonical_ref)
  }

  pub fn generate_namespace_variable_declaration(&mut self) -> Option<String> {
    if self.module.is_namespace_referenced() {
      let namespace_name = self.canonical_name_for(self.module.namespace_symbol);
      let exports: String = self
        .linking_info
        .resolved_exports
        .iter()
        .map(|(exported_name, symbol_ref)| {
          let canonical_ref = self.graph.symbols.par_canonical_ref_for(*symbol_ref);
          let symbol = self.graph.symbols.get(canonical_ref);
          let return_expr = if let Some(ns_alias) = &symbol.namespace_alias {
            let canonical_ns_name = &self.canonical_names[&ns_alias.namespace_ref];
            format!("{canonical_ns_name}.{exported_name}",)
          } else {
            let canonical_name = self.canonical_names.get(&canonical_ref).unwrap();
            format!("{canonical_name}",)
          };
          format!("  get {exported_name}() {{ return {return_expr} }}",)
        })
        .collect::<Vec<_>>()
        .join(",\n");
      Some(format!("var {namespace_name} = {{\n{exports}\n}};\n",))
    } else {
      None
    }
  }

  pub fn generate_import_commonjs_module(
    &self,
    importee: &NormalModule,
    importee_linking_info: &LinkingInfo,
    with_namespace_init: bool,
  ) -> String {
    let wrap_symbol_name = self.canonical_name_for(importee_linking_info.wrap_symbol.unwrap());
    let to_esm_runtime_symbol_name = self.canonical_name_for_runtime(&"__toESM".into());
    let code = format!(
      "{to_esm_runtime_symbol_name}({wrap_symbol_name}(){})",
      if self.module.module_type.is_esm() { ", 1" } else { "" }
    );
    if with_namespace_init {
      let namespace_name = self.canonical_name_for(importee.namespace_symbol);
      format!("var {namespace_name} = {code};\n")
    } else {
      code
    }
  }

  pub fn get_importee_by_span(&self, span: Span) -> &Module {
    &self.graph.modules[self.module.get_import_module_by_span(span)]
  }

  pub fn visit_binding_identifier(&mut self, ident: &'ast oxc::ast::ast::BindingIdentifier) {
    let symbol_ref: SymbolRef = (self.module.id, ident.expect_symbol_id()).into();

    match self.need_to_rename(symbol_ref) {
      Some(name) if name != &ident.name => {
        self.rename_symbol(ident.span, name.to_string());
      }
      _ => {}
    }
  }

  pub fn visit_identifier_reference(
    &mut self,
    ident: &'ast oxc::ast::ast::IdentifierReference,
    is_call: bool,
  ) {
    let Some(symbol_id) =
      self.graph.symbols.references_table[self.module.id][ident.reference_id.get().unwrap()]
    else {
      // This is global identifier references, eg `console.log`. We don't need to rewrite it.
      return;
    };
    let symbol_ref = (self.module.id, symbol_id).into();
    let symbol = self.graph.symbols.get(symbol_ref);
    if let Some(ns_alias) = &symbol.namespace_alias {
      // If import symbol from commonjs, the reference of the symbol is not resolved,
      // Here need write it to property access. eg `import { a } from 'cjs'; console.log(a)` => `console.log(cjs_ns.a)`
      // Note: we should rewrite call expression to indirect call, eg `import { a } from 'cjs'; console.log(a())` => `console.log((0, cjs_ns.a)())`
      let canonical_ns_name = self.canonical_name_for(ns_alias.namespace_ref);
      let property_name = &ns_alias.property_name;
      self.source.update(
        ident.span.start,
        ident.span.end,
        if is_call {
          format!("(0, {canonical_ns_name}.{property_name})",)
        } else {
          format!("{canonical_ns_name}.{property_name}",)
        },
      );
    } else if let Some(name) = self.need_to_rename(symbol_ref) {
      if ident.name != name {
        self.rename_symbol(ident.span, name.to_string());
      }
    }
  }

  pub fn visit_export_all_declaration(
    &mut self,
    decl: &'ast oxc::ast::ast::ExportAllDeclaration<'ast>,
  ) {
    if let Module::Normal(importee) = self.get_importee_by_span(decl.span) {
      if importee.exports_kind == ExportsKind::CommonJs {
        // __reExport(a_exports, __toESM(require_c()));
        let namespace_name = &self.canonical_names[&importee.namespace_symbol];
        let re_export_runtime_symbol_name = self.canonical_name_for_runtime(&"__reExport".into());
        self.source.update(
          decl.span.start,
          decl.span.end,
          format!(
            "{re_export_runtime_symbol_name}({namespace_name}, {});",
            self.generate_import_commonjs_module(
              importee,
              &self.graph.linking_infos[importee.id],
              false
            )
          ),
        );
        return;
      }
    }
    self.remove_node(decl.span);
  }

  pub fn visit_import_expression(&mut self, expr: &oxc::ast::ast::ImportExpression<'ast>) {
    if let oxc::ast::ast::Expression::StringLiteral(str) = &expr.source {
      if let Some(chunk_id) =
        self.chunk_graph.module_to_chunk[self.module.get_import_module_by_span(expr.span)]
      {
        let chunk = &self.chunk_graph.chunks[chunk_id];
        self.overwrite(
          str.span.start,
          str.span.end,
          // TODO: the path should be relative to the current importer chunk
          format!("'./{}'", chunk.file_name.as_ref().unwrap()),
        );
      } else {
        // external module doesn't belong to any chunk, just keep this as it is
      }
    }
  }

  pub fn visit_import_declaration(&mut self, decl: &'ast oxc::ast::ast::ImportDeclaration<'ast>) {
    self.remove_node(decl.span);
    let module_id = self.module.get_import_module_by_span(decl.span);
    let importee = &self.graph.modules[module_id];
    let importee_linking_info = &self.graph.linking_infos[module_id];
    let start = self.first_stmt_start.unwrap_or(decl.span.start);
    if let Module::Normal(importee) = importee {
      if importee.exports_kind == ExportsKind::CommonJs {
        self.source.append_right(
          start,
          self.generate_import_commonjs_module(
            importee,
            &self.graph.linking_infos[importee.id],
            true,
          ),
        );
      } else if let Some(wrap_symbol) = importee_linking_info.wrap_symbol {
        let wrap_symbol_name = self.canonical_name_for(wrap_symbol);
        // init wrapped esm module
        self.source.append_right(start, format!("{wrap_symbol_name}();\n"));
      }
    }
  }

  pub fn visit_call_expression(&mut self, expr: &'ast oxc::ast::ast::CallExpression<'ast>) {
    if let oxc::ast::ast::Expression::Identifier(ident) = &expr.callee {
      if ident.name == "require" {
        if let Module::Normal(importee) = self.get_importee_by_span(expr.span) {
          let importee_linking_info = &self.graph.linking_infos[importee.id];
          let wrap_symbol_name =
            self.canonical_name_for(importee_linking_info.wrap_symbol.unwrap());
          if importee.exports_kind == ExportsKind::CommonJs {
            self.source.update(expr.span.start, expr.span.end, format!("{wrap_symbol_name}()"));
          } else {
            let namespace_name = self.canonical_name_for(importee.namespace_symbol);
            let to_commonjs_runtime_symbol_name =
              self.canonical_name_for_runtime(&"__toCommonJS".into());
            self.source.update(
              expr.span.start,
              expr.span.end,
              format!(
                "({wrap_symbol_name}(), {to_commonjs_runtime_symbol_name}({namespace_name}))"
              ),
            );
          }
        }
      }
    }
  }

  pub fn visit_statement(&mut self, stmt: &'ast oxc::ast::ast::Statement<'ast>) {
    if !matches!(stmt, oxc::ast::ast::Statement::Declaration(_)) && self.first_stmt_start.is_none()
    {
      self.first_stmt_start = Some(stmt.span().start);
    }
  }
}
