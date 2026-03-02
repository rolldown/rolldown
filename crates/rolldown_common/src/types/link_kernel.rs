use std::path::PathBuf;

use oxc::span::Span;
use oxc_index::IndexVec;
use oxc_module_graph::{self as oxc_mg, CompactString, ModuleGraph};

use crate::{
  EcmaModuleAstUsage, EcmaViewMeta, EntryPoint, ImportRecordMeta, Module, ModuleIdx, ModuleTable,
  SymbolRef, side_effects::DeterminedSideEffects,
};

use super::module_payload::ModulePayload;
use super::symbol_extras::SymbolExtrasForModule;

/// Kernel that bridges scan-stage output to link-stage input.
///
/// Wraps the generic `oxc_module_graph::ModuleGraph` alongside
/// Rolldown-specific sidecars for heavy module data and per-symbol metadata.
#[derive(Debug, Default, Clone)]
pub struct LinkKernel {
  /// Generic link-time graph — the canonical source of truth for
  /// module topology, import/export records, and symbol linking.
  pub graph: ModuleGraph,
  /// Rolldown-only per-module payload (source text, AST, codegen state, etc.).
  pub module_payloads: IndexVec<ModuleIdx, Option<ModulePayload>>,
  /// Rolldown-only per-module symbol metadata (scopes, flags, namespace aliases, etc.).
  pub symbol_extras: IndexVec<ModuleIdx, Option<SymbolExtrasForModule>>,
}

impl LinkKernel {
  pub fn new() -> Self {
    Self::default()
  }

  /// Build a `LinkKernel` from scan-stage output.
  ///
  /// Converts Rolldown's `ModuleTable` into the canonical
  /// `oxc_module_graph` representation. The graph uses the same indices
  /// as the module table, so algorithms can operate without translation.
  ///
  /// Fields computed during linking (`has_dynamic_exports`,
  /// `is_tla_or_contains_tla`) are left at their defaults — the
  /// link stage passes will set them to correct values.
  pub fn from_module_table(
    module_table: &ModuleTable,
    entries: &[EntryPoint],
    runtime_id: ModuleIdx,
  ) -> Self {
    let mut lk = Self {
      graph: ModuleGraph::without_symbols(),
      ..Self::default()
    };
    populate_graph(&mut lk.graph, module_table, entries, runtime_id);
    lk
  }
}

// --- Graph population helpers ---

fn to_oxc_module_idx(idx: ModuleIdx) -> oxc_mg::types::ModuleIdx {
  oxc_mg::types::ModuleIdx::from_usize(idx.index())
}

fn to_oxc_symbol_ref(sr: SymbolRef) -> oxc_mg::types::SymbolRef {
  oxc_mg::types::SymbolRef::new(to_oxc_module_idx(sr.owner), sr.symbol)
}

fn to_oxc_import_kind(kind: crate::ImportKind) -> oxc_mg::types::ImportKind {
  match kind {
    crate::ImportKind::Import
    | crate::ImportKind::AtImport
    | crate::ImportKind::UrlImport
    | crate::ImportKind::NewUrl => oxc_mg::types::ImportKind::Static,
    crate::ImportKind::DynamicImport => oxc_mg::types::ImportKind::Dynamic,
    crate::ImportKind::Require => oxc_mg::types::ImportKind::Require,
    crate::ImportKind::HotAccept => oxc_mg::types::ImportKind::HotAccept,
  }
}

fn to_oxc_exports_kind(kind: crate::ExportsKind) -> oxc_mg::types::ExportsKind {
  match kind {
    crate::ExportsKind::Esm => oxc_mg::types::ExportsKind::Esm,
    crate::ExportsKind::CommonJs => oxc_mg::types::ExportsKind::CommonJs,
    crate::ExportsKind::None => oxc_mg::types::ExportsKind::None,
  }
}

fn to_oxc_side_effects(se: &DeterminedSideEffects) -> oxc_mg::SideEffects {
  match se {
    DeterminedSideEffects::Analyzed(true) | DeterminedSideEffects::UserDefined(true) => {
      oxc_mg::SideEffects::True
    }
    DeterminedSideEffects::Analyzed(false) | DeterminedSideEffects::UserDefined(false) => {
      oxc_mg::SideEffects::False
    }
    DeterminedSideEffects::NoTreeshake => oxc_mg::SideEffects::NoTreeshake,
  }
}

fn to_oxc_import_record_meta(meta: ImportRecordMeta) -> oxc_mg::types::ImportRecordMeta {
  let mut result = oxc_mg::types::ImportRecordMeta::empty();
  if meta.contains(ImportRecordMeta::IsExportStar) {
    result |= oxc_mg::types::ImportRecordMeta::IS_EXPORT_STAR;
  }
  result
}

fn populate_graph(
  graph: &mut ModuleGraph,
  module_table: &ModuleTable,
  entries: &[EntryPoint],
  runtime_id: ModuleIdx,
) {
  // Pre-allocate all module slots (keeps indices aligned).
  for _ in 0..module_table.modules.len() {
    graph.alloc_module_idx();
  }

  for (rd_idx, module) in module_table.modules.iter_enumerated() {
    let oxc_idx = to_oxc_module_idx(rd_idx);

    match module {
      Module::Normal(m) => {
        let mut import_records =
          Vec::with_capacity(m.import_records.len());
        for rec in m.import_records.iter() {
          import_records.push(oxc_mg::types::ResolvedImportRecord {
            resolved_module: rec.resolved_module.map(to_oxc_module_idx),
            kind: to_oxc_import_kind(rec.kind),
            namespace_ref: to_oxc_symbol_ref(rec.namespace_ref),
            meta: to_oxc_import_record_meta(rec.meta),
            is_type_only: false,
          });
        }

        let star_count = m.import_records.iter()
          .filter(|rec| rec.meta.contains(ImportRecordMeta::IsExportStar))
          .count();
        let mut star_export_entries =
          Vec::with_capacity(star_count + usize::from(m.ast_usage.contains(EcmaModuleAstUsage::IsCjsReexport)));
        for rec in m.import_records.iter().filter(|rec| rec.meta.contains(ImportRecordMeta::IsExportStar)) {
          star_export_entries.push(oxc_mg::types::StarExportEntry {
            module_request: CompactString::default(),
            resolved_module: rec.resolved_module.map(to_oxc_module_idx),
            span: Span::default(),
          });
        }

        if m.ast_usage.contains(EcmaModuleAstUsage::IsCjsReexport) {
          if let Some(first_rec) = m.import_records.first() {
            if let Some(target) = first_rec.resolved_module {
              star_export_entries.push(oxc_mg::types::StarExportEntry {
                module_request: CompactString::default(),
                resolved_module: Some(to_oxc_module_idx(target)),
                span: Span::default(),
              });
            }
          }
        }

        // Path unused — Rolldown creates wrapper symbols itself (skip_symbol_creation: true),
        // so the graph's path-based naming (init_xxx/require_xxx) is never triggered.
        let mut oxc_module = oxc_mg::NormalModule::new(
          oxc_idx,
          PathBuf::new(),
          to_oxc_symbol_ref(m.default_export_ref),
          to_oxc_symbol_ref(m.namespace_object_ref),
        );
        oxc_module.has_module_syntax = m.def_format.is_esm();
        oxc_module.exports_kind = to_oxc_exports_kind(m.exports_kind);
        oxc_module.has_top_level_await =
          m.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait);
        oxc_module.side_effects = to_oxc_side_effects(&m.side_effects);
        oxc_module.has_lazy_export = m.meta.has_lazy_export();
        oxc_module.execution_order_sensitive =
          m.meta.contains(EcmaViewMeta::ExecutionOrderSensitive);
        oxc_module.import_records = import_records;
        oxc_module.star_export_entries = star_export_entries;
        graph.add_normal_module(oxc_module);
      }
      Module::External(ext) => {
        graph.add_external_module(oxc_mg::ExternalModule {
          idx: oxc_idx,
          specifier: ext.name.as_str().into(),
          side_effects: to_oxc_side_effects(&ext.side_effects),
          namespace_ref: to_oxc_symbol_ref(ext.namespace_ref),
        });
      }
    }
  }

  // Set entries and runtime. Deduplicate while preserving order
  // (multiple entry points may share a module index).
  let mut seen = rustc_hash::FxHashSet::default();
  let entry_indices: Vec<oxc_mg::types::ModuleIdx> = entries
    .iter()
    .filter_map(|ep| {
      let oxc_idx = to_oxc_module_idx(ep.idx);
      seen.insert(oxc_idx).then_some(oxc_idx)
    })
    .collect();
  graph.set_entries(entry_indices);
  graph.set_runtime(to_oxc_module_idx(runtime_id));
}
