use std::fmt::Debug;

use index_vec::IndexVec;
use oxc::{
  semantic::{ScopeTree, SymbolId},
  span::{Atom, Span},
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
    linker_info::{LinkingInfo, LinkingInfoVec},
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
  pub unique_name: String,
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

  pub fn create_initial_resolved_exports(
    &self,
    self_linking_info: &mut LinkingInfo,
    symbols: &mut Symbols,
  ) {
    self.named_exports.iter().for_each(|(name, local_or_re_export)| {
      let resolved_export = match local_or_re_export {
        LocalOrReExport::Local(local) => ResolvedExport {
          symbol_ref: local.referenced,
          export_from: None,
          potentially_ambiguous_symbol_refs: None,
        },
        LocalOrReExport::Re(re) => {
          let symbol_ref = self.create_local_symbol(name.clone(), self_linking_info, symbols);
          self_linking_info.export_from_map.insert(
            symbol_ref.symbol,
            NamedImport {
              imported: re.imported.clone(),
              is_imported_star: re.is_imported_star,
              imported_as: symbol_ref,
              record_id: re.record_id,
            },
          );
          let rec = &self.import_records[re.record_id];
          ResolvedExport {
            symbol_ref,
            export_from: Some(rec.resolved_module),
            potentially_ambiguous_symbol_refs: None,
          }
        }
      };
      self_linking_info.resolved_exports.insert(name.clone(), resolved_export);
    });
  }

  pub fn create_resolved_exports_for_export_star(
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

    for module_id in self.star_export_modules() {
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
            for id in &*module_stack {
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

            let resolved_export = linking_infos[importee.id].resolved_exports[name].clone();

            let linking_info = &mut linking_infos[id];

            linking_info
              .resolved_exports
              .entry(name.clone())
              .and_modify(|export| {
                if export.symbol_ref != resolved_export.symbol_ref {
                  // potentially ambiguous export
                  if let Some(potentially_ambiguous_symbol_refs) =
                    &mut export.potentially_ambiguous_symbol_refs
                  {
                    potentially_ambiguous_symbol_refs.push(resolved_export.symbol_ref);
                  } else {
                    export.potentially_ambiguous_symbol_refs =
                      Some(vec![resolved_export.symbol_ref]);
                  }
                }
              })
              .or_insert(resolved_export);
          });

          importee.create_resolved_exports_for_export_star(
            id,
            linking_infos,
            modules,
            module_stack,
          );
        }
        Module::External(_) => {
          // unimplemented!("handle external module")
        }
      }
    }
    module_stack.remove(module_stack.len() - 1);
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
        self.unique_name
      )
      .into();
      let symbol_ref = self.create_local_symbol(name, self_linking_info, symbols);
      self_linking_info.wrap_symbol = Some(symbol_ref);
    }
  }

  pub fn create_local_symbol(
    &self,
    name: Atom,
    self_linking_info: &mut LinkingInfo,
    symbols: &mut Symbols,
  ) -> SymbolRef {
    let symbol_ref = symbols.create_symbol(self.id, name);
    self_linking_info
      .facade_stmt_infos
      .push(StmtInfo { declared_symbols: vec![symbol_ref], ..Default::default() });
    symbol_ref
  }

  #[allow(clippy::map_entry, clippy::use_self)]
  pub fn create_local_symbol_for_import_cjs(
    &self,
    importee: &NormalModule,
    self_linking_info: &mut LinkingInfo,
    symbols: &mut Symbols,
  ) {
    if !self_linking_info.local_symbol_for_import_cjs.contains_key(&importee.id) {
      let name = format!("import_{}", importee.unique_name).into();
      let symbol_ref = self.create_local_symbol(name, self_linking_info, symbols);
      self_linking_info.local_symbol_for_import_cjs.insert(importee.id, symbol_ref);
    }
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
