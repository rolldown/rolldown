use std::convert::Infallible;

use rolldown_common::{ConstExportMeta, ModuleIdx, ModuleTable, SymbolRef};
#[cfg(target_family = "wasm")]
use rolldown_utils::rayon::IteratorExt as _;
use rolldown_utils::{
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
  rayon::{IntoParallelRefMutIterator, ParallelIterator},
};
use rustc_hash::FxHashMap;

use super::ExtractGlobalConstantsPass;

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct ConstantExtractionInput {
  pub enabled: bool,
}

pub(in crate::stages::link_stage) struct GlobalConstantsDraft {
  constants: FxHashMap<SymbolRef, ConstExportMeta>,
}

impl GlobalConstantsDraft {
  pub(in crate::stages::link_stage) fn identity_owners(
    &self,
  ) -> impl Iterator<Item = ModuleIdx> + '_ {
    self.constants.keys().map(|symbol_ref| symbol_ref.owner)
  }

  pub(in crate::stages::link_stage) fn into_legacy(self) -> FxHashMap<SymbolRef, ConstExportMeta> {
    self.constants
  }
}

impl Pass for ExtractGlobalConstantsPass {
  type InputRead<'a> = ConstantExtractionInput;
  type InputOwned = ModuleTable;
  type OutputRead = ();
  type OutputOwned = (ModuleTable, GlobalConstantsDraft);
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    mut module_table: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let constants = if input.enabled {
      module_table
        .modules
        .par_iter_mut()
        .filter_map(|module| {
          let module = module.as_normal_mut()?;
          Some(
            std::mem::take(&mut module.constant_export_map)
              .into_iter()
              .map(|(symbol, value)| (SymbolRef { owner: module.idx, symbol }, value)),
          )
        })
        .flatten_iter()
        .collect::<FxHashMap<_, _>>()
    } else {
      FxHashMap::default()
    };

    Ok(token.finish((), (module_table, GlobalConstantsDraft { constants })))
  }
}

#[cfg(test)]
mod tests {
  use oxc::semantic::SymbolId;
  use rolldown_common::{ConstExportMeta, ConstantValue, ModuleTable, SymbolRef};
  use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};

  use super::super::test_utils::{module_idx, module_table, normal_module};
  use super::{ConstantExtractionInput, ExtractGlobalConstantsPass};

  fn table_with_constants() -> (ModuleTable, SymbolId) {
    let symbol = SymbolId::new(1);
    let mut table =
      module_table(vec![normal_module(0, false, Vec::new()), normal_module(1, false, Vec::new())]);
    table[module_idx(0)]
      .as_normal_mut()
      .expect("normal module")
      .constant_export_map
      .insert(symbol, ConstExportMeta::new(ConstantValue::Boolean(true), false));
    table[module_idx(1)]
      .as_normal_mut()
      .expect("normal module")
      .constant_export_map
      .insert(symbol, ConstExportMeta::new(ConstantValue::String("long".into()), true));
    (table, symbol)
  }

  #[test]
  fn enabled_mode_moves_constants_and_disabled_mode_preserves_module_maps() {
    let (table, symbol) = table_with_constants();
    let mut pipeline = PassPipelineCtx::new();
    let (_, (table, constants)) = run_infallible_pass(
      ExtractGlobalConstantsPass,
      &mut pipeline,
      ConstantExtractionInput { enabled: false },
      table,
    );
    assert!(constants.constants.is_empty());
    let first = table[module_idx(0)]
      .as_normal()
      .expect("normal module")
      .constant_export_map
      .get(&symbol)
      .expect("first preserved constant");
    assert!(std::matches!(first.value, ConstantValue::Boolean(true)));
    assert!(!first.commonjs_export);
    assert!(first.safe_to_inline);
    let second = table[module_idx(1)]
      .as_normal()
      .expect("normal module")
      .constant_export_map
      .get(&symbol)
      .expect("second preserved constant");
    assert!(std::matches!(&second.value, ConstantValue::String(value) if value == "long"));
    assert!(second.commonjs_export);
    assert!(!second.safe_to_inline);

    let (table, _) = table_with_constants();
    let (_, (table, constants)) = run_infallible_pass(
      ExtractGlobalConstantsPass,
      &mut pipeline,
      ConstantExtractionInput { enabled: true },
      table,
    );
    assert!(
      table[module_idx(0)].as_normal().expect("normal module").constant_export_map.is_empty()
    );
    let first = constants
      .constants
      .get(&SymbolRef { owner: module_idx(0), symbol })
      .expect("first extracted constant");
    assert!(std::matches!(first.value, ConstantValue::Boolean(true)));
    assert!(!first.commonjs_export);
    assert!(first.safe_to_inline);
    let second = constants
      .constants
      .get(&SymbolRef { owner: module_idx(1), symbol })
      .expect("second extracted constant");
    assert!(std::matches!(&second.value, ConstantValue::String(value) if value == "long"));
    assert!(second.commonjs_export);
    assert!(!second.safe_to_inline);
    assert_eq!(constants.constants.len(), 2);
    assert!(pipeline.into_diagnostics().is_empty());
  }
}
