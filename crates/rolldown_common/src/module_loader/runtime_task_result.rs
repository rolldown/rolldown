use rolldown_ecmascript::EcmaAst;

use crate::{NormalModule, SymbolRefDbForModule};

use super::runtime_module_brief::RuntimeModuleBrief;

pub struct RuntimeModuleTaskResult {
  pub runtime: RuntimeModuleBrief,
  pub local_symbol_ref_db: SymbolRefDbForModule,
  pub ast: EcmaAst,
  // pub warnings: Vec<BuildError>,
  pub module: NormalModule,
}
