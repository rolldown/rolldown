use std::convert::Infallible;

use oxc_index::IndexVec;
use rolldown_common::{
  EcmaModuleAstUsage, ExportsKind, ImportKind, Module, ModuleIdx, ModuleTable, OutputFormat,
  WrapKind,
};
use rolldown_utils::pass::{Pass, PassCtx, RawPassOutput, RunToken};

use super::{DetermineModuleFormatsPass, EntryPlanDraft};

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct DetermineModuleFormatsInput<'a> {
  pub module_table: &'a ModuleTable,
  pub entry_plan: &'a EntryPlanDraft,
  pub output_format: OutputFormat,
  pub code_splitting_disabled: bool,
}

pub(in crate::stages::link_stage) struct ModuleFormatsDraft {
  formats: IndexVec<ModuleIdx, Option<ExportsKind>>,
}

impl ModuleFormatsDraft {
  pub(in crate::stages::link_stage) fn get(&self, module_idx: ModuleIdx) -> Option<ExportsKind> {
    self.formats[module_idx]
  }

  pub(in crate::stages::link_stage) fn set(&mut self, module_idx: ModuleIdx, format: ExportsKind) {
    self.formats[module_idx] = Some(format);
  }

  pub(super) fn finalize(self) -> ModuleFormats {
    ModuleFormats { formats: self.formats }
  }
}

pub(in crate::stages::link_stage) struct ModuleFormats {
  formats: IndexVec<ModuleIdx, Option<ExportsKind>>,
}

impl ModuleFormats {
  pub(in crate::stages::link_stage) fn get(&self, module_idx: ModuleIdx) -> Option<ExportsKind> {
    self.formats[module_idx]
  }

  pub(in crate::stages::link_stage) fn module_count(&self) -> usize {
    self.formats.len()
  }

  pub(in crate::stages::link_stage) fn normal_modules(
    &self,
  ) -> impl Iterator<Item = (ModuleIdx, ExportsKind)> + '_ {
    self.formats.iter_enumerated().filter_map(|(idx, format)| format.map(|format| (idx, format)))
  }
}

pub(super) struct WrapperStateDraftSlot {
  pub(super) kind: Option<WrapKind>,
  pub(super) required_by_other_module: bool,
}

pub(in crate::stages::link_stage) struct WrapperSeeds {
  slots: IndexVec<ModuleIdx, WrapperStateDraftSlot>,
}

impl WrapperSeeds {
  pub(super) fn into_inner(self) -> IndexVec<ModuleIdx, WrapperStateDraftSlot> {
    self.slots
  }
}

impl Pass for DetermineModuleFormatsPass {
  type InputRead<'a> = DetermineModuleFormatsInput<'a>;
  type InputOwned = ();
  type OutputRead = ();
  type OutputOwned = (ModuleFormatsDraft, WrapperSeeds);
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    (): Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let DetermineModuleFormatsInput {
      module_table,
      entry_plan,
      output_format,
      code_splitting_disabled,
    } = input;
    let mut formats = module_table
      .modules
      .iter()
      .map(|module| module.as_normal().map(|module| module.exports_kind))
      .collect::<IndexVec<ModuleIdx, _>>();
    let mut wrapper_seeds = module_table
      .modules
      .iter()
      .map(|module| WrapperStateDraftSlot {
        kind: module.as_normal().map(|_| WrapKind::None),
        required_by_other_module: false,
      })
      .collect::<IndexVec<ModuleIdx, _>>();

    // Both module order and import-record order are semantic here. A promotion
    // made by an earlier importer must be visible to every later importer.
    for (importer_idx, module) in module_table.modules.iter_enumerated() {
      let Module::Normal(importer) = module else { continue };

      for record in &importer.import_records {
        let Some(importee_idx) = record.resolved_module else { continue };
        let Some(importee_kind) = formats[importee_idx] else { continue };
        let Some(importee) = module_table[importee_idx].as_normal() else { continue };

        match record.kind {
          ImportKind::Import => {
            if importee_kind == ExportsKind::None && !importee.meta.has_lazy_export() {
              formats[importee_idx] = Some(ExportsKind::Esm);
            }
          }
          ImportKind::Require => {
            let (format, wrap_kind) = required_format(importee_kind);
            formats[importee_idx] = Some(format);
            wrapper_seeds[importee_idx].kind = Some(wrap_kind);
          }
          ImportKind::DynamicImport if code_splitting_disabled => {
            let (format, wrap_kind) = required_format(importee_kind);
            formats[importee_idx] = Some(format);
            wrapper_seeds[importee_idx].kind = Some(wrap_kind);
          }
          ImportKind::AtImport => {
            std::unreachable!("A Js module would never import a CSS module via `@import`");
          }
          ImportKind::UrlImport => {
            std::unreachable!("A Js module would never import a CSS module via `url()`");
          }
          ImportKind::DynamicImport | ImportKind::NewUrl | ImportKind::HotAccept => {}
        }
      }

      let Some(importer_kind) = formats[importer_idx] else { continue };
      if importer_kind == ExportsKind::CommonJs
        && (!entry_plan.contains_root(importer_idx)
          || std::matches!(output_format, OutputFormat::Esm)
          || (std::matches!(output_format, OutputFormat::Iife | OutputFormat::Umd)
            && importer.ast_usage.intersects(EcmaModuleAstUsage::ModuleOrExports)))
      {
        wrapper_seeds[importer_idx].kind = Some(WrapKind::Cjs);
      }
    }

    Ok(token.finish((), (ModuleFormatsDraft { formats }, WrapperSeeds { slots: wrapper_seeds })))
  }
}

fn required_format(current: ExportsKind) -> (ExportsKind, WrapKind) {
  match current {
    ExportsKind::Esm => (ExportsKind::Esm, WrapKind::Esm),
    ExportsKind::CommonJs | ExportsKind::None => (ExportsKind::CommonJs, WrapKind::Cjs),
  }
}

#[cfg(test)]
mod tests {
  use rolldown_common::{
    EcmaModuleAstUsage, EcmaViewMeta, EntryPoint, EntryPointKind, ExportsKind, ImportKind,
    ModuleTable, OutputFormat, WrapKind,
  };
  use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};

  use super::super::{
    CanonicalizeEntriesPass,
    test_utils::{entry_point, external_module, module_idx, module_table, normal_module},
  };
  use super::{DetermineModuleFormatsInput, DetermineModuleFormatsPass};

  fn set_exports_kind(modules: &mut ModuleTable, index: usize, kind: ExportsKind) {
    modules[module_idx(index)].as_normal_mut().expect("normal module").exports_kind = kind;
  }

  fn classify(
    modules: &ModuleTable,
    entries: &[usize],
    output_format: OutputFormat,
    code_splitting_disabled: bool,
  ) -> (Vec<Option<ExportsKind>>, Vec<Option<WrapKind>>) {
    classify_entries(
      modules,
      entries
        .iter()
        .copied()
        .map(|index| entry_point(index, EntryPointKind::UserDefined))
        .collect(),
      output_format,
      code_splitting_disabled,
    )
  }

  fn classify_entries(
    modules: &ModuleTable,
    entries: Vec<EntryPoint>,
    output_format: OutputFormat,
    code_splitting_disabled: bool,
  ) -> (Vec<Option<ExportsKind>>, Vec<Option<WrapKind>>) {
    let mut pipeline = PassPipelineCtx::new();
    let (_, entry_plan) =
      run_infallible_pass(CanonicalizeEntriesPass, &mut pipeline, modules, entries);
    let (_, (formats, wrappers)) = run_infallible_pass(
      DetermineModuleFormatsPass,
      &mut pipeline,
      DetermineModuleFormatsInput {
        module_table: modules,
        entry_plan: &entry_plan,
        output_format,
        code_splitting_disabled,
      },
      (),
    );
    assert!(pipeline.into_diagnostics().is_empty());
    let formats = (0..modules.modules.len()).map(|idx| formats.get(module_idx(idx))).collect();
    let wrappers =
      (0..modules.modules.len()).map(|idx| wrappers.slots[module_idx(idx)].kind).collect();
    (formats, wrappers)
  }

  #[test]
  fn preserves_import_record_order_when_promoting_none_modules() {
    for (imports, expected_format, expected_wrapper) in [
      (
        vec![
          (ImportKind::Import, Some(2), oxc::span::Span::new(1, 2)),
          (ImportKind::Require, Some(2), oxc::span::Span::new(3, 4)),
        ],
        ExportsKind::Esm,
        WrapKind::Esm,
      ),
      (
        vec![
          (ImportKind::Require, Some(2), oxc::span::Span::new(1, 2)),
          (ImportKind::Import, Some(2), oxc::span::Span::new(3, 4)),
        ],
        ExportsKind::CommonJs,
        WrapKind::Cjs,
      ),
    ] {
      let mut modules = module_table(vec![
        normal_module(0, false, Vec::new()),
        normal_module(1, false, imports),
        normal_module(2, false, Vec::new()),
      ]);
      set_exports_kind(&mut modules, 2, ExportsKind::None);
      let (formats, wrappers) = classify(&modules, &[1], OutputFormat::Esm, false);
      assert_eq!(formats[2], Some(expected_format));
      assert_eq!(wrappers[2], Some(expected_wrapper));
    }
  }

  #[test]
  fn preserves_module_order_when_promoting_none_modules() {
    for (first, second, expected_format, expected_wrapper) in [
      (ImportKind::Import, ImportKind::Require, ExportsKind::Esm, WrapKind::Esm),
      (ImportKind::Require, ImportKind::Import, ExportsKind::CommonJs, WrapKind::Cjs),
    ] {
      let mut modules = module_table(vec![
        normal_module(0, false, Vec::new()),
        normal_module(1, false, vec![(first, Some(3), oxc::span::Span::new(1, 2))]),
        normal_module(2, false, vec![(second, Some(3), oxc::span::Span::new(3, 4))]),
        normal_module(3, false, Vec::new()),
      ]);
      set_exports_kind(&mut modules, 3, ExportsKind::None);
      let (formats, wrappers) = classify(&modules, &[1, 2], OutputFormat::Esm, false);
      assert_eq!(formats[3], Some(expected_format));
      assert_eq!(wrappers[3], Some(expected_wrapper));
    }
  }

  #[test]
  fn preserves_self_import_record_order_when_promoting_none_modules() {
    for (imports, expected_format, expected_wrapper) in [
      (
        vec![
          (ImportKind::Import, Some(0), oxc::span::Span::new(1, 2)),
          (ImportKind::Require, Some(0), oxc::span::Span::new(3, 4)),
        ],
        ExportsKind::Esm,
        WrapKind::Esm,
      ),
      (
        vec![
          (ImportKind::Require, Some(0), oxc::span::Span::new(1, 2)),
          (ImportKind::Import, Some(0), oxc::span::Span::new(3, 4)),
        ],
        ExportsKind::CommonJs,
        WrapKind::Cjs,
      ),
    ] {
      let mut modules = module_table(vec![normal_module(0, false, imports)]);
      set_exports_kind(&mut modules, 0, ExportsKind::None);
      let (formats, wrappers) = classify(&modules, &[0], OutputFormat::Cjs, false);
      assert_eq!(formats[0], Some(expected_format));
      assert_eq!(wrappers[0], Some(expected_wrapper));
    }
  }

  #[test]
  fn keeps_lazy_none_for_static_import_and_only_inlines_dynamic_imports_when_disabled() {
    let mut static_modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), oxc::span::Span::new(1, 2))]),
      normal_module(1, false, Vec::new()),
    ]);
    set_exports_kind(&mut static_modules, 1, ExportsKind::None);
    static_modules[module_idx(1)]
      .as_normal_mut()
      .expect("normal module")
      .meta
      .insert(EcmaViewMeta::HasLazyExport);
    let (formats, wrappers) = classify(&static_modules, &[0], OutputFormat::Esm, false);
    assert_eq!(formats[1], Some(ExportsKind::None));
    assert_eq!(wrappers[1], Some(WrapKind::None));

    for (disabled, expected_format, expected_wrapper) in
      [(false, ExportsKind::None, WrapKind::None), (true, ExportsKind::CommonJs, WrapKind::Cjs)]
    {
      let mut modules = module_table(vec![
        normal_module(
          0,
          false,
          vec![(ImportKind::DynamicImport, Some(1), oxc::span::Span::new(1, 2))],
        ),
        normal_module(1, false, Vec::new()),
      ]);
      set_exports_kind(&mut modules, 1, ExportsKind::None);
      let (formats, wrappers) = classify(&modules, &[0], OutputFormat::Esm, disabled);
      assert_eq!(formats[1], Some(expected_format));
      assert_eq!(wrappers[1], Some(expected_wrapper));
    }
  }

  #[test]
  fn applies_require_and_dynamic_import_matrices_without_mutating_modules() {
    for (import_kind, code_splitting_disabled) in
      [(ImportKind::Require, false), (ImportKind::DynamicImport, true)]
    {
      for (initial, expected_format, expected_wrapper) in [
        (ExportsKind::Esm, ExportsKind::Esm, WrapKind::Esm),
        (ExportsKind::CommonJs, ExportsKind::CommonJs, WrapKind::Cjs),
        (ExportsKind::None, ExportsKind::CommonJs, WrapKind::Cjs),
      ] {
        let mut modules = module_table(vec![
          normal_module(0, false, vec![(import_kind, Some(1), oxc::span::Span::new(1, 2))]),
          normal_module(1, false, Vec::new()),
        ]);
        set_exports_kind(&mut modules, 1, initial);
        let (formats, wrappers) =
          classify(&modules, &[0, 1], OutputFormat::Cjs, code_splitting_disabled);
        assert_eq!(formats[1], Some(expected_format));
        assert_eq!(wrappers[1], Some(expected_wrapper));
        assert_eq!(
          modules[module_idx(1)].as_normal().expect("normal module").exports_kind,
          initial
        );
      }
    }

    for initial in [ExportsKind::Esm, ExportsKind::CommonJs, ExportsKind::None] {
      let mut modules = module_table(vec![
        normal_module(
          0,
          false,
          vec![(ImportKind::DynamicImport, Some(1), oxc::span::Span::new(1, 2))],
        ),
        normal_module(1, false, Vec::new()),
      ]);
      set_exports_kind(&mut modules, 1, initial);
      let (formats, wrappers) = classify(&modules, &[0, 1], OutputFormat::Cjs, false);
      assert_eq!(formats[1], Some(initial));
      assert_eq!(wrappers[1], Some(WrapKind::None));
    }
  }

  #[test]
  fn leaves_non_format_ecma_import_kinds_inert() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::NewUrl, Some(1), oxc::span::Span::new(1, 2)),
          (ImportKind::HotAccept, Some(1), oxc::span::Span::new(3, 4)),
          (ImportKind::Require, None, oxc::span::Span::new(5, 6)),
        ],
      ),
      normal_module(1, false, Vec::new()),
    ]);
    set_exports_kind(&mut modules, 1, ExportsKind::None);
    let (formats, wrappers) = classify(&modules, &[0], OutputFormat::Esm, false);
    assert_eq!(formats[1], Some(ExportsKind::None));
    assert_eq!(wrappers[1], Some(WrapKind::None));
  }

  #[test]
  fn applies_the_commonjs_entry_and_output_format_matrix() {
    for (is_entry, output_format, ast_usage, expected) in [
      (false, OutputFormat::Cjs, EcmaModuleAstUsage::empty(), WrapKind::Cjs),
      (true, OutputFormat::Esm, EcmaModuleAstUsage::empty(), WrapKind::Cjs),
      (true, OutputFormat::Cjs, EcmaModuleAstUsage::empty(), WrapKind::None),
      (true, OutputFormat::Iife, EcmaModuleAstUsage::empty(), WrapKind::None),
      (true, OutputFormat::Umd, EcmaModuleAstUsage::ModuleRef, WrapKind::Cjs),
      (true, OutputFormat::Iife, EcmaModuleAstUsage::ExportsRef, WrapKind::Cjs),
    ] {
      let mut modules = module_table(vec![normal_module(0, false, Vec::new())]);
      let module = modules[module_idx(0)].as_normal_mut().expect("normal module");
      module.exports_kind = ExportsKind::CommonJs;
      module.ast_usage = ast_usage;
      let entries = if is_entry { &[0][..] } else { &[][..] };
      let (_, wrappers) = classify(&modules, entries, output_format, false);
      assert_eq!(wrappers[0], Some(expected));
    }
  }

  #[test]
  fn applies_the_commonjs_entry_exemption_to_every_entry_kind() {
    for kind in [
      EntryPointKind::UserDefined,
      EntryPointKind::DynamicImport,
      EntryPointKind::EmittedUserDefined,
    ] {
      let mut modules = module_table(vec![normal_module(0, false, Vec::new())]);
      set_exports_kind(&mut modules, 0, ExportsKind::CommonJs);
      let (_, wrappers) =
        classify_entries(&modules, vec![entry_point(0, kind)], OutputFormat::Cjs, false);
      assert_eq!(wrappers[0], Some(WrapKind::None), "entry kind {kind:?}");
    }
  }

  #[test]
  fn marks_external_slots_as_outside_both_artifacts() {
    let modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Require, Some(1), oxc::span::Span::new(1, 2))]),
      external_module(1, "external"),
    ]);
    let (formats, wrappers) = classify(&modules, &[0], OutputFormat::Esm, false);
    assert_eq!(formats, vec![Some(ExportsKind::Esm), None]);
    assert_eq!(wrappers, vec![Some(WrapKind::None), None]);
  }
}
