use arcstr::ArcStr;
use oxc::{ast_visit::VisitMut, span::SourceType};
use rolldown_common::{EcmaModuleAstUsage, Module, ModuleIdx, ModuleTable};
use rolldown_ecmascript::EcmaCompiler;
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_error::{BuildResult, ResultExt};
use rustc_hash::FxHashMap;

use crate::hmr::hmr_ast_finalizer::HmrAstFinalizer;

pub struct HmrManagerInput {
  pub module_db: ModuleTable,
}

pub struct HmrManager {
  module_db: ModuleTable,
  module_idx_by_abs_path: FxHashMap<ArcStr, ModuleIdx>,
}

impl HmrManager {
  pub fn new(input: HmrManagerInput) -> Self {
    let module_idx_by_abs_path = input
      .module_db
      .modules
      .iter()
      .filter_map(|m| m.as_normal())
      .map(|m| {
        let filename = m.id.resource_id().clone();
        let module_idx = m.idx;
        (filename, module_idx)
      })
      .collect();
    Self { module_db: input.module_db, module_idx_by_abs_path }
  }
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

      let filename = changed_module.id.resource_id().clone();

      // TODO: We should get newest source and ast directly from module, but now we just manually fetch them.
      let source: String = std::fs::read_to_string(filename.as_str()).map_err_to_unhandleable()?;
      let mut ast = EcmaCompiler::parse(&filename, source, SourceType::default())?;
      let (symbol_table, scope_tree) = ast.make_symbol_table_and_scope_tree();

      ast.program.with_mut(|fields| {
        let mut finalizer = HmrAstFinalizer {
          modules: &self.module_db.modules,
          alloc: fields.allocator,
          snippet: AstSnippet::new(fields.allocator),
          scopes: &scope_tree,
          symbols: &symbol_table,
          import_binding: FxHashMap::default(),
          module: changed_module,
        };

        finalizer.visit_program(fields.program);
      });

      let codegen = EcmaCompiler::print(&ast, &filename, false);
      outputs.push(codegen.code);
    }

    todo!()
  }
}
