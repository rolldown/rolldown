use std::convert::Infallible;

use rolldown_common::{
  DeclaredSymbols, ExportsKind, ImportRecordMeta, ModuleTable, OutputFormat, StmtInfo,
  StmtInfoMeta, SymbolRef, TaggedSymbolRef,
};
use rolldown_utils::pass::{Pass, PassCtx, RawPassOutput, RunToken};

use crate::type_alias::IndexStmtInfos;

use super::{
  CreateSyntheticExportStatementsPass, ExternalStarExports, ModuleFormats, ResolvedExports,
  ShimmedMissingExports,
};

// See internal-docs/pass-based-pipeline/implementation.md.

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct CreateSyntheticExportStatementsInput<'a> {
  pub module_table: &'a ModuleTable,
  pub module_formats: &'a ModuleFormats,
  pub resolved_exports: &'a ResolvedExports,
  pub shimmed_missing_exports: &'a ShimmedMissingExports,
  pub external_star_exports: &'a ExternalStarExports,
  pub export_all_helper: SymbolRef,
  pub re_export_helper: SymbolRef,
  pub output_format: OutputFormat,
  pub generated_code_symbols: bool,
}

impl Pass for CreateSyntheticExportStatementsPass {
  type InputRead<'a> = CreateSyntheticExportStatementsInput<'a>;
  type InputOwned = IndexStmtInfos;
  type OutputRead = ();
  type OutputOwned = IndexStmtInfos;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    mut stmt_infos: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let CreateSyntheticExportStatementsInput {
      module_table,
      module_formats,
      resolved_exports,
      shimmed_missing_exports,
      external_star_exports,
      export_all_helper,
      re_export_helper,
      output_format,
      generated_code_symbols,
    } = input;
    let module_count = module_table.modules.len();
    for (domain, actual) in [
      ("synthetic statements / formats", module_formats.module_count()),
      ("synthetic statements / resolved exports", resolved_exports.module_count()),
      ("synthetic statements / shims", shimmed_missing_exports.module_count()),
      ("synthetic statements / external stars", external_star_exports.module_count()),
      ("synthetic statements / statements", stmt_infos.len()),
    ] {
      std::assert_eq!(actual, module_count, "{domain} layout must match modules");
    }

    for (module_idx, module) in module_table.modules.iter_enumerated() {
      let valid = match module {
        rolldown_common::Module::Normal(module) => {
          module_formats.get(module_idx).is_some()
            && resolved_exports.has_normal_slot(module_idx)
            && shimmed_missing_exports.get(module_idx).is_some()
            && external_star_exports
              .get(module_idx)
              .iter()
              .all(|record_idx| module.import_records.get(*record_idx).is_some())
        }
        rolldown_common::Module::External(_) => {
          module_formats.get(module_idx).is_none()
            && !resolved_exports.has_normal_slot(module_idx)
            && shimmed_missing_exports.get(module_idx).is_none()
            && external_star_exports.get(module_idx).is_empty()
        }
      };
      std::assert!(valid, "synthetic-statement slot shape must match module {module_idx:?}");
    }

    for (module_idx, module) in module_table.modules.iter_enumerated() {
      let Some(module) = module.as_normal() else {
        continue;
      };
      let Some(shimmed_missing_exports) = shimmed_missing_exports.get(module_idx) else {
        std::unreachable!("validated normal modules must have missing-export shim slots");
      };
      let stmt_infos = &mut stmt_infos[module_idx];

      for symbol_ref in shimmed_missing_exports.values() {
        let mut declared_symbols = DeclaredSymbols::new();
        declared_symbols.push(TaggedSymbolRef::normal(*symbol_ref));
        stmt_infos.add_stmt_info(StmtInfo {
          declared_symbols,
          referenced_symbols: Vec::new(),
          eval_flags: false.into(),
          import_records: Vec::new(),
          meta: StmtInfoMeta::default(),
          ..Default::default()
        });
      }

      if module_formats.get(module_idx) != Some(ExportsKind::Esm) {
        continue;
      }

      let mut referenced_symbols = Vec::new();
      let mut declared_symbols = DeclaredSymbols::new();
      if !resolved_exports.canonical_exports_is_empty(module_idx) || generated_code_symbols {
        referenced_symbols.push(export_all_helper.into());
        referenced_symbols.extend(
          resolved_exports
            .canonical_exports(module_idx, false)
            .map(|(_, export)| export.symbol_ref.into()),
        );
      }
      let external_stars = external_star_exports.get(module_idx);
      if !external_stars.is_empty() {
        referenced_symbols.push(re_export_helper.into());
        if std::matches!(output_format, OutputFormat::Esm) {
          for record_idx in external_stars {
            let record = &module.import_records[*record_idx];
            if record.meta.contains(ImportRecordMeta::EntryLevelExternal) {
              continue;
            }
            referenced_symbols.push(record.namespace_ref.into());
            declared_symbols.push(TaggedSymbolRef::normal(record.namespace_ref));
          }
        }
      }
      declared_symbols.push(TaggedSymbolRef::normal(module.namespace_object_ref));
      stmt_infos.replace_namespace_stmt_info(StmtInfo {
        declared_symbols,
        referenced_symbols,
        eval_flags: false.into(),
        import_records: Vec::new(),
        meta: StmtInfoMeta::default(),
        ..Default::default()
      });
    }

    Ok(token.finish((), stmt_infos))
  }
}

#[cfg(test)]
mod tests {
  use oxc::{semantic::Scoping, span::Span};
  use oxc_index::IndexVec;
  use rolldown_common::{
    ExportsKind, ImportKind, ImportRecordIdx, ImportRecordMeta, LocalExport, OutputFormat,
    StmtInfoIdx, StmtInfos, SymbolOrMemberExprRef, SymbolRefDb, SymbolRefDbForModule,
  };
  use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};

  use super::super::{
    CollectExternalStarExportsPass, CollectResolvedExportsPass,
    CreateSyntheticExportStatementsInput, CreateSyntheticExportStatementsPass,
    FinalizeResolvedExportsPass,
    bind_imports::test_support::shimmed_missing_exports,
    determine_module_formats::test_support::module_formats,
    test_utils::{external_module, module_idx, module_table, normal_module},
  };

  #[test]
  fn preserves_shim_identity_and_namespace_reference_order_without_touching_non_esm_slots() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(2), Span::new(1, 2)),
          (ImportKind::Import, Some(2), Span::new(2, 3)),
          (ImportKind::Import, Some(2), Span::new(3, 4)),
        ],
      ),
      normal_module(1, false, Vec::new()),
      external_module(2, "external"),
      normal_module(3, false, Vec::new()),
    ]);
    let mut symbols = SymbolRefDb::new();
    for module_index in [0, 1, 3] {
      let module_idx = module_idx(module_index);
      let scoping = Scoping::default();
      let root_scope_id = scoping.root_scope_id();
      symbols
        .store_local_db(module_idx, SymbolRefDbForModule::new(scoping, module_idx, root_scope_id));
      assert_eq!(
        symbols.create_facade_root_symbol_ref(module_idx, "namespace"),
        modules[module_idx].as_normal().expect("normal module").namespace_object_ref
      );
    }
    let alpha = symbols.create_facade_root_symbol_ref(module_idx(0), "alpha");
    let commonjs = symbols.create_facade_root_symbol_ref(module_idx(0), "commonjs");
    let shim = symbols.create_facade_root_symbol_ref(module_idx(0), "shim");
    let cjs_shim = symbols.create_facade_root_symbol_ref(module_idx(1), "cjs_shim");
    let export_all_helper = symbols.create_facade_root_symbol_ref(module_idx(0), "export_all");
    let re_export_helper = symbols.create_facade_root_symbol_ref(module_idx(0), "re_export");
    let external_namespace = symbols.create_facade_root_symbol_ref(module_idx(0), "external_ns");
    let flattened_external_namespace =
      symbols.create_facade_root_symbol_ref(module_idx(0), "flattened_external_ns");
    let second_external_namespace =
      symbols.create_facade_root_symbol_ref(module_idx(0), "second_external_ns");
    let module = modules[module_idx(0)].as_normal_mut().expect("ESM module");
    module.named_exports.extend([
      (
        "z-commonjs".into(),
        LocalExport { span: Span::new(4, 5), referenced: commonjs, came_from_commonjs: true },
      ),
      (
        "alpha".into(),
        LocalExport { span: Span::new(3, 4), referenced: alpha, came_from_commonjs: false },
      ),
    ]);
    module.import_records[ImportRecordIdx::from_usize(0)].namespace_ref = external_namespace;
    module.import_records[ImportRecordIdx::from_usize(0)]
      .meta
      .insert(ImportRecordMeta::IsExportStar);
    module.import_records[ImportRecordIdx::from_usize(1)].namespace_ref =
      flattened_external_namespace;
    module.import_records[ImportRecordIdx::from_usize(1)]
      .meta
      .insert(ImportRecordMeta::IsExportStar | ImportRecordMeta::EntryLevelExternal);
    module.import_records[ImportRecordIdx::from_usize(2)].namespace_ref = second_external_namespace;
    module.import_records[ImportRecordIdx::from_usize(2)]
      .meta
      .insert(ImportRecordMeta::IsExportStar);

    let mut pipeline = PassPipelineCtx::new();
    let (_, resolved_draft) =
      run_infallible_pass(CollectResolvedExportsPass, &mut pipeline, &modules, ());
    let (_, resolved_exports) =
      run_infallible_pass(FinalizeResolvedExportsPass, &mut pipeline, &symbols, resolved_draft);
    let (_, external_stars) =
      run_infallible_pass(CollectExternalStarExportsPass, &mut pipeline, &modules, ());
    let formats = module_formats(&[
      Some(ExportsKind::Esm),
      Some(ExportsKind::CommonJs),
      None,
      Some(ExportsKind::Esm),
    ]);
    let shims = shimmed_missing_exports([
      Some(vec![("shim".into(), shim)]),
      Some(vec![("cjs-shim".into(), cjs_shim)]),
      None,
      Some(Vec::new()),
    ]);
    let statements = modules.modules.iter().map(|_| StmtInfos::new()).collect::<IndexVec<_, _>>();

    let (_, statements) = run_infallible_pass(
      CreateSyntheticExportStatementsPass,
      &mut pipeline,
      CreateSyntheticExportStatementsInput {
        module_table: &modules,
        module_formats: &formats,
        resolved_exports: &resolved_exports,
        shimmed_missing_exports: &shims,
        external_star_exports: &external_stars,
        export_all_helper,
        re_export_helper,
        output_format: OutputFormat::Esm,
        generated_code_symbols: true,
      },
      statements,
    );

    let esm_statements = &statements[module_idx(0)];
    assert_eq!(esm_statements.len(), 2);
    assert_eq!(
      esm_statements
        .get(StmtInfos::NAMESPACE_STMT_IDX)
        .declared_symbols
        .iter()
        .map(rolldown_common::TaggedSymbolRef::inner)
        .collect::<Vec<_>>(),
      [
        external_namespace,
        second_external_namespace,
        modules[module_idx(0)].as_normal().unwrap().namespace_object_ref,
      ]
    );
    assert_eq!(
      esm_statements
        .get(StmtInfos::NAMESPACE_STMT_IDX)
        .referenced_symbols
        .iter()
        .map(|reference| match reference {
          SymbolOrMemberExprRef::Symbol(symbol_ref) => *symbol_ref,
          SymbolOrMemberExprRef::MemberExpr(_) => panic!("synthetic namespace uses bare symbols"),
        })
        .collect::<Vec<_>>(),
      [export_all_helper, alpha, re_export_helper, external_namespace, second_external_namespace,]
    );
    let shim_statement = esm_statements.get(rolldown_common::StmtInfoIdx::from_usize(1));
    assert_eq!(shim_statement.declared_symbols[0].inner(), shim);
    assert_eq!(
      esm_statements.declared_stmts_by_symbol(&shim),
      [rolldown_common::StmtInfoIdx::from_usize(1)]
    );
    assert_eq!(statements[module_idx(1)].len(), 2);
    assert_eq!(
      statements[module_idx(1)].get(StmtInfoIdx::from_usize(1)).declared_symbols[0].inner(),
      cjs_shim
    );
    assert_eq!(
      statements[module_idx(1)].get(StmtInfos::NAMESPACE_STMT_IDX).declared_symbols.len(),
      0
    );
    assert_eq!(
      statements[module_idx(3)]
        .get(StmtInfos::NAMESPACE_STMT_IDX)
        .referenced_symbols
        .iter()
        .map(|reference| match reference {
          SymbolOrMemberExprRef::Symbol(symbol_ref) => *symbol_ref,
          SymbolOrMemberExprRef::MemberExpr(_) => panic!("synthetic namespace uses bare symbols"),
        })
        .collect::<Vec<_>>(),
      [export_all_helper]
    );

    let cjs_statements =
      modules.modules.iter().map(|_| StmtInfos::new()).collect::<IndexVec<_, _>>();
    let (_, cjs_statements) = run_infallible_pass(
      CreateSyntheticExportStatementsPass,
      &mut pipeline,
      CreateSyntheticExportStatementsInput {
        module_table: &modules,
        module_formats: &formats,
        resolved_exports: &resolved_exports,
        shimmed_missing_exports: &shims,
        external_star_exports: &external_stars,
        export_all_helper,
        re_export_helper,
        output_format: OutputFormat::Cjs,
        generated_code_symbols: false,
      },
      cjs_statements,
    );
    let cjs_namespace = cjs_statements[module_idx(0)].get(StmtInfos::NAMESPACE_STMT_IDX);
    assert_eq!(
      cjs_namespace
        .referenced_symbols
        .iter()
        .map(|reference| match reference {
          SymbolOrMemberExprRef::Symbol(symbol_ref) => *symbol_ref,
          SymbolOrMemberExprRef::MemberExpr(_) => panic!("synthetic namespace uses bare symbols"),
        })
        .collect::<Vec<_>>(),
      [export_all_helper, alpha, re_export_helper]
    );
    assert_eq!(
      cjs_namespace
        .declared_symbols
        .iter()
        .map(rolldown_common::TaggedSymbolRef::inner)
        .collect::<Vec<_>>(),
      [modules[module_idx(0)].as_normal().unwrap().namespace_object_ref]
    );
    assert!(pipeline.into_diagnostics().is_empty());
  }

  #[test]
  fn rejects_statement_layout_mismatch_before_mutation() {
    let modules = module_table(vec![normal_module(0, false, Vec::new())]);
    let symbols = SymbolRefDb::new();
    let mut pipeline = PassPipelineCtx::new();
    let (_, resolved_draft) =
      run_infallible_pass(CollectResolvedExportsPass, &mut pipeline, &modules, ());
    let (_, resolved_exports) =
      run_infallible_pass(FinalizeResolvedExportsPass, &mut pipeline, &symbols, resolved_draft);
    let (_, external_stars) =
      run_infallible_pass(CollectExternalStarExportsPass, &mut pipeline, &modules, ());
    let formats = module_formats(&[Some(ExportsKind::Esm)]);
    let shims = shimmed_missing_exports([Some(Vec::new())]);
    let helper_ref = modules[module_idx(0)].as_normal().unwrap().namespace_object_ref;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      run_infallible_pass(
        CreateSyntheticExportStatementsPass,
        &mut pipeline,
        CreateSyntheticExportStatementsInput {
          module_table: &modules,
          module_formats: &formats,
          resolved_exports: &resolved_exports,
          shimmed_missing_exports: &shims,
          external_star_exports: &external_stars,
          export_all_helper: helper_ref,
          re_export_helper: helper_ref,
          output_format: OutputFormat::Esm,
          generated_code_symbols: false,
        },
        IndexVec::new(),
      )
    }));
    assert!(result.is_err());
  }
}
