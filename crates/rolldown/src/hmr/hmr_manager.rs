use std::{
  fmt::Write as _,
  ops::{Deref, DerefMut},
  sync::Arc,
};

use arcstr::ArcStr;
use oxc::ast_visit::VisitMut;
use rolldown_common::{
  EcmaModuleAstUsage, HmrBoundary, HmrBoundaryOutput, HmrOutput, Module, ModuleIdx, ModuleTable,
};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler};
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_error::BuildResult;
use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  SharedOptions, SharedResolver, hmr::hmr_ast_finalizer::HmrAstFinalizer,
  module_loader::ModuleLoader, type_alias::IndexEcmaAst, types::scan_stage_cache::ScanStageCache,
};

pub struct HmrManagerInput {
  pub module_db: ModuleTable,
  pub options: SharedOptions,
  pub fs: OsFileSystem,
  pub resolver: SharedResolver,
  pub plugin_driver: SharedPluginDriver,
  pub index_ecma_ast: IndexEcmaAst,
  pub cache: ScanStageCache,
  pub session_span: tracing::Span,
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
  pub async fn generate_hmr_patch(
    &mut self,
    changed_file_paths: Vec<String>,
  ) -> BuildResult<HmrOutput> {
    let mut changed_modules = FxIndexSet::default();
    for changed_file_path in changed_file_paths {
      let changed_file_path = ArcStr::from(changed_file_path);
      match self.module_idx_by_abs_path.get(&changed_file_path) {
        Some(module_idx) => {
          changed_modules.insert(*module_idx);
        }
        _ => {
          dbg!("No corresponding module found for changed file path: {:?}", changed_file_path);
        }
      }
    }

    let mut affected_modules = FxIndexSet::default();
    let mut hmr_boundary = FxIndexSet::default();
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
    }
    if need_to_full_reload {
      return Ok(HmrOutput::default());
    }

    let mut modules_to_invalidate = changed_modules.clone();
    // FIXME(hyf0): In general, only modules got edited need to be invalidated, because we need to refetch their latest content.
    // For those modules that are not edited, we should be able to reuse their AST. But currently we don't have a good way to do that
    // due to architecture limitation.
    modules_to_invalidate.extend(affected_modules.clone());

    let module_infos_to_be_updated = modules_to_invalidate
      .iter()
      .map(|module_idx| {
        let module = &self.module_db.modules[*module_idx];
        let Module::Normal(module) = module else {
          unreachable!("HMR only supports normal module");
        };
        module.originative_resolved_id.clone()
      })
      .collect::<Vec<_>>();

    let module_loader = ModuleLoader::new(
      self.fs,
      Arc::clone(&self.options),
      Arc::clone(&self.resolver),
      Arc::clone(&self.plugin_driver),
      std::mem::take(&mut self.cache),
      false,
      self.session_span.clone(),
    )?;

    let module_loader_output =
      module_loader.fetch_modules(vec![], module_infos_to_be_updated).await?;

    self.cache = module_loader_output.cache;

    affected_modules.extend(module_loader_output.new_added_modules_from_partial_scan);
    // Update

    let mut updated_modules =
      module_loader_output.module_table.into_iter_enumerated().into_iter().collect::<Vec<_>>();
    updated_modules.sort_by_key(|(idx, _)| *idx);

    // TODO(hyf0): This is a temporary merging solution. We need to find a better way to handle this.
    for (idx, module) in updated_modules {
      if idx.index() >= self.module_db.modules.len() {
        // This module is newly added, we need to insert it into the module db.
        let generated_id = self.module_db.modules.push(module);
        assert_eq!(generated_id, idx, "Module index mismatch");
      } else {
        // This module is already in the module db, we need to update it.
        self.module_db.modules[idx] = module;
      }
    }
    self.index_ecma_ast = module_loader_output.index_ecma_ast;

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
      let affected_module = &self.input.module_db.modules[affected_module_idx];
      let Module::Normal(affected_module) = affected_module else {
        unreachable!("HMR only supports normal module");
      };

      let filename = affected_module.id.resource_id().clone();
      let ecma_ast_idx = affected_module.ecma_ast_idx.unwrap();
      let modules = &self.input.module_db.modules;
      let ast = &mut self.input.index_ecma_ast[ecma_ast_idx].0;

      ast.program.with_mut(|fields| {
        let scoping = EcmaAst::make_semantic(fields.program).into_scoping();
        let mut finalizer = HmrAstFinalizer {
          modules,
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

      let codegen = EcmaCompiler::print(ast, &filename, false);
      outputs.push(codegen.code);
    }
    let mut patch = outputs.concat();
    hmr_boundary.iter().for_each(|boundary| {
      let init_fn_name = &module_idx_to_init_fn_name[&boundary.boundary];
      writeln!(patch, "{init_fn_name}()").unwrap();
    });
    write!(
      patch,
      "\n__rolldown_runtime__.applyUpdates([{}]);",
      hmr_boundary
        .iter()
        .map(|boundary| {
          let module = &self.module_db.modules[boundary.boundary];
          format!("'{}'", module.stable_id())
        })
        .collect::<Vec<_>>()
        .join(",")
    )
    .unwrap();

    Ok(HmrOutput {
      patch,
      hmr_boundaries: hmr_boundary
        .into_iter()
        .map(|boundary| HmrBoundaryOutput {
          boundary: self.module_db.modules[boundary.boundary].stable_id().into(),
          accepted_via: self.module_db.modules[boundary.accepted_via].stable_id().into(),
        })
        .collect(),
    })
  }

  fn propagate_update(
    &self,
    module_idx: ModuleIdx,
    visited_modules: &mut FxHashSet<ModuleIdx>,
    hmr_boundaries: &mut FxIndexSet<HmrBoundary>,
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
      hmr_boundaries.insert(HmrBoundary { boundary: module_idx, accepted_via: module_idx });
      return true;
    }
    module.importers_idx.iter().all(|importer_idx| {
      self.propagate_update(*importer_idx, visited_modules, hmr_boundaries, affected_modules)
    })
  }
}
