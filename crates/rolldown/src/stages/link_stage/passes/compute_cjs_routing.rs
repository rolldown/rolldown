use std::convert::Infallible;

use oxc_index::IndexVec;
use rolldown_common::{ExportsKind, ModuleIdx, ModuleTable, SymbolRef};
use rolldown_utils::{
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
  rayon::{IntoParallelIterator, ParallelIterator},
};
use rustc_hash::FxHashMap;

use super::{ComputeCjsRoutingPass, DynamicExports, ModuleFormats};

// See internal-docs/pass-based-pipeline/implementation.md.

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct ComputeCjsRoutingInput<'a> {
  pub module_table: &'a ModuleTable,
  pub module_formats: &'a ModuleFormats,
  pub dynamic_exports: &'a DynamicExports,
}

enum RelationWithCommonJs {
  CommonJs,
  IndirectDependOnCommonJs,
}

struct CjsRoutesForImporter {
  named_imports: FxHashMap<SymbolRef, ModuleIdx>,
  import_record_namespaces: FxHashMap<SymbolRef, ModuleIdx>,
}

pub(in crate::stages::link_stage) struct CjsRoutingDraft {
  module_count: usize,
  importers: FxHashMap<ModuleIdx, CjsRoutesForImporter>,
}

impl CjsRoutingDraft {
  pub(in crate::stages::link_stage) fn module_count(&self) -> usize {
    self.module_count
  }

  pub(in crate::stages::link_stage) fn named_target(
    &self,
    importer: ModuleIdx,
    symbol_ref: SymbolRef,
  ) -> Option<ModuleIdx> {
    self.importers.get(&importer).and_then(|slot| slot.named_imports.get(&symbol_ref)).copied()
  }

  pub(in crate::stages::link_stage) fn namespace_target(
    &self,
    importer: ModuleIdx,
    symbol_ref: SymbolRef,
  ) -> Option<ModuleIdx> {
    self
      .importers
      .get(&importer)
      .and_then(|slot| slot.import_record_namespaces.get(&symbol_ref))
      .copied()
  }

  pub(in crate::stages::link_stage) fn routes_for(
    &self,
    importer: ModuleIdx,
  ) -> impl Iterator<Item = (&SymbolRef, &ModuleIdx)> {
    self
      .importers
      .get(&importer)
      .into_iter()
      .flat_map(|slot| slot.named_imports.iter().chain(slot.import_record_namespaces.iter()))
  }

  pub(in crate::stages::link_stage) fn finalize(self) -> CjsRoutingFinal {
    CjsRoutingFinal {
      module_count: self.module_count,
      importers: self
        .importers
        .into_iter()
        .filter_map(|(module_idx, routes)| {
          (!routes.import_record_namespaces.is_empty())
            .then_some((module_idx, routes.import_record_namespaces))
        })
        .collect(),
    }
  }
}

pub(in crate::stages::link_stage) struct CjsRoutingFinal {
  module_count: usize,
  importers: FxHashMap<ModuleIdx, FxHashMap<SymbolRef, ModuleIdx>>,
}

impl CjsRoutingFinal {
  pub(in crate::stages::link_stage) fn module_count(&self) -> usize {
    self.module_count
  }

  pub(in crate::stages::link_stage) fn into_importers(
    self,
  ) -> FxHashMap<ModuleIdx, FxHashMap<SymbolRef, ModuleIdx>> {
    self.importers
  }
}

impl Pass for ComputeCjsRoutingPass {
  type InputRead<'a> = ComputeCjsRoutingInput<'a>;
  type InputOwned = ();
  type OutputRead = ();
  type OutputOwned = CjsRoutingDraft;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    (): Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let ComputeCjsRoutingInput { module_table, module_formats, dynamic_exports } = input;
    std::assert_eq!(
      module_formats.module_count(),
      module_table.modules.len(),
      "module-format layout must match modules before CJS routing"
    );
    std::assert_eq!(
      dynamic_exports.module_count(),
      module_table.modules.len(),
      "dynamic-export layout must match modules before CJS routing"
    );
    let relations = module_table
      .modules
      .iter_enumerated()
      .map(|(module_idx, module)| {
        module.as_normal().and_then(|_| {
          if module_formats.get(module_idx) == Some(ExportsKind::CommonJs) {
            Some(RelationWithCommonJs::CommonJs)
          } else if dynamic_exports.contains(module_idx) {
            Some(RelationWithCommonJs::IndirectDependOnCommonJs)
          } else {
            None
          }
        })
      })
      .collect::<IndexVec<ModuleIdx, _>>();

    let importers = (0..module_table.modules.len())
      .into_par_iter()
      .filter_map(|index| {
        let module_idx = ModuleIdx::new(index);
        let module = module_table[module_idx].as_normal()?;
        let mut named_imports = FxHashMap::default();
        let mut import_record_namespaces = FxHashMap::default();

        for named_import in module.named_imports.values() {
          let record = &module.import_records[named_import.record_idx];
          if let Some(importee_idx) = record.resolved_module
            && relations[importee_idx].is_some()
          {
            named_imports.insert(named_import.imported_as, importee_idx);
          }
        }
        for record in &module.import_records {
          if let Some(importee_idx) = record.resolved_module
            && std::matches!(relations[importee_idx], Some(RelationWithCommonJs::CommonJs))
          {
            import_record_namespaces.insert(record.namespace_ref, importee_idx);
          }
        }

        (!named_imports.is_empty() || !import_record_namespaces.is_empty())
          .then_some((module_idx, CjsRoutesForImporter { named_imports, import_record_namespaces }))
      })
      .collect();

    Ok(token.finish((), CjsRoutingDraft { module_count: module_table.modules.len(), importers }))
  }
}

#[cfg(test)]
mod tests {
  use oxc::{semantic::SymbolId, span::Span};
  use rolldown_common::{
    ExportsKind, ImportKind, ImportRecordIdx, NamedImport, Specifier, SymbolRef,
  };
  use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};

  use super::super::{
    ComputeCjsRoutingInput, ComputeCjsRoutingPass,
    compute_dynamic_exports::test_support::dynamic_exports,
    determine_module_formats::test_support::module_formats,
    test_utils::{external_module, module_idx, module_table, normal_module},
  };

  #[test]
  fn preserves_relation_and_record_kind_boundaries_while_finalizing_namespace_routes() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(1), Span::new(1, 2)),
          (ImportKind::Import, Some(2), Span::new(2, 3)),
          (ImportKind::Import, Some(3), Span::new(3, 4)),
          (ImportKind::Import, Some(4), Span::new(4, 5)),
          (ImportKind::Import, None, Span::new(5, 6)),
          (ImportKind::Require, Some(1), Span::new(6, 7)),
        ],
      ),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
      external_module(3, "external"),
      normal_module(4, false, Vec::new()),
    ]);
    let named_cjs = SymbolRef { owner: module_idx(0), symbol: SymbolId::new(1) };
    let named_dynamic = SymbolRef { owner: module_idx(0), symbol: SymbolId::new(2) };
    let named_ordinary = SymbolRef { owner: module_idx(0), symbol: SymbolId::new(3) };
    let named_unresolved = SymbolRef { owner: module_idx(0), symbol: SymbolId::new(4) };
    let named_require = SymbolRef { owner: module_idx(0), symbol: SymbolId::new(5) };
    let cjs_namespace = SymbolRef { owner: module_idx(0), symbol: SymbolId::new(6) };
    let external_namespace = SymbolRef { owner: module_idx(0), symbol: SymbolId::new(7) };
    let ordinary_namespace = SymbolRef { owner: module_idx(0), symbol: SymbolId::new(8) };
    let unresolved_namespace = SymbolRef { owner: module_idx(0), symbol: SymbolId::new(9) };
    let require_namespace = SymbolRef { owner: module_idx(0), symbol: SymbolId::new(10) };
    let importer = modules[module_idx(0)].as_normal_mut().expect("normal importer");
    importer.import_records[ImportRecordIdx::from_usize(0)].namespace_ref = cjs_namespace;
    importer.import_records[ImportRecordIdx::from_usize(2)].namespace_ref = external_namespace;
    importer.import_records[ImportRecordIdx::from_usize(3)].namespace_ref = ordinary_namespace;
    importer.import_records[ImportRecordIdx::from_usize(4)].namespace_ref = unresolved_namespace;
    importer.import_records[ImportRecordIdx::from_usize(5)].namespace_ref = require_namespace;
    importer.named_imports.insert(
      named_cjs,
      NamedImport {
        imported: Specifier::from("value"),
        span_imported: Span::new(10, 11),
        imported_as: named_cjs,
        record_idx: ImportRecordIdx::from_usize(0),
      },
    );
    importer.named_imports.insert(
      named_dynamic,
      NamedImport {
        imported: Specifier::from("value"),
        span_imported: Span::new(11, 12),
        imported_as: named_dynamic,
        record_idx: ImportRecordIdx::from_usize(1),
      },
    );
    for (record, imported_as) in [(3, named_ordinary), (4, named_unresolved), (5, named_require)] {
      importer.named_imports.insert(
        imported_as,
        NamedImport {
          imported: Specifier::from("value"),
          span_imported: Span::new(12 + record, 13 + record),
          imported_as,
          record_idx: ImportRecordIdx::from_usize(record as usize),
        },
      );
    }

    let formats = module_formats(&[
      Some(ExportsKind::Esm),
      Some(ExportsKind::CommonJs),
      Some(ExportsKind::Esm),
      None,
      Some(ExportsKind::Esm),
    ]);
    let dynamic_exports = dynamic_exports(5, [module_idx(2), module_idx(3)]);
    let mut pipeline = PassPipelineCtx::new();
    let (_, routing) = run_infallible_pass(
      ComputeCjsRoutingPass,
      &mut pipeline,
      ComputeCjsRoutingInput {
        module_table: &modules,
        module_formats: &formats,
        dynamic_exports: &dynamic_exports,
      },
      (),
    );

    assert_eq!(routing.module_count(), 5);
    assert_eq!(routing.named_target(module_idx(0), named_cjs), Some(module_idx(1)));
    assert_eq!(routing.named_target(module_idx(0), named_dynamic), Some(module_idx(2)));
    assert_eq!(routing.named_target(module_idx(0), named_ordinary), None);
    assert_eq!(routing.named_target(module_idx(0), named_unresolved), None);
    assert_eq!(routing.named_target(module_idx(0), named_require), Some(module_idx(1)));
    assert_eq!(routing.namespace_target(module_idx(0), cjs_namespace), Some(module_idx(1)));
    assert_eq!(routing.namespace_target(module_idx(0), external_namespace), None);
    assert_eq!(routing.namespace_target(module_idx(0), ordinary_namespace), None);
    assert_eq!(routing.namespace_target(module_idx(0), unresolved_namespace), None);
    assert_eq!(routing.namespace_target(module_idx(0), require_namespace), Some(module_idx(1)));
    let final_routing = routing.finalize().into_importers();
    assert_eq!(final_routing[&module_idx(0)].len(), 2);
    assert!(!final_routing.contains_key(&module_idx(1)));
    assert!(!final_routing.contains_key(&module_idx(3)));
    assert!(pipeline.into_diagnostics().is_empty());
  }
}
