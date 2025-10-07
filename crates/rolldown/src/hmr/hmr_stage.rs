use std::{
  ops::{Deref, DerefMut},
  sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
  },
};

use arcstr::ArcStr;
use oxc_traverse::traverse_mut;
use rolldown_common::{
  ClientHmrInput, ClientHmrUpdate, HmrBoundary, HmrBoundaryOutput, HmrPatch, HmrUpdate, Module,
  ModuleIdx, ModuleTable, ScanMode,
};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler, PrintOptions};
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_error::BuildResult;
use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;
use rolldown_sourcemap::{Source, SourceJoiner, SourceMapSource};
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::{
  concat_string,
  indexmap::{FxIndexMap, FxIndexSet},
  rayon::{IntoParallelIterator, ParallelIterator},
};
use rustc_hash::{FxHashMap, FxHashSet};
use sugar_path::SugarPath;

use crate::{
  SharedOptions, SharedResolver, hmr::hmr_ast_finalizer::HmrAstFinalizer,
  module_loader::ModuleLoader, type_alias::IndexEcmaAst, types::scan_stage_cache::ScanStageCache,
  utils::process_code_and_sourcemap::process_code_and_sourcemap,
};

pub struct HmrStageInput<'a> {
  pub options: SharedOptions,
  pub fs: OsFileSystem,
  pub resolver: SharedResolver,
  pub plugin_driver: SharedPluginDriver,
  pub cache: &'a mut ScanStageCache,
  pub next_hmr_patch_id: Arc<AtomicU32>,
}

impl HmrStageInput<'_> {
  pub fn module_table(&self) -> &ModuleTable {
    &self.cache.get_snapshot().module_table
  }

  pub fn index_ecma_ast(&self) -> &IndexEcmaAst {
    &self.cache.get_snapshot().index_ecma_ast
  }
}

pub struct HmrStage<'a> {
  pub(crate) input: HmrStageInput<'a>,
}

impl<'a> Deref for HmrStage<'a> {
  type Target = HmrStageInput<'a>;

  fn deref(&self) -> &Self::Target {
    &self.input
  }
}

impl DerefMut for HmrStage<'_> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.input
  }
}

impl<'a> HmrStage<'a> {
  pub fn new(input: HmrStageInput<'a>) -> Self {
    Self { input }
  }

  /// Compute hmr update caused by `import.meta.hot.invalidate()`.
  pub async fn compute_update_for_calling_invalidate(
    &mut self,
    // The parameter is the stable id of the module that called `import.meta.hot.invalidate()`.
    invalidate_caller: String,
    first_invalidated_by: Option<String>,
    executed_modules: &FxHashSet<String>,
  ) -> BuildResult<HmrUpdate> {
    tracing::debug!(
      target: "hmr",
      "compute_update_for_calling_invalidate: caller: {:?}, first_invalidated_by: {:?}",
      invalidate_caller,
      first_invalidated_by,
    );
    let module_idx = self
      .cache
      .module_idx_by_stable_id
      .get(&invalidate_caller)
      .copied()
      .unwrap_or_else(|| panic!("Not found modules for file: {invalidate_caller}"));

    let caller = self.module_table().modules[module_idx].as_normal().unwrap();

    if !executed_modules.contains(&caller.stable_id) {
      // If this module is not registered, we simply ignore it.
      return Ok(HmrUpdate::Noop);
    }

    // Only self accepting modules are allowed to call `import.meta.hot.invalidate()`.
    if !caller.is_hmr_self_accepting_module() {
      return Ok(HmrUpdate::FullReload {
        reason: "not self accepting for this invalidation".to_string(),
      });
    }

    // Calling `import.meta.hot.invalidate()` means this module can't handle the update and wants to pass it to its importers.
    // If there are no importers, the update can't be handled at all, which requires a full reload.
    if caller.importers_idx.is_empty() {
      return Ok(HmrUpdate::FullReload {
        reason: format!(
          "There are no importers to handle `import.meta.hot.invalidate()` called by `{}`",
          caller.stable_id
        ),
      });
    }

    // Stale modules don't include the caller itself, because the caller's latest content/code has already been executed on the client side.
    // Since it was already executed, it was able to determine that it couldn't handle the update and needed to call `import.meta.hot.invalidate()`.
    //
    // We can safely batch these importers into one update, because we know no file edits have occurred and the HMR boundary relationships
    // remain unchanged.
    let mut stale_modules = caller.importers_idx.clone();
    stale_modules.swap_remove(&caller.idx); // ignore self-imports

    // Workaround: Create a temporary single-client array to call compute_hmr_update
    let temp_client = ClientHmrInput {
      client_id: String::new(), // placeholder, not used in this path
      executed_modules: executed_modules.clone(),
    };

    let mut results = self
      .compute_hmr_update(
        &stale_modules,
        &FxIndexSet::default(),
        first_invalidated_by,
        &[temp_client],
      )
      .await?;

    // Extract the single result
    // ret.is_self_accepting = true; // (hyf0) TODO: what's this for?
    Ok(results.pop().unwrap().update)
  }

  pub async fn compute_hmr_update_for_file_changes(
    &mut self,
    changed_file_paths: &[String],
    clients: &[ClientHmrInput],
  ) -> BuildResult<Vec<ClientHmrUpdate>> {
    tracing::debug!(target: "hmr", "compute_hmr_update_for_file_changes: {:?}", changed_file_paths);

    // 1. Identify changed modules
    let mut changed_modules = FxIndexSet::default();
    for changed_file_path in changed_file_paths {
      let changed_file_path = ArcStr::from(changed_file_path.to_slash().unwrap());
      // Check if the file itself is a module
      if let Some(module_idx) = self.cache.module_idx_by_abs_path.get(&changed_file_path) {
        changed_modules.insert(*module_idx);
      }

      // Check if any modules have this file as a transform dependency
      for entry in self.plugin_driver.transform_dependencies.iter() {
        let module_idx = *entry.key();
        let deps = entry.value();
        if deps.contains(&changed_file_path) {
          changed_modules.insert(module_idx);
        }
      }
    }

    tracing::debug!(
      target: "hmr",
      "initial changed modules {:?}",
      changed_modules.iter()
        .map(|module_idx| self.module_table().modules[*module_idx].stable_id())
        .collect::<Vec<_>>(),
    );

    if changed_modules.is_empty() {
      return Ok(
        clients
          .iter()
          .map(|client| ClientHmrUpdate {
            client_id: client.client_id.clone(),
            update: HmrUpdate::Noop,
          })
          .collect(),
      );
    }

    self.compute_hmr_update(&changed_modules, &changed_modules, None, clients).await
  }

  async fn compute_hmr_update(
    &mut self,
    stale_modules: &FxIndexSet<ModuleIdx>,
    changed_modules: &FxIndexSet<ModuleIdx>,
    first_invalidated_by: Option<String>,
    clients: &[ClientHmrInput],
  ) -> BuildResult<Vec<ClientHmrUpdate>> {
    // 1. Compute prerequisites for each client
    let mut clients_prerequisites = Vec::with_capacity(clients.len());
    for client in clients {
      let prerequisites = self.compute_out_hmr_prerequisites(
        stale_modules,
        first_invalidated_by.as_deref(),
        &client.executed_modules,
      );

      tracing::debug!(
        target: "hmr",
        "computed prerequisites for client {}: boundaries {:?}, require_full_reload: {}",
        client.client_id,
        prerequisites.boundaries.iter()
          .map(|boundary| self.module_table().modules[boundary.boundary].stable_id())
          .collect::<Vec<_>>(),
        prerequisites.require_full_reload,
      );

      clients_prerequisites.push((client.client_id.clone(), prerequisites));
    }

    // 2. Do ONE module refetch and cache merge (if needed)
    let new_added_modules = if changed_modules.is_empty() {
      FxIndexSet::default()
    } else {
      let modules_to_be_refetched = changed_modules
        .iter()
        .filter_map(|module_idx| {
          let module = &self.module_table().modules[*module_idx];
          if let Module::Normal(module) = module {
            Some(module.originative_resolved_id.clone())
          } else {
            // unreachable!("HMR only supports normal module. Got {:?}", module.id());
            None
          }
        })
        .collect::<Vec<_>>();

      let fetch_mode = ScanMode::Partial(modules_to_be_refetched);

      let mut module_loader = ModuleLoader::new(
        self.fs.clone(),
        Arc::clone(&self.options),
        Arc::clone(&self.resolver),
        Arc::clone(&self.plugin_driver),
        self.cache,
        fetch_mode.is_full(),
      )?;

      let module_loader_output = module_loader.fetch_modules(fetch_mode).await?;

      // We manually impl `Drop` for `ModuleLoader` to avoid missing assign `importers` to
      // `self.cache`, but rustc is not smart enough to infer actually we don't touch it in `drop`
      // implementation, so we need to manually drop it.
      drop(module_loader);

      let new_added_modules = module_loader_output.new_added_modules_from_partial_scan.clone();

      tracing::debug!(
        target: "hmr",
        "New added modules: {:?}",
        new_added_modules
          .iter()
          .map(|module_idx| module_loader_output.module_table.get(*module_idx).stable_id())
          .collect::<Vec<_>>(),
      );

      self.cache.merge(module_loader_output.into());
      new_added_modules
    };

    // 3. For each client, render their HMR patch or return full reload
    let mut client_updates = Vec::with_capacity(clients.len());
    for (client_id, prerequisites) in clients_prerequisites {
      let update = if prerequisites.require_full_reload {
        HmrUpdate::FullReload {
          reason: prerequisites.full_reload_reason.unwrap_or_else(|| "Unknown reason".to_string()),
        }
      } else {
        self.render_hmr_patch_from_prerequisites(prerequisites, &new_added_modules).await?
      };

      client_updates.push(ClientHmrUpdate { client_id, update });
    }

    Ok(client_updates)
  }

  // Kept for backwards compatibility - this method is no longer used but kept in case
  // it's needed for other use cases
  #[expect(dead_code)]
  #[expect(clippy::too_many_lines)]
  async fn compute_hmr_update_single(
    &mut self,
    stale_modules: &FxIndexSet<ModuleIdx>,
    changed_modules: &FxIndexSet<ModuleIdx>,
    first_invalidated_by: Option<String>,
    executed_modules: &FxHashSet<String>,
  ) -> BuildResult<HmrUpdate> {
    let hmr_prerequisites = self.compute_out_hmr_prerequisites(
      stale_modules,
      first_invalidated_by.as_deref(),
      executed_modules,
    );

    tracing::debug!(
      target: "hmr",
      "computed out `hmr_boundaries` {:?}",
      hmr_prerequisites.boundaries.iter()
        .map(|boundary| self.module_table().modules[boundary.boundary].stable_id())
        .collect::<Vec<_>>(),
    );

    tracing::debug!(
      target: "hmr",
      "computed out `stale_modules` {:?}",
      hmr_prerequisites.modules_to_be_updated.iter()
        .map(|module_idx| self.module_table().modules[*module_idx].stable_id())
        .collect::<Vec<_>>(),
    );

    let mut modules_to_be_updated = hmr_prerequisites.modules_to_be_updated;

    if !changed_modules.is_empty() {
      let modules_to_be_refetched = changed_modules
        .iter()
        .filter_map(|module_idx| {
          let module = &self.module_table().modules[*module_idx];
          if let Module::Normal(module) = module {
            Some(module.originative_resolved_id.clone())
          } else {
            // unreachable!("HMR only supports normal module. Got {:?}", module.id());
            None
          }
        })
        .collect::<Vec<_>>();

      let fetch_mode = ScanMode::Partial(modules_to_be_refetched);

      let mut module_loader = ModuleLoader::new(
        self.fs.clone(),
        Arc::clone(&self.options),
        Arc::clone(&self.resolver),
        Arc::clone(&self.plugin_driver),
        self.cache,
        fetch_mode.is_full(),
      )?;

      let module_loader_output = module_loader.fetch_modules(fetch_mode).await?;

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
      modules_to_be_updated
        .extend(module_loader_output.new_added_modules_from_partial_scan.clone());
      self.cache.merge(module_loader_output.into());

      // Note: New added modules might include external modules. There's no way to "update" them, so we need to remove them.
      modules_to_be_updated.retain(|idx| self.module_table().modules[*idx].is_normal());
    }

    if hmr_prerequisites.require_full_reload {
      return Ok(HmrUpdate::FullReload {
        reason: hmr_prerequisites
          .full_reload_reason
          .unwrap_or_else(|| "Unknown reason".to_string()),
      });
    }

    // Sorting `modules_to_be_updated` is not strictly necessary, but it:
    // - Makes the snapshot more stable when we change logic that affects the order of modules.
    modules_to_be_updated
      .sort_by_cached_key(|module_idx| self.module_table().modules[*module_idx].id());

    let module_idx_to_init_fn_name = modules_to_be_updated
      .iter()
      .enumerate()
      .map(|(index, module_idx)| {
        let Module::Normal(module) = &self.module_table().modules[*module_idx] else {
          unreachable!(
            "External modules should be removed before. But got {:?}",
            self.module_table().modules[*module_idx].id()
          );
        };
        let prefix = if module.exports_kind.is_commonjs() { "require" } else { "init" };

        // We use `index` as a part of the function name to avoid name collision without needing to deconflict.
        (*module_idx, format!("{}_{}_{}", prefix, module.repr_name, index))
      })
      .collect::<FxHashMap<_, _>>();

    let index_ecma_ast = self.index_ecma_ast();
    let module_render_inputs = modules_to_be_updated
      .iter()
      .copied()
      .map(|affected_module_idx| {
        let affected_module = &self.module_table().modules[affected_module_idx];
        let Module::Normal(affected_module) = affected_module else {
          unreachable!("HMR only supports normal module");
        };

        debug_assert_eq!(affected_module_idx, affected_module.idx);
        let ecma_ast =
          index_ecma_ast[affected_module_idx].as_ref().expect("Normal module should have an AST");

        ModuleRenderInput {
          idx: affected_module.idx,
          ecma_ast: ecma_ast.clone_with_another_arena(),
        }
      })
      .collect::<Vec<_>>();

    let mut source_joiner = SourceJoiner::default();
    let rendered_sources = module_render_inputs
      .into_par_iter()
      .enumerate()
      .flat_map(|(index, render_input)| {
        let ModuleRenderInput { idx: affected_module_idx, ecma_ast: mut ast } = render_input;

        let affected_module = &self.module_table().modules[affected_module_idx];
        let Module::Normal(affected_module) = affected_module else {
          unreachable!("HMR only supports normal module");
        };

        let enable_sourcemap = self.options.sourcemap.is_some() && !affected_module.is_virtual();
        let use_pife_for_module_wrappers =
          self.options.optimization.is_pife_for_module_wrappers_enabled();
        let modules = &self.module_table().modules;

        ast.program.with_mut(|fields| {
          let scoping = EcmaAst::make_semantic(fields.program, /*with_cfg*/ false).into_scoping();

          let mut finalizer = HmrAstFinalizer {
            modules,
            alloc: fields.allocator,
            snippet: AstSnippet::new(fields.allocator),
            builder: &oxc::ast::AstBuilder::new(fields.allocator),
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

          traverse_mut(&mut finalizer, fields.allocator, fields.program, scoping, ());
        });

        let codegen = EcmaCompiler::print_with(
          &ast,
          PrintOptions {
            sourcemap: enable_sourcemap,
            filename: affected_module.id.to_string(),
            print_legal_comments: false, // ignore hmr chunk comments
            initial_indent: 0,
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

    hmr_prerequisites.boundaries.iter().for_each(|boundary| {
      let init_fn_name = &module_idx_to_init_fn_name[&boundary.accepted_via];
      source_joiner.append_source(format!("{init_fn_name}()"));
    });

    source_joiner.append_source(format!(
      "__rolldown_runtime__.applyUpdates([{}]);",
      hmr_prerequisites
        .boundaries
        .iter()
        .map(|boundary| {
          let boundary_mod = &self.module_table().modules[boundary.boundary];
          let accepted_via = &self.module_table().modules[boundary.accepted_via];
          format!("['{}', '{}']", boundary_mod.stable_id(), accepted_via.stable_id())
        })
        .collect::<Vec<_>>()
        .join(",")
    ));

    let (mut code, mut map) = source_joiner.join();

    let hmr_patch_id = self.next_hmr_patch_id.fetch_add(1, Ordering::Relaxed);
    let filename = format!("hmr_patch_{hmr_patch_id}.js");

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
      hmr_boundaries: hmr_prerequisites
        .boundaries
        .into_iter()
        .map(|boundary| HmrBoundaryOutput {
          boundary: self.module_table().modules[boundary.boundary].stable_id().into(),
          accepted_via: self.module_table().modules[boundary.accepted_via].stable_id().into(),
        })
        .collect(),
    }))
  }

  #[expect(clippy::too_many_lines)]
  async fn render_hmr_patch_from_prerequisites(
    &self,
    hmr_prerequisites: HmrPrerequisites,
    new_added_modules: &FxIndexSet<ModuleIdx>,
  ) -> BuildResult<HmrUpdate> {
    let mut modules_to_be_updated = hmr_prerequisites.modules_to_be_updated;

    // Extend with newly added modules from refetch
    modules_to_be_updated.extend(new_added_modules.iter().copied());
    // Note: New added modules might include external modules. There's no way to "update" them, so we need to remove them.
    modules_to_be_updated.retain(|idx| self.module_table().modules[*idx].is_normal());

    // Sorting `modules_to_be_updated` is not strictly necessary, but it:
    // - Makes the snapshot more stable when we change logic that affects the order of modules.
    modules_to_be_updated
      .sort_by_cached_key(|module_idx| self.module_table().modules[*module_idx].id());

    let module_idx_to_init_fn_name = modules_to_be_updated
      .iter()
      .enumerate()
      .map(|(index, module_idx)| {
        let Module::Normal(module) = &self.module_table().modules[*module_idx] else {
          unreachable!(
            "External modules should be removed before. But got {:?}",
            self.module_table().modules[*module_idx].id()
          );
        };
        let prefix = if module.exports_kind.is_commonjs() { "require" } else { "init" };

        // We use `index` as a part of the function name to avoid name collision without needing to deconflict.
        (*module_idx, format!("{}_{}_{}", prefix, module.repr_name, index))
      })
      .collect::<FxHashMap<_, _>>();

    let index_ecma_ast = self.index_ecma_ast();
    let module_render_inputs = modules_to_be_updated
      .iter()
      .copied()
      .map(|affected_module_idx| {
        let affected_module = &self.module_table().modules[affected_module_idx];
        let Module::Normal(affected_module) = affected_module else {
          unreachable!("HMR only supports normal module");
        };

        debug_assert_eq!(affected_module_idx, affected_module.idx);
        let ecma_ast =
          index_ecma_ast[affected_module_idx].as_ref().expect("Normal module should have an AST");

        ModuleRenderInput {
          idx: affected_module.idx,
          ecma_ast: ecma_ast.clone_with_another_arena(),
        }
      })
      .collect::<Vec<_>>();

    let mut source_joiner = SourceJoiner::default();
    let rendered_sources = module_render_inputs
      .into_par_iter()
      .enumerate()
      .flat_map(|(index, render_input)| {
        let ModuleRenderInput { idx: affected_module_idx, ecma_ast: mut ast } = render_input;

        let affected_module = &self.module_table().modules[affected_module_idx];
        let Module::Normal(affected_module) = affected_module else {
          unreachable!("HMR only supports normal module");
        };

        let enable_sourcemap = self.options.sourcemap.is_some() && !affected_module.is_virtual();
        let use_pife_for_module_wrappers =
          self.options.optimization.is_pife_for_module_wrappers_enabled();
        let modules = &self.module_table().modules;

        ast.program.with_mut(|fields| {
          let scoping = EcmaAst::make_semantic(fields.program, /*with_cfg*/ false).into_scoping();

          let mut finalizer = HmrAstFinalizer {
            modules,
            alloc: fields.allocator,
            snippet: AstSnippet::new(fields.allocator),
            builder: &oxc::ast::AstBuilder::new(fields.allocator),
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

          traverse_mut(&mut finalizer, fields.allocator, fields.program, scoping, ());
        });

        let codegen = EcmaCompiler::print_with(
          &ast,
          PrintOptions {
            sourcemap: enable_sourcemap,
            filename: affected_module.id.to_string(),
            print_legal_comments: false, // ignore hmr chunk comments
            initial_indent: 0,
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

    hmr_prerequisites.boundaries.iter().for_each(|boundary| {
      let init_fn_name = &module_idx_to_init_fn_name[&boundary.accepted_via];
      source_joiner.append_source(format!("{init_fn_name}()"));
    });

    source_joiner.append_source(format!(
      "__rolldown_runtime__.applyUpdates([{}]);",
      hmr_prerequisites
        .boundaries
        .iter()
        .map(|boundary| {
          let boundary_mod = &self.module_table().modules[boundary.boundary];
          let accepted_via = &self.module_table().modules[boundary.accepted_via];
          format!("['{}', '{}']", boundary_mod.stable_id(), accepted_via.stable_id())
        })
        .collect::<Vec<_>>()
        .join(",")
    ));

    let (mut code, mut map) = source_joiner.join();

    let hmr_patch_id = self.next_hmr_patch_id.fetch_add(1, Ordering::Relaxed);
    let filename = format!("hmr_patch_{hmr_patch_id}.js");

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
      hmr_boundaries: hmr_prerequisites
        .boundaries
        .into_iter()
        .map(|boundary| HmrBoundaryOutput {
          boundary: self.module_table().modules[boundary.boundary].stable_id().into(),
          accepted_via: self.module_table().modules[boundary.accepted_via].stable_id().into(),
        })
        .collect(),
    }))
  }

  fn propagate_update(
    &self,
    module_idx: ModuleIdx,
    hmr_boundaries: &mut FxIndexSet<HmrBoundary>,
    propagate_stack: &mut Vec<ModuleIdx>,
    modules_to_be_updated: &mut FxIndexSet<ModuleIdx>,
    executed_modules: &FxHashSet<String>,
  ) -> PropagateUpdateStatus {
    modules_to_be_updated.insert(module_idx);

    let Module::Normal(module) = &self.module_table().modules[module_idx] else {
      // We consider reaching external modules as a boundary.
      return PropagateUpdateStatus::ReachHmrBoundary;
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
        // Note: our traversal is done by reaching `importers`, so the vec order is opposite to the import order.
        .rev()
        .collect::<Vec<_>>();

      return PropagateUpdateStatus::Circular(cycle_chain);
    }

    if module.is_hmr_self_accepting_module() {
      hmr_boundaries.insert(HmrBoundary { boundary: module_idx, accepted_via: module_idx });
      return PropagateUpdateStatus::ReachHmrBoundary;
    } else if module.importers_idx.is_empty() {
      // This module is not self-accepting and doesn't have any potential importer that might accept its update
      return PropagateUpdateStatus::NoBoundary(module_idx);
    }

    let mut importers_idx = module.importers_idx.iter().copied().collect::<Vec<_>>();
    // FIXME(hyf0): In practice, the order of importers doesn't matter since we're going to traverse all of them.
    // However, non-deterministic order causes unstable snapshots.
    importers_idx
      .sort_by_key(|importer_idx| self.module_table().modules[*importer_idx].stable_id());

    for importer_idx in importers_idx {
      let Module::Normal(importer) = &self.module_table().modules[importer_idx] else {
        continue;
      };

      if !executed_modules.contains(&importer.stable_id) {
        // If this module is not registered, we simply ignore it.
        continue;
      }

      if importer.can_accept_hmr_dependency_for(&module.id) {
        modules_to_be_updated.insert(module_idx);
        hmr_boundaries.insert(HmrBoundary { boundary: importer_idx, accepted_via: module_idx });
        continue;
      }

      propagate_stack.push(module_idx);
      let status = self.propagate_update(
        importer_idx,
        hmr_boundaries,
        propagate_stack,
        modules_to_be_updated,
        executed_modules,
      );
      propagate_stack.pop();
      if !status.is_reach_hmr_boundary() {
        return status;
      }
    }

    PropagateUpdateStatus::ReachHmrBoundary
  }

  fn compute_out_hmr_prerequisites(
    &self,
    stale_modules: &FxIndexSet<ModuleIdx>,
    first_invalidated_by: Option<&str>,
    executed_modules: &FxHashSet<String>,
  ) -> HmrPrerequisites {
    let mut hmr_boundaries = FxIndexSet::default();
    let mut require_full_reload = false;
    let mut full_reload_reason = None;
    let mut modules_to_be_updated = FxIndexSet::default();

    for stale_module in stale_modules.iter().copied() {
      if require_full_reload {
        break;
      }
      let mut boundaries = FxIndexSet::default();

      if !executed_modules.contains(self.module_table().modules[stale_module].stable_id()) {
        // If this module is not registered, we simply ignore it.
        continue;
      }

      let propagate_update_status = self.propagate_update(
        stale_module,
        &mut boundaries,
        &mut vec![],
        &mut modules_to_be_updated,
        executed_modules,
      );

      match propagate_update_status {
        PropagateUpdateStatus::Circular(cycle_chain) => {
          require_full_reload = true;
          full_reload_reason = Some(format!(
            "circular import chain: {}",
            cycle_chain
              .iter()
              .map(|module_idx| self.module_table().modules[*module_idx].stable_id())
              .collect::<Vec<_>>()
              .join(" -> ")
          ));
          break;
        }
        PropagateUpdateStatus::NoBoundary(idx) => {
          require_full_reload = true;
          let module = &self.module_table().modules[idx];
          full_reload_reason =
            Some(format!("no hmr boundary found for module `{}`", module.stable_id()));
          break;
        }
        PropagateUpdateStatus::ReachHmrBoundary => {}
      }

      // If import.meta.hot.invalidate was already called on that module for the same update,
      // it means any importer of that module can't hot update. We should fall back to full reload.
      if let Some(first_invalidated_by) = first_invalidated_by.as_ref() {
        if boundaries.iter().any(|boundary| {
          self.module_table().modules[boundary.accepted_via].stable_id() == *first_invalidated_by
        }) {
          require_full_reload = true;
          // full_reload_reason = Some("circular import invalidate".to_string());
          continue;
        }
      }

      hmr_boundaries.extend(boundaries);
    }

    HmrPrerequisites {
      boundaries: hmr_boundaries,
      modules_to_be_updated,
      require_full_reload,
      full_reload_reason,
    }
  }
}

struct HmrPrerequisites {
  boundaries: FxIndexSet<HmrBoundary>,
  modules_to_be_updated: FxIndexSet<ModuleIdx>,
  require_full_reload: bool,
  full_reload_reason: Option<String>,
}

enum PropagateUpdateStatus {
  Circular(Vec<ModuleIdx>), // The circular dependency chain
  ReachHmrBoundary,
  NoBoundary(ModuleIdx),
}

impl PropagateUpdateStatus {
  pub fn is_reach_hmr_boundary(&self) -> bool {
    matches!(self, Self::ReachHmrBoundary)
  }
}

struct ModuleRenderInput {
  pub idx: ModuleIdx,
  pub ecma_ast: EcmaAst,
}
