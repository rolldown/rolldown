pub mod commonjs_source_render;
pub mod esm_source_render;
pub mod esm_wrap_source_render;
pub mod scanner;
use index_vec::IndexVec;
use oxc::{
  semantic::ReferenceId,
  span::{Atom, Span},
};
use rolldown_common::{ExportsKind, ModuleId, SymbolRef};
use rustc_hash::FxHashMap;
use string_wizard::{MagicString, UpdateOptions};

use super::{
  chunk::{chunk::Chunk, ChunkId},
  graph::symbols::{get_reference_final_name, get_symbol_final_name, Symbols},
  module::{module::Module, module_id::ModuleVec, NormalModule},
  runtime::Runtime,
};

pub struct RendererContext<'ast> {
  symbols: &'ast Symbols,
  final_names: &'ast FxHashMap<SymbolRef, Atom>,
  source: &'ast mut MagicString<'static>,
  module_to_chunk: &'ast IndexVec<ModuleId, Option<ChunkId>>,
  chunks: &'ast IndexVec<ChunkId, Chunk>,
  modules: &'ast ModuleVec,
  module: &'ast NormalModule,
  wrap_symbol_name: Option<&'ast Atom>,
  namespace_symbol_name: Option<&'ast Atom>,
  default_symbol_name: Option<&'ast Atom>,
  runtime: &'ast Runtime,
}

impl<'ast> RendererContext<'ast> {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    symbols: &'ast Symbols,
    final_names: &'ast FxHashMap<SymbolRef, Atom>,
    source: &'ast mut MagicString<'static>,
    module_to_chunk: &'ast IndexVec<ModuleId, Option<ChunkId>>,
    chunks: &'ast IndexVec<ChunkId, Chunk>,
    modules: &'ast ModuleVec,
    module: &'ast NormalModule,
    runtime: &'ast Runtime,
  ) -> Self {
    let wrap_symbol_name =
      module.wrap_symbol.and_then(|s| get_symbol_final_name(s, symbols, final_names));
    let namespace_symbol_name = get_symbol_final_name(
      (module.id, module.namespace_symbol.0.symbol).into(),
      symbols,
      final_names,
    );
    let default_symbol_name = module
      .default_export_symbol
      .and_then(|s| get_symbol_final_name((module.id, s).into(), symbols, final_names));
    Self {
      symbols,
      final_names,
      source,
      module_to_chunk,
      chunks,
      modules,
      module,
      wrap_symbol_name,
      namespace_symbol_name,
      default_symbol_name,
      runtime,
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

  #[allow(clippy::needless_pass_by_value)]
  pub fn rename_symbol(&mut self, span: Span, name: Atom) {
    self.overwrite(span.start, span.end, name.to_string());
  }

  pub fn get_symbol_final_name(&self, symbol: SymbolRef) -> Option<&'ast Atom> {
    get_symbol_final_name(symbol, self.symbols, self.final_names)
  }

  pub fn get_reference_final_name(
    &self,
    module_id: ModuleId,
    reference_id: ReferenceId,
  ) -> Option<&Atom> {
    get_reference_final_name(module_id, reference_id, self.symbols, self.final_names)
  }

  pub fn get_runtime_symbol_final_name(&self, name: &Atom) -> &Atom {
    let symbol = self.runtime.resolve_symbol(name);
    self.get_symbol_final_name(symbol).unwrap()
  }

  pub fn visit_binding_identifier(&mut self, ident: &'ast oxc::ast::ast::BindingIdentifier) {
    if let Some(name) =
      self.get_symbol_final_name((self.module.id, ident.symbol_id.get().unwrap()).into())
    {
      if ident.name != name {
        self.rename_symbol(ident.span, name.clone());
      }
    }
  }

  pub fn visit_identifier_reference(&mut self, ident: &'ast oxc::ast::ast::IdentifierReference) {
    if let Some(name) =
      self.get_reference_final_name(self.module.id, ident.reference_id.get().unwrap())
    {
      if ident.name != name {
        self.rename_symbol(ident.span, name.clone());
      }
    }
  }

  pub fn visit_export_all_declaration(
    &mut self,
    decl: &'ast oxc::ast::ast::ExportAllDeclaration<'ast>,
  ) {
    self.remove_node(decl.span);
  }

  pub fn visit_import_expression(&mut self, expr: &oxc::ast::ast::ImportExpression<'ast>) {
    if let oxc::ast::ast::Expression::StringLiteral(str) = &expr.source {
      let rec = &self.module.import_records[self.module.imports.get(&expr.span).copied().unwrap()];

      if let Some(chunk_id) = self.module_to_chunk[rec.resolved_module] {
        let chunk = &self.chunks[chunk_id];
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
    let rec = &self.module.import_records[self.module.imports.get(&decl.span).copied().unwrap()];
    let importee = &self.modules[rec.resolved_module];
    if let Module::Normal(importee) = importee {
      if importee.exports_kind == ExportsKind::CommonJs {
        // add import cjs symbol binding
        let namespace_name = self
          .get_symbol_final_name((importee.id, importee.namespace_symbol.0.symbol).into())
          .unwrap();
        let wrap_symbol_name = self.get_symbol_final_name(importee.wrap_symbol.unwrap()).unwrap();
        let to_esm_runtime_symbol_name = self.get_runtime_symbol_final_name(&"__toESM".into());
        self.source.prepend_left(
          decl.span.start,
          format!(
            "var {namespace_name} = {to_esm_runtime_symbol_name}({wrap_symbol_name}(){});\n",
            if self.module.module_type.is_esm() {
              ", 1"
            } else {
              ""
            }
          ),
        );
        decl.specifiers.iter().for_each(|s| match s {
          oxc::ast::ast::ImportDeclarationSpecifier::ImportSpecifier(spec) => {
            if let Some(name) = self.get_symbol_final_name(
              (importee.id, importee.cjs_symbols.get(spec.imported.name()).unwrap().symbol).into(),
            ) {
              self
                .source
                .prepend_left(decl.span.start, format!("var {name} = {namespace_name}.{name};\n"));
            }
          }
          oxc::ast::ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => {
            if let Some(name) = self.get_symbol_final_name(
              (importee.id, importee.cjs_symbols.get(&Atom::new_inline("default")).unwrap().symbol)
                .into(),
            ) {
              self
                .source
                .prepend_left(decl.span.start, format!("var {name} = {namespace_name}.default;\n"));
            }
          }
          oxc::ast::ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {}
        });
      } else if let Some(wrap_symbol) = importee.wrap_symbol {
        let wrap_symbol_name = self.get_symbol_final_name(wrap_symbol).unwrap();
        // init wrapped esm module
        self.source.prepend_left(decl.span.start, format!("{wrap_symbol_name}();\n"));
      }
    }
  }
}
