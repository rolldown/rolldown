use std::fmt::Debug;

use index_vec::IndexVec;
use oxc::{
  semantic::{ScopeTree, SymbolId},
  span::{Atom, Span},
  syntax::symbol,
};
use rolldown_common::{
  ExportsKind, ImportRecord, ImportRecordId, LocalOrReExport, ModuleId, ModuleType, NamedImport,
  ResolvedExport, ResourceId, StmtInfo, StmtInfos, SymbolRef, WrapKind,
};
use rolldown_oxc::OxcProgram;
use rustc_hash::{FxHashMap, FxHashSet};
use string_wizard::MagicString;

use crate::bundler::{
  graph::{
    linker::{LinkingInfo, LinkingInfoVec},
    symbols::Symbols,
  },
  visitors::{
    cjs_renderer::CjsRenderer, esm_renderer::EsmRenderer, wrapped_esm_renderer::WrappedEsmRenderer,
    RendererBase,
  },
};

use super::{Module, ModuleRenderContext, ModuleVec};

#[derive(Debug)]
pub struct NormalModule {
  pub exec_order: u32,
  pub id: ModuleId,
  pub is_entry: bool,
  pub resource_id: ResourceId,
  pub module_type: ModuleType,
  pub namespace_symbol: SymbolRef,
  pub ast: OxcProgram,
  pub named_imports: FxHashMap<SymbolId, NamedImport>,
  pub named_exports: FxHashMap<Atom, LocalOrReExport>,
  /// `stmt_infos[0]` represents the namespace binding statement
  pub stmt_infos: StmtInfos,
  pub import_records: IndexVec<ImportRecordId, ImportRecord>,
  pub imports: FxHashMap<Span, ImportRecordId>,
  // [[StarExportEntries]] in https://tc39.es/ecma262/#sec-source-text-module-records
  pub star_exports: Vec<ImportRecordId>,
  pub exports_kind: ExportsKind,
  pub scope: ScopeTree,
  pub default_export_symbol: Option<SymbolId>,
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
  pub fn is_namespace_referenced(&self) -> bool {
    self.stmt_infos.get(0.into()).is_included
  }

  #[allow(clippy::needless_pass_by_value)]
  pub fn render(&self, ctx: ModuleRenderContext<'_>) -> Option<MagicString<'static>> {
    let source = self.ast.source();
    // FIXME: should not clone here
    let mut source = MagicString::new(source.to_string());
    let self_linking_info = &ctx.graph.linking_infos[self.id];
    let base = RendererBase::new(
      ctx.graph,
      ctx.canonical_names,
      &mut source,
      ctx.chunk_graph,
      self,
      self_linking_info,
    );

    match &self_linking_info.wrap_kind {
      WrapKind::None => EsmRenderer::new(base).apply(),
      WrapKind::CJS => CjsRenderer::new(base).apply(),
      WrapKind::ESM => WrappedEsmRenderer::new(base).apply(),
    }

    source.prepend(format!("// {}\n", self.resource_id.prettify()));

    // TODO trim
    if source.len() == 0 {
      None
    } else {
      Some(source)
    }
  }

  pub fn add_initial_resolved_exports(
    &self,
    self_linking_info: &mut LinkingInfo,
    symbols: &mut Symbols,
  ) {
    self.named_exports.iter().for_each(|(name, local_or_re_export)| {
      let resolved_export = match local_or_re_export {
        LocalOrReExport::Local(local) => {
          ResolvedExport { symbol_ref: local.referenced, export_from: None }
        }
        LocalOrReExport::Re(re) => {
          let symbol_ref = self.generate_local_symbol(name.clone(), self_linking_info, symbols);
          self_linking_info.export_from_map.insert(
            name.clone(),
            NamedImport {
              imported: re.imported.clone(),
              is_imported_star: re.is_imported_star,
              imported_as: symbol_ref,
              record_id: re.record_id,
            },
          );
          let rec = &self.import_records[re.record_id];
          ResolvedExport { symbol_ref, export_from: Some(rec.resolved_module) }
        }
      };
      self_linking_info.resolved_exports.insert(name.clone(), resolved_export);
    });
  }

  pub fn add_resolved_exports_for_export_star(
    &self,
    id: ModuleId,
    linking_infos: &mut LinkingInfoVec,
    modules: &ModuleVec,
    module_stack: &mut Vec<ModuleId>,
  ) {
    if module_stack.contains(&self.id) {
      return;
    }
    module_stack.push(self.id);

    for module_id in self.get_star_exports_modules() {
      let importee = &modules[module_id];
      match importee {
        Module::Normal(importee) => {
          // Export star from commonjs will be resolved at runtime
          if importee.exports_kind == ExportsKind::CommonJs {
            continue;
          }

          importee.named_exports.iter().for_each(|(name, _)| {
            // ES6 export star ignore default export
            if name.as_str() == "default" {
              return;
            }

            // This export star is shadowed if any file in the stack has a matching real named export
            for id in module_stack.iter() {
              let module = &modules[*id];
              match module {
                Module::Normal(module) => {
                  if module.named_exports.contains_key(name) {
                    return;
                  }
                }
                Module::External(_) => {}
              }
            }

            let resolved_export = *linking_infos[importee.id].resolved_exports.get(name).unwrap();

            let linking_info = &mut linking_infos[id];

            match linking_info.resolved_exports.entry(name.clone()) {
              std::collections::hash_map::Entry::Occupied(entry) => {
                if entry.get().symbol_ref != resolved_export.symbol_ref {
                  // potentially ambiguous export
                  match linking_info.potentially_ambiguous_exports.entry(name.clone()) {
                    std::collections::hash_map::Entry::Occupied(entry) => {
                      entry.into_mut().push(resolved_export.symbol_ref);
                    }
                    std::collections::hash_map::Entry::Vacant(ambiguous_exports_entry) => {
                      ambiguous_exports_entry.insert(vec![resolved_export.symbol_ref]);
                    }
                  }
                }
              }
              std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(resolved_export);
              }
            }
          });

          importee.add_resolved_exports_for_export_star(id, linking_infos, modules, module_stack);
        }
        Module::External(_) => {
          // unimplemented!("handle external module")
        }
      }
    }
    module_stack.remove(module_stack.len() - 1);
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
        .star_export_modules()
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
      for module_id in self.star_export_modules() {
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
        .star_export_modules()
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
    let mut queue = self.star_export_modules().collect::<Vec<_>>();

    while let Some(importee_id) = queue.pop() {
      if !visited.contains(&importee_id) {
        visited.insert(importee_id);
        resolved.push(importee_id);
        let importee = &modules[importee_id];
        match importee {
          Module::Normal(importee) => queue.extend(importee.star_export_modules()),
          Module::External(_) => {}
        }
      }
    }

    resolved
  }

  pub fn create_wrap_symbol(&self, self_linking_info: &mut LinkingInfo, symbols: &mut Symbols) {
    if self_linking_info.wrap_symbol.is_none() {
      let name = format!(
        "{}_{}",
        if self.exports_kind == ExportsKind::CommonJs { "require" } else { "init" },
        self.resource_id.generate_unique_name()
      )
      .into();
      let symbol = symbols.create_symbol(self.id, name).symbol;
      self_linking_info.wrap_symbol = Some((self.id, symbol).into());
      self_linking_info
        .facade_stmt_infos
        .push(StmtInfo { declared_symbols: vec![(self.id, symbol).into()], ..Default::default() });
    }
  }

  pub fn reference_symbol_in_facade_stmt_infos(
    &self,
    other_module_symbol_ref: SymbolRef,
    self_linking_info: &mut LinkingInfo,
    _symbols: &mut Symbols,
  ) {
    debug_assert!(other_module_symbol_ref.owner != self.id);

    self_linking_info.facade_stmt_infos.push(StmtInfo {
      declared_symbols: vec![],
      // Since the facade symbol is used, it should be referenced. This will be used to
      // create correct cross-chunk links
      referenced_symbols: vec![other_module_symbol_ref],
      ..Default::default()
    });
  }

  pub fn star_export_modules(&self) -> impl Iterator<Item = ModuleId> + '_ {
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
