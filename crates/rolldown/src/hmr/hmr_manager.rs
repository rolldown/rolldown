use std::{
  ops::{Deref, DerefMut},
  ptr::addr_of,
  sync::Arc,
};

use arcstr::ArcStr;
use oxc::ast_visit::VisitMut;
use rolldown_common::{
  EcmaModuleAstUsage, HmrBoundary, HmrBoundaryOutput, HmrPatch, HmrUpdate, Module, ModuleIdx,
  ModuleTable,
};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler, PrintOptions};
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_error::BuildResult;
use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;
use rolldown_sourcemap::{Source, SourceJoiner, SourceMapSource};
use rolldown_utils::{
  concat_string,
  indexmap::{FxIndexMap, FxIndexSet},
  rayon::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator},
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  SharedOptions, SharedResolver, hmr::hmr_ast_finalizer::HmrAstFinalizer,
  module_loader::ModuleLoader, type_alias::IndexEcmaAst, types::scan_stage_cache::ScanStageCache,
  utils::process_code_and_sourcemap::process_code_and_sourcemap,
};

pub struct HmrManagerInput {
  pub module_db: ModuleTable,
  pub options: SharedOptions,
  pub fs: OsFileSystem,
  pub resolver: SharedResolver,
  pub plugin_driver: SharedPluginDriver,
  pub index_ecma_ast: IndexEcmaAst,
  pub cache: ScanStageCache,
}

pub struct HmrManager {
  input: HmrManagerInput,
  module_idx_by_abs_path: FxHashMap<ArcStr, ModuleIdx>,
  module_idx_by_stable_id: FxHashMap<String, ModuleIdx>,
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
    let module_idx_by_stable_id =
      input.module_db.modules.iter().map(|m| (m.stable_id().to_string(), m.idx())).collect();
    Self { input, module_idx_by_abs_path, module_idx_by_stable_id }
  }

  /// Compute hmr update caused by `import.meta.hot.invalidate()`.
  pub async fn compute_update_for_calling_invalidate(
    &mut self,
    // The parameter is the stable id of the module that called `import.meta.hot.invalidate()`.
    caller: String,
    first_invalidated_by: Option<String>,
  ) -> BuildResult<HmrUpdate> {
    let module_idx = self
      .module_idx_by_stable_id
      .get(&caller)
      .copied()
      .unwrap_or_else(|| panic!("Not found modules for file: {caller}"));
    let module = self.module_db.modules[module_idx].as_normal().unwrap();

    // only self accept modules can be invalidated
    if !module.ast_usage.contains(EcmaModuleAstUsage::HmrSelfAccept) {
      return Ok(HmrUpdate::FullReload {
        reason: "not self accepting for this invalidation".to_string(),
      });
    }

    // Modules could be empty if a root module is invalidated via import.meta.hot.invalidate()
    if module.importers_idx.is_empty() {
      return Ok(HmrUpdate::FullReload {
        reason: "no importers for this invalidation".to_string(),
      });
    }

    // Notice this update is caused by `import.meta.hot.invalidate` call, not by module changes.
    // We know that the hmr boundaries relationships are not changed, because there're no real edits happened.
    // It's safe to just pass the whole importers as start points to compute the hmr update.
    let start_points = module.importers_idx.clone();
    let ret = self.compute_hmr_update(start_points, first_invalidated_by).await?;
    // ret.is_self_accepting = true; // (hyf0) TODO: what's this for?
    Ok(ret)
  }

  pub async fn compute_hmr_update_for_file_changes(
    &mut self,
    changed_file_paths: Vec<String>,
  ) -> BuildResult<Vec<HmrUpdate>> {
    let mut changed_modules = FxIndexSet::default();
    for changed_file_path in changed_file_paths {
      let changed_file_path = ArcStr::from(changed_file_path);
      if let Some(module_idx) = self.module_idx_by_abs_path.get(&changed_file_path) {
        changed_modules.insert(*module_idx);
      }
    }

    tracing::debug!(
      target: "hmr",
      "initial changed modules {:?}",
      changed_modules.iter()
        .map(|module_idx| self.module_db.modules[*module_idx].stable_id())
        .collect::<Vec<_>>(),
    );

    if changed_modules.is_empty() {
      return Ok(vec![HmrUpdate::Noop]);
    }

    let mut updates = vec![];
    for changed_module_idx in changed_modules.iter().copied() {
      // Notice we don't just pass the whole changed modules, because each change might contains editing `import.meta.accept`.
      // Editing `import.meta.accept will change the hmr boundaries relationships. We need to ensure the subsequent change could observe
      // this possible behavior of the previous change.
      let start_points = FxIndexSet::from_iter([changed_module_idx]);
      let update = self.compute_hmr_update(FxIndexSet::from_iter(start_points), None).await?;
      updates.push(update);
    }

    Ok(updates)
  }

  #[expect(clippy::too_many_lines)]
  async fn compute_hmr_update(
    &mut self,
    start_points: FxIndexSet<ModuleIdx>,
    first_invalidated_by: Option<String>,
  ) -> BuildResult<HmrUpdate> {
    let mut affected_modules = FxIndexSet::default();
    let mut need_to_full_reload = false;
    let mut full_reload_reason = None;

    // We're checking if this change could be handled by hmr. If so, we need to find the HMR boundaries.
    let hmr_boundaries = self.compute_out_hmr_boundaries(
      &start_points,
      &mut need_to_full_reload,
      first_invalidated_by.as_deref(),
      &mut full_reload_reason,
    );

    tracing::debug!(
      target: "hmr",
      "computed out `hmr_boundaries` {:?}",
      hmr_boundaries.iter()
        .map(|boundary| self.module_db.modules[boundary.boundary].stable_id())
        .collect::<Vec<_>>(),
    );

    // The HMR process will execute the code starting from the HMR boundaries. The HMR process expects all dependencies
    // of the HMR boundaries to be re-executed. We collect them here to include them in the HMR output.
    affected_modules
      .extend(Self::collect_affected_modules_from_boundaries(&self.module_db, &hmr_boundaries));

    if need_to_full_reload {
      return Ok(HmrUpdate::FullReload {
        reason: full_reload_reason.unwrap_or_else(|| "Unknown reason".to_string()),
      });
    }

    tracing::debug!(
      target: "hmr",
      "computed out `affected_modules` {:?}",
      affected_modules.iter()
        .map(|module_idx| self.module_db.modules[*module_idx].stable_id())
        .collect::<Vec<_>>(),
    );

    let mut modules_to_invalidate = start_points.clone();
    // FIXME(hyf0): In general, only modules got edited need to be invalidated, because we need to refetch their latest content.
    // For those modules that are not edited but affected, we should be able to reuse their previous AST instead of going through the whole
    // module loading process again. But currently we don't have a good way to do that due to architecture limitation.
    modules_to_invalidate.extend(affected_modules.clone());

    // FIXME: It's expected that `rolldown:runtime` appears in the `affected_modules`. But the module loader can't handle it properly, it'll
    // report an error that disk doesn't exist file called `rolldown:runtime`.
    self.module_db.iter().find_map(|m| (m.id() == "rolldown:runtime").then_some(m.idx())).inspect(
      |runtime_idx| {
        modules_to_invalidate.shift_remove(runtime_idx);
      },
    );

    tracing::debug!(
      target: "hmr",
      "modules_to_invalidate` {:?}",
      modules_to_invalidate.iter()
        .map(|module_idx| self.module_db.modules[*module_idx].stable_id())
        .collect::<Vec<_>>(),
    );

    let module_infos_to_be_updated = modules_to_invalidate
      .iter()
      .filter_map(|module_idx| {
        let module = &self.module_db.modules[*module_idx];
        if let Module::Normal(module) = module {
          Some(module.originative_resolved_id.clone())
        } else {
          // unreachable!("HMR only supports normal module. Got {:?}", module.id());
          None
        }
      })
      .collect::<Vec<_>>();

    let mut module_loader = ModuleLoader::new(
      self.fs.clone(),
      Arc::clone(&self.options),
      Arc::clone(&self.resolver),
      Arc::clone(&self.plugin_driver),
      &mut self.cache,
      false,
    )?;

    let mut module_loader_output =
      module_loader.fetch_modules(vec![], &module_infos_to_be_updated).await?;

    // We manually impl `Drop` for `ModuleLoader` to avoid missing assign `importers` to
    // `self.cache`, but rustc is not smart enough to infer actually we don't touch it in `drop`
    // implementation, so we need to manually drop it.
    drop(module_loader);

    tracing::debug!(
      target: "hmr",
      "New added modules` {:?}",
      module_loader_output
        .new_added_modules_from_partial_scan
        .iter()
        .map(|module_idx| module_loader_output.module_table.get(*module_idx).stable_id())
        .collect::<Vec<_>>(),
    );

    affected_modules.extend(module_loader_output.new_added_modules_from_partial_scan);

    let mut updated_modules =
      module_loader_output.module_table.into_iter_enumerated().into_iter().collect::<Vec<_>>();
    tracing::debug!(
      target: "hmr",
      "updated_modules` {:?}",
      updated_modules
        .iter().map(|(idx, module)| (idx, module.stable_id()))
        .collect::<Vec<_>>(),
    );
    updated_modules.sort_by_key(|(idx, _)| *idx);

    // TODO(hyf0): This is a temporary merging solution. We need to find a better way to handle this.
    for (idx, module) in updated_modules {
      if idx.index() >= self.module_db.modules.len() {
        // This module is newly added, we need to insert it into the module db.
        let generated_id = self.module_db.modules.push(module);
        self.index_ecma_ast.push(module_loader_output.index_ecma_ast.get_mut(idx).take());
        assert_eq!(generated_id, idx, "Module index mismatch");
      } else {
        // This module is already in the module db, we need to update it.
        self.module_db.modules[idx] = module;
        self.index_ecma_ast[idx] = module_loader_output.index_ecma_ast.get_mut(idx).take();
      }
    }
    tracing::debug!(
      target: "hmr",
      "New added modules2` {:?}",
      affected_modules
        .iter()
        .map(|module_idx| self.module_db.modules[*module_idx].stable_id())
        .collect::<Vec<_>>(),
    );

    // Remove external modules from affected_modules.
    affected_modules.retain(|idx| {
      let module = &self.module_db.modules[*idx];
      // It's possible that affected modules are external modules. New added code might contains import from external modules.
      // However, HMR doesn't need to deal with them.
      !matches!(module, Module::External(_))
    });

    // It's actually unnecessary to sort `affected_modules` here, but sorting it:
    // - Makes the snapshot less changeable when we change logic that affects the order of modules.
    affected_modules.sort_by_cached_key(|module_idx| self.module_db.modules[*module_idx].id());

    let module_idx_to_init_fn_name = affected_modules
      .iter()
      .enumerate()
      .map(|(index, module_idx)| {
        let Module::Normal(module) = &self.module_db.modules[*module_idx] else {
          unreachable!(
            "External modules should be removed before. But got {:?}",
            self.module_db.modules[*module_idx].id()
          );
        };
        let prefix = if module.exports_kind.is_commonjs() { "require" } else { "init" };

        // We use `index` as a part of the function name to avoid name collision without needing to deconflict.
        (*module_idx, format!("{}_{}_{}", prefix, module.repr_name, index))
      })
      .collect::<FxHashMap<_, _>>();

    let module_render_inputs = affected_modules
      .iter()
      .copied()
      .map(|affected_module_idx| {
        let affected_module = &self.input.module_db.modules[affected_module_idx];
        let Module::Normal(affected_module) = affected_module else {
          unreachable!("HMR only supports normal module");
        };

        debug_assert_eq!(affected_module_idx, affected_module.idx);
        let ast = self.input.index_ecma_ast[affected_module_idx]
          .as_ref()
          .expect("Normal module should have an AST");

        // SAFETY: `affected_modules` is a set, so we won't have multiple mutable references to the same ast.
        let mut_ast = unsafe { &mut *(addr_of!(*ast).cast_mut()) };

        ModuleRenderInput { idx: affected_module.idx, ecma_ast: mut_ast }
      })
      .collect::<Vec<_>>();

    let mut source_joiner = SourceJoiner::default();
    let rendered_sources = module_render_inputs
      .into_par_iter()
      .enumerate()
      .flat_map(|(index, render_input)| {
        let ModuleRenderInput { idx: affected_module_idx, ecma_ast: ast } = render_input;

        let affected_module = &self.input.module_db.modules[affected_module_idx];
        let Module::Normal(affected_module) = affected_module else {
          unreachable!("HMR only supports normal module");
        };

        let enable_sourcemap = self.options.sourcemap.is_some() && !affected_module.is_virtual();
        let use_pife_for_module_wrappers =
          self.options.optimization.is_pife_for_module_wrappers_enabled();
        let modules = &self.input.module_db.modules;

        ast.program.with_mut(|fields| {
          let scoping = EcmaAst::make_semantic(fields.program, /*with_cfg*/ false).into_scoping();
          let mut finalizer = HmrAstFinalizer {
            modules,
            alloc: fields.allocator,
            snippet: AstSnippet::new(fields.allocator),
            builder: &oxc::ast::AstBuilder::new(fields.allocator),
            scoping: &scoping,
            import_bindings: FxHashMap::default(),
            module: affected_module,
            exports: oxc::allocator::Vec::new_in(fields.allocator),
            affected_module_idx_to_init_fn_name: &module_idx_to_init_fn_name,
            use_pife_for_module_wrappers,
            dependencies: FxIndexSet::default(),
            imports: FxHashSet::default(),
            generated_static_import_infos: FxHashMap::default(),
            re_export_all_dependencies: FxIndexSet::default(),
            generated_static_import_stmts_from_external: FxIndexMap::default(),
            unique_index: index,
            named_exports: FxHashMap::default(),
          };

          finalizer.visit_program(fields.program);
        });

        let codegen = EcmaCompiler::print_with(
          ast,
          PrintOptions {
            sourcemap: enable_sourcemap,
            filename: affected_module.id.to_string(),
            print_legal_comments: false, // ignore hmr chunk comments
          },
        );

        let intro_comment: Box<dyn Source + Send> =
          Box::new(concat_string!("//#region ", affected_module.debug_id));
        let outro_comment: Box<dyn Source + Send> = Box::new(concat_string!("//#endregion"));

        let code_source: Box<dyn Source + Send> = if let Some(map) = codegen.map {
          Box::new(SourceMapSource::new(codegen.code, map))
        } else {
          Box::new(codegen.code)
        };

        [intro_comment, code_source, outro_comment]
      })
      .collect::<Vec<_>>();

    for source in rendered_sources {
      source_joiner.append_source_dyn(source);
    }

    hmr_boundaries.iter().for_each(|boundary| {
      let init_fn_name = &module_idx_to_init_fn_name[&boundary.boundary];
      source_joiner.append_source(format!("{init_fn_name}()"));
    });

    source_joiner.append_source(format!(
      "__rolldown_runtime__.applyUpdates([{}]);",
      hmr_boundaries
        .iter()
        .map(|boundary| {
          let module = &self.module_db.modules[boundary.boundary];
          format!("'{}'", module.stable_id())
        })
        .collect::<Vec<_>>()
        .join(",")
    ));

    let (mut code, mut map) = source_joiner.join();

    let filename = format!(
      "{}.js",
      std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()
    );

    let file_dir = self.options.cwd.as_path().join(&self.options.out_dir);

    let sourcemap_asset = if let Some(map) = map.as_mut() {
      process_code_and_sourcemap(
        &self.options,
        &mut code,
        map,
        &file_dir,
        filename.as_str(),
        0,
        /*is_css*/ false,
      )
      .await?
    } else {
      None
    };

    Ok(HmrUpdate::Patch(HmrPatch {
      code,
      filename,
      sourcemap_filename: sourcemap_asset.as_ref().map(|asset| asset.filename.to_string()),
      sourcemap: sourcemap_asset.map(|asset| asset.source.try_into_string()).transpose()?,
      hmr_boundaries: hmr_boundaries
        .into_iter()
        .map(|boundary| HmrBoundaryOutput {
          boundary: self.module_db.modules[boundary.boundary].stable_id().into(),
          accepted_via: self.module_db.modules[boundary.accepted_via].stable_id().into(),
        })
        .collect(),
    }))
  }

  fn propagate_update(
    &self,
    module_idx: ModuleIdx,
    hmr_boundaries: &mut FxIndexSet<HmrBoundary>,
    propagate_stack: &mut Vec<ModuleIdx>,
  ) -> PropagateUpdateStatus {
    let Module::Normal(module) = &self.module_db.modules[module_idx] else {
      // We consider reaching external modules as a boundary.
      return PropagateUpdateStatus::ReachedBoundary;
    };

    if let Some(circular_start_index) = propagate_stack
      .iter()
      .enumerate()
      .find_map(|(index, each_module_idx)| (module_idx == *each_module_idx).then_some(index))
    {
      // Jumping into this branch means we have a circular dependency.
      // X -> Y means X imports Y. and we have
      // A -> B -> C -> D(edited)
      // C -> B
      // When we reach to C again, the stack contains [D, C, B]
      let cycle_chain = propagate_stack[circular_start_index..]
        .iter()
        .copied()
        .chain(std::iter::once(module_idx))
        // Notice, our traversal is done by reaching `importers`, so the vec order is opposite to the import order.
        .rev()
        .collect::<Vec<_>>();

      return PropagateUpdateStatus::Circular(cycle_chain);
    }

    if module.is_hmr_self_accepting_module() {
      hmr_boundaries.insert(HmrBoundary { boundary: module_idx, accepted_via: module_idx });
      return PropagateUpdateStatus::ReachedBoundary;
    } else if module.importers_idx.is_empty() {
      // This module is not self-aceepting and don't have any potential importer that might accepts its update
      return PropagateUpdateStatus::NoBoundary(module_idx);
    }

    let mut importers_idx = module.importers_idx.iter().copied().collect::<Vec<_>>();
    // FIXME(hyf0): In practice, the order of importers doesn't matter since we gonna traverse all of them.
    // However, non-deterministic order causes unstable snapshots.
    importers_idx.sort_by_key(|importer_idx| self.module_db.modules[*importer_idx].stable_id());

    for importer_idx in importers_idx {
      let Module::Normal(importer) = &self.module_db.modules[importer_idx] else {
        continue;
      };

      if importer.can_accept_hmr_dependency_for(&module.id) {
        hmr_boundaries.insert(HmrBoundary { boundary: importer_idx, accepted_via: module_idx });
        continue;
      }

      propagate_stack.push(module_idx);
      let status = self.propagate_update(importer_idx, hmr_boundaries, propagate_stack);
      propagate_stack.pop();
      if !status.is_reached_boundary() {
        return status;
      }
    }

    PropagateUpdateStatus::ReachedBoundary
  }

  fn collect_affected_modules_from_boundaries(
    modules: &ModuleTable,
    hmr_boundaries: &FxIndexSet<HmrBoundary>,
  ) -> impl IntoIterator<Item = ModuleIdx> {
    fn collect_dependencies(
      modules: &ModuleTable,
      module_idx: ModuleIdx,
      visited: &mut FxHashSet<ModuleIdx>,
    ) {
      if visited.contains(&module_idx) {
        return;
      }
      visited.insert(module_idx);

      let module = &modules[module_idx];
      match module {
        Module::Normal(normal_module) => {
          normal_module.import_records.iter().for_each(|import_record| {
            collect_dependencies(modules, import_record.resolved_module, visited);
          });
        }
        Module::External(_external_module) => {
          // No need to deal with external modules in HMR.
        }
      }
    }
    let mut visited = FxHashSet::default();
    hmr_boundaries.iter().for_each(|boundary| {
      collect_dependencies(modules, boundary.boundary, &mut visited);
    });

    visited
  }

  fn compute_out_hmr_boundaries(
    &self,
    start_points: &FxIndexSet<ModuleIdx>,
    need_to_full_reload: &mut bool,
    first_invalidated_by: Option<&str>,
    reason: &mut Option<String>,
  ) -> FxIndexSet<HmrBoundary> {
    let mut hmr_boundaries = FxIndexSet::default();

    for start_point in start_points.iter().copied() {
      if *need_to_full_reload {
        break;
      }
      let mut boundaries = FxIndexSet::default();
      let propagate_status = self.propagate_update(start_point, &mut boundaries, &mut vec![]);

      match propagate_status {
        PropagateUpdateStatus::Circular(cycle_chain) => {
          *need_to_full_reload = true;
          *reason = Some(format!(
            "circular import chain: {}",
            cycle_chain
              .iter()
              .map(|module_idx| self.module_db.modules[*module_idx].stable_id())
              .collect::<Vec<_>>()
              .join(" -> ")
          ));
          break;
        }
        PropagateUpdateStatus::NoBoundary(idx) => {
          *need_to_full_reload = true;
          let module = &self.module_db.modules[idx];
          *reason = Some(format!("no hmr boundary found for module `{}`", module.stable_id()));
          break;
        }
        PropagateUpdateStatus::ReachedBoundary => {}
      }

      // If import.meta.hot.invalidate was called already on that module for the same update,
      // it means any importer of that module can't hot update. We should fallback to full reload.
      if let Some(first_invalidated_by) = first_invalidated_by.as_ref() {
        if boundaries.iter().any(|boundary| {
          self.module_db.modules[boundary.accepted_via].stable_id() == *first_invalidated_by
        }) {
          *need_to_full_reload = true;
          // full_reload_reason = Some("circular import invalidate".to_string());
          continue;
        }
      }

      hmr_boundaries.extend(boundaries);
    }

    hmr_boundaries
  }
}

enum PropagateUpdateStatus {
  Circular(Vec<ModuleIdx>), // The circular dependency chain
  ReachedBoundary,
  NoBoundary(ModuleIdx),
}

impl PropagateUpdateStatus {
  pub fn is_reached_boundary(&self) -> bool {
    matches!(self, Self::ReachedBoundary)
  }
}

struct ModuleRenderInput<'me> {
  pub idx: ModuleIdx,
  pub ecma_ast: &'me mut EcmaAst,
}
