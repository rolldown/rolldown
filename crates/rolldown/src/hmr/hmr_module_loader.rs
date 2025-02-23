use std::sync::Arc;

use arcstr::ArcStr;
use oxc::span::SourceType;
use rolldown_common::{Module, ModuleIdx, ModuleLoaderMsg, ModuleTable};
use rolldown_ecmascript::EcmaCompiler;
use rolldown_error::{BuildResult, ResultExt};
use rustc_hash::FxHashMap;

use crate::{SharedOptions, module_loader::task_context::TaskContext};

pub struct HmrModuleLoader<'me> {
  options: SharedOptions,
  shared_context: Arc<TaskContext>,
  rx: tokio::sync::mpsc::Receiver<ModuleLoaderMsg>,
  remaining: u32,
  pub module_db: &'me ModuleTable,
  pub fetched_modules: FxHashMap<ArcStr, ModuleIdx>,
  pub remaining_tasks: u32,
}

impl HmrModuleLoader<'_> {
  pub fn run(&mut self, changed_module_idx: Vec<ModuleIdx>) -> BuildResult<()> {
    let mut changed_module_ids = vec![];
    for changed_module_idx in changed_module_idx {
      let Module::Normal(module) = &self.module_db.modules[changed_module_idx] else {
        continue;
      };
      changed_module_ids.push(module.id.clone());
    }

    changed_module_ids.iter().for_each(|id| {
      self.fetched_modules.remove(id.resource_id());
    });

    let mut outputs = vec![];
    for changed_module_id in changed_module_ids {
      let filename = changed_module_id.resource_id();
      let source: String = std::fs::read_to_string(filename.as_str()).map_err_to_unhandleable()?;

      // TODO: get source type from previous compilation
      let ast = EcmaCompiler::parse(filename, source, SourceType::default())?;

      // TODO: modify the AST

      let codegen = EcmaCompiler::print(&ast, filename, false);
      outputs.push(codegen.code);
    }

    Ok(())
  }
}
