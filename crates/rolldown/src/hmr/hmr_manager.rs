use arcstr::ArcStr;
use oxc::{ast_visit::VisitMut, span::SourceType};
use rolldown_common::{EcmaModuleAstUsage, Module, ModuleIdx, ModuleTable};
use rolldown_ecmascript::EcmaCompiler;
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_error::{BuildResult, ResultExt};
use rolldown_utils::indexmap::FxIndexMap;

use crate::hmr::hmr_ast_finalizer::HmrAstFinalizer;

pub struct HmrManager {
  module_db: ModuleTable,
  pub module_idx_by_abs_path: FxIndexMap<ArcStr, ModuleIdx>,
}

impl HmrManager {
  #[expect(clippy::dbg_macro)] // FIXME: Remove dbg! macro once the feature is stable
  pub fn generate_hmr_patch(&self, changed_file_paths: Vec<String>) -> BuildResult<String> {
    let mut changed_modules = vec![];
    for changed_file_path in changed_file_paths {
      let changed_file_path = ArcStr::from(changed_file_path);
      match self.module_idx_by_abs_path.get(&changed_file_path) {
        Some(module_idx) => {
          changed_modules.push(*module_idx);
        }
        _ => {
          dbg!("No corresponding module found for changed file path: {:?}", changed_file_path);
        }
      }
    }

    // Only changed modules might introduce new modules, we run a new module loader to fetch possible new modules and updated content of changed modules
    // TODO(hyf0): Run module loader

    let mut affected_modules = vec![];
    while let Some(changed_module_idx) = changed_modules.pop() {
      let Module::Normal(changed_module) = &self.module_db.modules[changed_module_idx] else {
        continue;
      };

      if changed_module.ast_usage.contains(EcmaModuleAstUsage::HmrSelfAccept) {
        affected_modules.push(changed_module_idx);
        continue;
      }

      // TODO(hyf0): If it's not a self-accept module, we should traverse its dependents recursively
    }

    let mut outputs = vec![];
    for changed_module_idx in affected_modules {
      let changed_module = &self.module_db.modules[changed_module_idx];
      let Module::Normal(changed_module) = changed_module else {
        unreachable!("HMR only supports normal module");
      };

      let filename = changed_module.id.resource_id();

      // TODO: We should get newest source and ast directly from module, but now we just manually fetch them.
      let source: String = std::fs::read_to_string(filename.as_str()).map_err_to_unhandleable()?;
      let mut ast = EcmaCompiler::parse(filename, source, SourceType::default())?;

      ast.program.with_mut(|fields| {
        let mut finalizer = HmrAstFinalizer {
          modules: &self.module_db.modules,
          alloc: fields.allocator,
          snippet: AstSnippet::new(fields.allocator),
        };

        finalizer.visit_program(fields.program);
      });

      let codegen = EcmaCompiler::print(&ast, filename, false);
      outputs.push(codegen.code);
    }

    todo!()
  }
}
