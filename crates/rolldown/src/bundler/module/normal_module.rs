use std::fmt::Debug;

use index_vec::IndexVec;
use oxc::{
  semantic::{ScopeTree, SymbolId},
  span::{Atom, Span},
};
use rolldown_common::{
  ExportsKind, ImportRecord, ImportRecordId, LocalOrReExport, ModuleId, ModuleType, NamedImport,
  ResourceId, StmtInfo, SymbolRef,
};
use rolldown_oxc::OxcProgram;
use rustc_hash::{FxHashMap, FxHashSet};
use string_wizard::MagicString;

use super::{module::ModuleRenderContext, module_id::ModuleVec};
use crate::bundler::{
  graph::{linker::LinkerModule, symbols::Symbols},
  module::module::Module,
  visitors::{
    commonjs_source_render::CommonJsSourceRender, esm_source_render::EsmSourceRender,
    esm_wrap_source_render::EsmWrapSourceRender, RendererContext,
  },
};

#[derive(Debug)]
pub struct NormalModule {
  pub exec_order: u32,
  pub id: ModuleId,
  pub is_entry: bool,
  pub resource_id: ResourceId,
  pub module_type: ModuleType,
  pub ast: OxcProgram,
  pub named_imports: FxHashMap<SymbolId, NamedImport>,
  pub named_exports: FxHashMap<Atom, LocalOrReExport>,
  pub stmt_infos: Vec<StmtInfo>,
  pub import_records: IndexVec<ImportRecordId, ImportRecord>,
  pub imports: FxHashMap<Span, ImportRecordId>,
  // [[StarExportEntries]] in https://tc39.es/ecma262/#sec-source-text-module-records
  pub star_exports: Vec<ImportRecordId>,
  pub exports_kind: ExportsKind,
  pub scope: ScopeTree,
  pub namespace_symbol: SymbolRef,
  pub default_export_symbol: Option<SymbolId>,
}

#[derive(Debug, Clone)]
pub struct UnresolvedSymbol {
  // The unresolved symbol is from the importee namespace symbol
  pub importee_namespace: SymbolRef,
  // The unresolved symbol reference symbol name
  pub reference_name: Option<Atom>,
}

#[derive(Debug)]
pub enum Resolution {
  None,
  Ambiguous,
  Found(SymbolRef),
  // Mark the symbol symbol maybe from commonjs
  // - The importee has `export * from 'cjs'`
  // - The importee is commonjs
  Runtime(SymbolRef),
}

impl NormalModule {
  #[allow(clippy::needless_pass_by_value)]
  pub fn render(&self, ctx: ModuleRenderContext<'_>) -> Option<MagicString<'static>> {
    // FIXME: should not clone
    let source = self.ast.source();
    let mut source = MagicString::new(source.to_string());
    let self_linker_module = &ctx.graph.linker_modules[self.id];
    let ctx = RendererContext::new(
      ctx.graph,
      ctx.canonical_names,
      &mut source,
      ctx.chunk_graph,
      self,
      self_linker_module,
    );

    if self.exports_kind == ExportsKind::CommonJs {
      CommonJsSourceRender::new(ctx).apply();
    } else if self_linker_module.wrap_symbol.is_some() {
      EsmWrapSourceRender::new(ctx).apply();
    } else {
      EsmSourceRender::new(ctx).apply();
    }

    source.prepend(format!("// {}\n", self.resource_id.prettify()));

    // TODO trim
    if source.len() == 0 {
      None
    } else {
      Some(source)
    }
  }

  pub fn initialize_namespace(&self, self_linker_module: &mut LinkerModule) {
    if !self_linker_module.is_symbol_for_namespace_referenced {
      self_linker_module.is_symbol_for_namespace_referenced = true;
      self_linker_module
        .facade_stmt_infos
        .push(StmtInfo { declared_symbols: vec![self.namespace_symbol], ..Default::default() });
    }
  }

  // https://tc39.es/ecma262/#sec-getexportednames
  pub fn get_exported_names<'modules>(
    &'modules self,
    stack: &mut Vec<ModuleId>,
    modules: &'modules ModuleVec,
  ) -> FxHashSet<&'modules Atom> {
    if stack.contains(&self.id) {
      // cycle
      return FxHashSet::default();
    }

    stack.push(self.id);

    let ret: FxHashSet<&'modules Atom> = {
      self
        .get_star_exports_modules()
        .flat_map(|id| {
          let importee = &modules[id];
          match importee {
            Module::Normal(importee) => importee
              .get_exported_names(stack, modules)
              .into_iter()
              .filter(|name| name.as_str() != "default")
              .collect::<Vec<_>>(),
            Module::External(importee) => importee
              .symbols_imported_by_others
              .keys()
              .filter(|name| name.as_str() != "default")
              .collect(),
          }
        })
        .chain(self.named_exports.keys())
        .collect()
    };

    stack.pop();
    ret
  }

  // https://tc39.es/ecma262/#sec-resolveexport
  #[allow(clippy::too_many_lines)]
  pub fn resolve_export<'modules>(
    &'modules self,
    export_name: &'modules Atom,
    resolve_set: &mut Vec<(ModuleId, &'modules Atom)>,
    modules: &'modules ModuleVec,
    symbols: &mut Symbols,
  ) -> Resolution {
    let record = (self.id, export_name);
    if resolve_set.iter().rev().any(|prev| prev == &record) {
      unimplemented!("handle cycle")
    }
    resolve_set.push(record);

    let ret = if let Some(info) = self.named_exports.get(export_name) {
      match info {
        LocalOrReExport::Local(local) => {
          if let Some(named_import) = self.named_imports.get(&local.referenced.symbol) {
            let record = &self.import_records[named_import.record_id];
            let importee = &modules[record.resolved_module];
            match importee {
              Module::Normal(importee) => {
                let resolved = if named_import.is_imported_star {
                  Resolution::Found(importee.namespace_symbol)
                } else {
                  importee.resolve_export(&named_import.imported, resolve_set, modules, symbols)
                };
                if let Resolution::Found(exist) = &resolved {
                  symbols.union(local.referenced, *exist);
                }
                resolved
              }
              Module::External(importee) => {
                let resolve =
                  importee.resolve_export(&named_import.imported, named_import.is_imported_star);
                return Resolution::Found(resolve);
              }
            }
          } else {
            Resolution::Found(local.referenced)
          }
        }
        LocalOrReExport::Re(re) => {
          let record = &self.import_records[re.record_id];
          let importee = &modules[record.resolved_module];
          match importee {
            Module::Normal(importee) => {
              if re.is_imported_star {
                return Resolution::Found(importee.namespace_symbol);
              }
              importee.resolve_export(&re.imported, resolve_set, modules, symbols)
            }
            Module::External(importee) => {
              let resolve = importee.resolve_export(&re.imported, re.is_imported_star);
              return Resolution::Found(resolve);
            }
          }
        }
      }
    } else {
      if export_name.as_str() == "default" {
        return Resolution::None;
      }
      let mut star_resolution: Option<SymbolRef> = None;
      for module_id in self.get_star_exports_modules() {
        let importee = &modules[module_id];
        match importee {
          Module::Normal(importee) => {
            match importee.resolve_export(export_name, resolve_set, modules, symbols) {
              Resolution::None => continue,
              Resolution::Ambiguous => return Resolution::Ambiguous,
              Resolution::Found(exist) => {
                if let Some(star_resolution) = star_resolution {
                  if star_resolution == exist {
                    continue;
                  }
                  return Resolution::Ambiguous;
                }
                star_resolution = Some(exist);
              }
              Resolution::Runtime(_) => {
                unreachable!("should not found symbol at runtime for esm")
              }
            }
          }
          Module::External(_) => {
            // unimplemented!("handle external module")
          }
        }
      }

      star_resolution.map_or(Resolution::None, Resolution::Found)
    };

    resolve_set.pop();

    ret
  }

  pub fn resolve_export_for_esm_and_cjs<'modules>(
    &'modules self,
    export_name: &'modules Atom,
    resolve_set: &mut Vec<(ModuleId, &'modules Atom)>,
    modules: &'modules ModuleVec,
    symbols: &mut Symbols,
  ) -> Resolution {
    // First found symbol resolution for esm, if not found, then try to resolve for commonjs
    let resolution = self.resolve_export(export_name, resolve_set, modules, symbols);
    if matches!(resolution, Resolution::None) {
      let has_cjs_star_resolution = self
        .get_star_exports_modules()
        .map(|id| {
          let importee = &modules[id];
          match importee {
            Module::Normal(importee) => importee.exports_kind == ExportsKind::CommonJs,
            Module::External(_) => false,
          }
        })
        .any(|is_cjs| is_cjs);
      if let Some(info) = self.named_exports.get(export_name) {
        match info {
          LocalOrReExport::Local(local) => {
            if let Some(named_import) = self.named_imports.get(&local.referenced.symbol) {
              let record = &self.import_records[named_import.record_id];
              let importee = &modules[record.resolved_module];
              match importee {
                Module::Normal(importee) => {
                  if importee.exports_kind == ExportsKind::CommonJs {
                    return Resolution::Runtime(importee.namespace_symbol);
                  }
                }
                Module::External(_) => {}
              }
            }
          }
          LocalOrReExport::Re(re) => {
            let record = &self.import_records[re.record_id];
            let importee = &modules[record.resolved_module];
            match importee {
              Module::Normal(importee) => {
                if importee.exports_kind == ExportsKind::CommonJs {
                  return Resolution::Runtime(importee.namespace_symbol);
                }
              }
              Module::External(_) => {}
            }
          }
        }
      } else if has_cjs_star_resolution || self.exports_kind == ExportsKind::CommonJs {
        return Resolution::Runtime(self.namespace_symbol);
      }
      return Resolution::None;
    }
    resolution
  }

  pub fn resolve_star_exports(&self, modules: &ModuleVec) -> Vec<ModuleId> {
    let mut visited = FxHashSet::default();
    let mut resolved = vec![];
    let mut queue = self.get_star_exports_modules().collect::<Vec<_>>();

    while let Some(importee_id) = queue.pop() {
      if !visited.contains(&importee_id) {
        visited.insert(importee_id);
        resolved.push(importee_id);
        let importee = &modules[importee_id];
        match importee {
          Module::Normal(importee) => queue.extend(importee.get_star_exports_modules()),
          Module::External(_) => {}
        }
      }
    }

    resolved
  }

  pub fn create_wrap_symbol(&self, self_linker_module: &mut LinkerModule, symbols: &mut Symbols) {
    if self_linker_module.wrap_symbol.is_none() {
      let name = format!(
        "{}_{}",
        if self.exports_kind == ExportsKind::CommonJs { "require" } else { "init" },
        self.resource_id.generate_unique_name()
      )
      .into();
      let symbol = symbols.create_symbol(self.id, name).symbol;
      self_linker_module.wrap_symbol = Some((self.id, symbol).into());
      self_linker_module
        .facade_stmt_infos
        .push(StmtInfo { declared_symbols: vec![(self.id, symbol).into()], ..Default::default() });
      self.initialize_namespace(self_linker_module);
    }
  }

  pub fn generate_symbol_import_and_use(
    &self,
    symbol_ref_from_importee: SymbolRef,
    self_linker_module: &mut LinkerModule,
    symbols: &mut Symbols,
  ) {
    debug_assert!(symbol_ref_from_importee.owner != self.id);
    let name = symbols.get_original_name(symbol_ref_from_importee).clone();
    let local_symbol_ref = self.generate_local_symbol(name, self_linker_module, symbols);
    symbols.union(local_symbol_ref, symbol_ref_from_importee);
  }

  pub fn generate_local_symbol(
    &self,
    name: Atom,
    self_linker_module: &mut LinkerModule,
    symbols: &mut Symbols,
  ) -> SymbolRef {
    let local_symbol_ref = symbols.create_symbol(self.id, name);
    self_linker_module.facade_stmt_infos.push(StmtInfo {
      // FIXME: should store the symbol in `used_symbols` instead of `declared_symbols`.
      // The deconflict for runtime symbols would be handled in the deconflict on cross-chunk-imported
      // symbols
      declared_symbols: vec![local_symbol_ref],
      ..Default::default()
    });
    local_symbol_ref
  }

  pub fn get_star_exports_modules(&self) -> impl Iterator<Item = ModuleId> + '_ {
    self.star_exports.iter().map(|rec_id| {
      let rec = &self.import_records[*rec_id];
      rec.resolved_module
    })
  }

  pub fn get_import_module_by_span(&self, span: Span) -> ModuleId {
    let record = &self.import_records[self.imports[&span]];
    record.resolved_module
  }
}
