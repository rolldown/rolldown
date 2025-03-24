use std::{
  ops::{Deref, DerefMut},
  sync::Arc,
};

use arcstr::ArcStr;
use oxc::{ast_visit::VisitMut, span::SourceType};
use rolldown_common::{EcmaModuleAstUsage, Module, ModuleIdx, ModuleTable};
use rolldown_ecmascript::EcmaCompiler;
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_error::{BuildResult, ResultExt};
use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  SharedOptions, SharedResolver, hmr::hmr_ast_finalizer::HmrAstFinalizer,
  module_loader::ModuleLoader,
};

pub struct HmrManagerInput {
  pub module_db: ModuleTable,
  pub options: SharedOptions,
  pub fs: OsFileSystem,
  pub resolver: SharedResolver,
  pub plugin_driver: SharedPluginDriver,
}

pub struct HmrManager {
  input: HmrManagerInput,
  module_idx_by_abs_path: FxHashMap<ArcStr, ModuleIdx>,
}

impl Deref for HmrManager {
  type Target = HmrManagerInput;

  fn deref(&self) -> &Self::Target {
    &self.input
  }
}

impl DerefMut for HmrManager {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.input
  }
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
    Self { input, module_idx_by_abs_path }
  }
  #[expect(clippy::dbg_macro, clippy::too_many_lines)] // FIXME: Remove dbg! macro once the feature is stable
  pub async fn generate_hmr_patch(&self, changed_file_paths: Vec<String>) -> BuildResult<String> {
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

    let _module_loader = ModuleLoader::new(
      self.fs,
      Arc::clone(&self.options),
      Arc::clone(&self.resolver),
      Arc::clone(&self.plugin_driver),
      // place holder, since this module loader is unused
      FxHashMap::default(),
      /*is_full_scan*/ true,
    );

    let mut hmr_boundary = FxIndexSet::default();
    let mut affected_modules = FxIndexSet::default();
    let mut need_to_full_reload = false;
    while let Some(changed_module_idx) = changed_modules.pop() {
      if need_to_full_reload {
        break;
      }
      let mut visited_modules = FxHashSet::default();

      let is_reach_to_hmr_boundary = self.propagate_update(
        changed_module_idx,
        &mut visited_modules,
        &mut hmr_boundary,
        &mut affected_modules,
      );

      if !is_reach_to_hmr_boundary {
        need_to_full_reload = true;
      }

      // TODO(hyf0): If it's not a self-accept module, we should traverse its dependents recursively
    }
    if need_to_full_reload {
      return Ok(String::new());
    }

    let module_idx_to_init_fn_name = affected_modules
      .iter()
      .enumerate()
      .map(|(index, module_idx)| {
        let Module::Normal(module) = &self.module_db.modules[*module_idx] else {
          unreachable!("HMR only supports normal module");
        };

        (*module_idx, format!("init_{}_{}", module.repr_name, index))
      })
      .collect::<FxHashMap<_, _>>();

    let mut outputs = vec![];
    for affected_module_idx in affected_modules {
      let affected_module = &self.module_db.modules[affected_module_idx];
      let Module::Normal(affected_module) = affected_module else {
        unreachable!("HMR only supports normal module");
      };

      let filename = affected_module.id.resource_id().clone();

      // TODO: We should get newest source and ast directly from module, but now we just manually fetch them.
      let source: String = std::fs::read_to_string(filename.as_str()).map_err_to_unhandleable()?;
      let mut previous_module_type = affected_module.module_type.clone();
      let transformed_source = self
        .plugin_driver
        .transform(&affected_module.id, source, &mut vec![], &mut None, &mut previous_module_type)
        .await?;

      // Only support hmr on js family for now
      assert!(
        matches!(
          previous_module_type,
          rolldown_common::ModuleType::Js
            | rolldown_common::ModuleType::Jsx
            | rolldown_common::ModuleType::Ts
            | rolldown_common::ModuleType::Tsx
        ),
        "HMR only supports js family modules"
      );
      let source_type = match previous_module_type {
        rolldown_common::ModuleType::Js => SourceType::mjs(),
        rolldown_common::ModuleType::Jsx => SourceType::jsx(),
        rolldown_common::ModuleType::Ts => SourceType::ts(),
        rolldown_common::ModuleType::Tsx => SourceType::tsx(),
        _ => unreachable!(),
      };

      let mut ast = EcmaCompiler::parse(&filename, transformed_source, source_type)?;
      let scoping = ast.make_scoping();

      ast.program.with_mut(|fields| {
        let mut finalizer = HmrAstFinalizer {
          modules: &self.module_db.modules,
          alloc: fields.allocator,
          snippet: AstSnippet::new(fields.allocator),
          scoping: &scoping,
          import_binding: FxHashMap::default(),
          module: affected_module,
          exports: FxHashMap::default(),
          affected_module_idx_to_init_fn_name: &module_idx_to_init_fn_name,
          dependencies: FxIndexSet::default(),
        };

        finalizer.visit_program(fields.program);
      });

      let codegen = EcmaCompiler::print(&ast, &filename, false);
      outputs.push(codegen.code);
    }
    let mut patch = outputs.concat();
    hmr_boundary.iter().for_each(|idx| {
      let init_fn_name = &module_idx_to_init_fn_name[idx];
      patch.push_str(&format!("{init_fn_name}()\n"));
    });
    patch.push_str(&format!(
      "\n__rolldown_runtime__.applyUpdates([{}]);",
      hmr_boundary
        .iter()
        .map(|idx| {
          let module = &self.module_db.modules[*idx];
          format!("'{}'", module.stable_id())
        })
        .collect::<Vec<_>>()
        .join(",")
    ));

    Ok(patch)
  }

  fn propagate_update(
    &self,
    module_idx: ModuleIdx,
    visited_modules: &mut FxHashSet<ModuleIdx>,
    hmr_boundaries: &mut FxIndexSet<ModuleIdx>,
    affected_modules: &mut FxIndexSet<ModuleIdx>,
  ) -> bool /* is reached to hmr boundary  */ {
    if visited_modules.contains(&module_idx) {
      // At this point, we consider circular dependencies as a full reload. We can improve this later.
      return false;
    }

    visited_modules.insert(module_idx);
    let Module::Normal(module) = &self.module_db.modules[module_idx] else {
      unreachable!("HMR only supports normal module");
    };
    affected_modules.insert(module_idx);

    if module.ast_usage.contains(EcmaModuleAstUsage::HmrSelfAccept) {
      hmr_boundaries.insert(module_idx);
      return true;
    }
    module.importers_idx.iter().all(|importer_idx| {
      self.propagate_update(*importer_idx, visited_modules, hmr_boundaries, affected_modules)
    })
  }
}
