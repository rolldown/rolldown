use std::fmt::Debug;

use index_vec::IndexVec;
use oxc::{
  semantic::SymbolId,
  span::{Atom, Span},
};
use rolldown_common::{
  ExportsKind, ImportKind, ImportRecord, ImportRecordId, LocalExport, ModuleId, ModuleType,
  NamedImport, ResolvedExport, ResourceId, StmtInfo, StmtInfos, SymbolRef,
};
use rolldown_oxc::OxcProgram;
use rustc_hash::{FxHashMap, FxHashSet};
use string_wizard::MagicString;

use crate::bundler::{
  linker::linker_info::{LinkingInfo, LinkingInfoVec},
  renderer::{AstRenderContext, AstRenderer, RenderKind},
  utils::{ast_scope::AstScope, symbols::Symbols},
};

use super::{Module, ModuleRenderContext, ModuleVec};

#[derive(Debug)]
pub struct NormalModule {
  pub exec_order: u32,
  pub id: ModuleId,
  pub is_entry: bool,
  pub resource_id: ResourceId,
  pub pretty_path: String,
  /// Representative name of `FilePath`, which is created by `FilePath#representative_name` belong to `resource_id`
  pub repr_name: String,
  pub module_type: ModuleType,
  pub namespace_symbol: SymbolRef,
  pub ast: OxcProgram,
  pub named_imports: FxHashMap<SymbolId, NamedImport>,
  pub named_exports: FxHashMap<Atom, LocalExport>,
  /// `stmt_infos[0]` represents the namespace binding statement
  pub stmt_infos: StmtInfos,
  pub import_records: IndexVec<ImportRecordId, ImportRecord>,
  /// The key is the `Span` of `ImportDeclaration`, `ImportExpression`, `ExportNamedDeclaration`, `ExportAllDeclaration`
  /// and `CallExpression`(only when the callee is `require`).
  pub imports: FxHashMap<Span, ImportRecordId>,
  // [[StarExportEntries]] in https://tc39.es/ecma262/#sec-source-text-module-records
  pub star_exports: Vec<ImportRecordId>,
  pub exports_kind: ExportsKind,
  pub scope: AstScope,
  pub default_export_symbol: Option<SymbolId>,
}

impl NormalModule {
  pub fn is_namespace_referenced(&self) -> bool {
    self.stmt_infos.get(0.into()).is_included
  }

  pub fn static_imports(&self) -> impl Iterator<Item = &ImportRecord> {
    self
      .import_records
      .iter()
      .filter(|rec| matches!(rec.kind, ImportKind::Import | ImportKind::Require))
  }

  #[allow(clippy::needless_pass_by_value)]
  pub fn render(&self, ctx: ModuleRenderContext<'_>) -> Option<MagicString<'static>> {
    let source = self.ast.source();
    // FIXME: should not clone here
    let mut source = MagicString::new(source.to_string());
    let self_linking_info = &ctx.graph.linking_infos[self.id];
    let base = AstRenderContext::new(
      ctx.graph,
      ctx.canonical_names,
      &mut source,
      ctx.chunk_graph,
      self,
      self_linking_info,
    );

    let render_kind = RenderKind::from_wrap_kind(&self_linking_info.wrap_kind);
    let mut renderer = AstRenderer::new(base, &self.stmt_infos, render_kind);
    renderer.render();

    source.prepend(format!("// {}\n", self.pretty_path));

    // TODO trim
    if source.len() == 0 {
      None
    } else {
      Some(source)
    }
  }

  pub fn create_initial_resolved_exports(&self, self_linking_info: &mut LinkingInfo) {
    self.named_exports.iter().for_each(|(name, local)| {
      let resolved_export =
        ResolvedExport { symbol_ref: local.referenced, potentially_ambiguous_symbol_refs: None };
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

  pub fn declare_symbol(
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

  pub fn star_export_modules(&self) -> impl Iterator<Item = ModuleId> + '_ {
    self.star_exports.iter().map(|rec_id| {
      let rec = &self.import_records[*rec_id];
      rec.resolved_module
    })
  }

  pub fn importee_id_by_span(&self, span: Span) -> ModuleId {
    let record = &self.import_records[self.imports[&span]];
    record.resolved_module
  }
}
