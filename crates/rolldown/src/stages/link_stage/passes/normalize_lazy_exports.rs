use std::convert::Infallible;

use rolldown_common::{ModuleTable, SymbolRefDb};
use rolldown_utils::{
  IndexBitSet,
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
};

use crate::stages::link_stage::{
  lazy_json_export_initializers::LazyJsonExportInitializers,
  non_splittable_json_defaults::NonSplittableJsonDefaults,
};
use crate::type_alias::{IndexEcmaAst, IndexStmtInfos};

use super::{
  CjsNamespaceMerges, EntryPlanDraft, GlobalConstantsDraft, ModuleFormats, ModuleFormatsDraft,
  ModuleWrappers, NormalizeLazyExportsPass, WrapperDeclarationsDraft,
};
use crate::stages::link_stage::generate_lazy_export::normalize_lazy_exports;

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct NormalizeLazyExportsInput<'a> {
  pub entry_plan: &'a EntryPlanDraft,
  pub cjs_namespace_merges: &'a CjsNamespaceMerges,
  pub global_constants: &'a GlobalConstantsDraft,
}

pub(in crate::stages::link_stage) struct NormalizeLazyExportsOwned {
  pub module_table: ModuleTable,
  pub ast_table: IndexEcmaAst,
  pub stmt_infos: IndexStmtInfos,
  pub symbols: SymbolRefDb,
  pub module_formats: ModuleFormatsDraft,
  pub wrapper_declarations: WrapperDeclarationsDraft,
}

pub(in crate::stages::link_stage) struct NormalizeLazyExportsOutput {
  pub module_table: ModuleTable,
  pub ast_table: IndexEcmaAst,
  pub stmt_infos: IndexStmtInfos,
  pub symbols: SymbolRefDb,
  pub module_formats: ModuleFormats,
  pub module_wrappers: ModuleWrappers,
  pub lazy_json_export_initializers: LazyJsonExportInitializers,
  pub non_splittable_json_defaults: NonSplittableJsonDefaults,
}

impl Pass for NormalizeLazyExportsPass {
  type InputRead<'a> = NormalizeLazyExportsInput<'a>;
  type InputOwned = NormalizeLazyExportsOwned;
  type OutputRead = ();
  type OutputOwned = NormalizeLazyExportsOutput;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    owned: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let NormalizeLazyExportsOwned {
      mut module_table,
      mut ast_table,
      mut stmt_infos,
      mut symbols,
      mut module_formats,
      mut wrapper_declarations,
    } = owned;
    let mut protected_identity_owners = IndexBitSet::new(module_table.modules.len());
    for module_idx in input
      .entry_plan
      .related_identity_owners()
      .chain(input.cjs_namespace_merges.identity_owners())
      .chain(input.global_constants.identity_owners())
    {
      protected_identity_owners.set_bit(module_idx);
    }
    let (lazy_json_export_initializers, non_splittable_json_defaults, diagnostics) =
      normalize_lazy_exports(
        &mut module_table,
        &mut ast_table,
        &mut stmt_infos,
        &mut symbols,
        &mut module_formats,
        &mut wrapper_declarations,
        &protected_identity_owners,
      );
    cx.extend(diagnostics);

    Ok(token.finish(
      (),
      NormalizeLazyExportsOutput {
        module_table,
        ast_table,
        stmt_infos,
        symbols,
        module_formats: module_formats.finalize(),
        module_wrappers: wrapper_declarations.finalize(),
        lazy_json_export_initializers,
        non_splittable_json_defaults,
      },
    ))
  }
}
