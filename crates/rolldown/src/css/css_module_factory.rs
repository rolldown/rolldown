use std::sync::Arc;

use oxc::index::IndexVec;
use rolldown_common::{side_effects::DeterminedSideEffects, CssModule, ModuleId};
use rolldown_css::CssCompiler;
use rolldown_error::DiagnosableResult;

use crate::types::module_factory::{
  CreateModuleArgs, CreateModuleContext, CreateModuleReturn, ModuleFactory,
};

pub struct CssModuleFactory;

impl ModuleFactory for CssModuleFactory {
  async fn create_module<'any>(
    ctx: &mut CreateModuleContext<'any>,
    args: CreateModuleArgs,
  ) -> anyhow::Result<DiagnosableResult<CreateModuleReturn>> {
    let id = ModuleId::new(Arc::clone(&ctx.resolved_id.id));
    let stable_id = id.stabilize(&ctx.options.cwd);

    let source = args.source.try_into_string()?;

    let css_ast = CssCompiler::parse(&source, id.to_string())?;

    let module = CssModule {
      exec_order: u32::MAX,
      idx: ctx.module_index,
      stable_id,
      id,
      ast_idx: None,
      source: source.into(),
      // TODO: we should let user to specify side effects for css modules
      side_effects: DeterminedSideEffects::NoTreeshake,
      import_records: IndexVec::default(),
    };

    Ok(Ok(CreateModuleReturn {
      module: module.into(),
      resolved_deps: IndexVec::default(),
      raw_import_records: IndexVec::default(),
      ecma_related: None,
      css_related: Some(css_ast),
    }))
  }
}
