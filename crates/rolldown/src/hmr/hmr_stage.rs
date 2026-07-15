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
  ClientHmrInput, ClientHmrUpdate, HmrLazyChunkOutput, HmrPatch, HmrUpdate, ImportKind, Module,
  ModuleIdx, ModuleTable, ScanMode, WatcherChangeKind,
};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler, PrintCommentsOptions, PrintOptions};
use rolldown_ecmascript_utils::AstFactory;
use rolldown_error::BuildResult;
use rolldown_fs::FileSystem;
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

pub struct HmrStageInput<'a, Fs: FileSystem + Clone + 'static> {
  pub options: SharedOptions,
  pub fs: Fs,
  pub resolver: SharedResolver<Fs>,
  pub plugin_driver: SharedPluginDriver,
  pub cache: &'a mut ScanStageCache,
  pub next_hmr_patch_id: Arc<AtomicU32>,
}

impl<Fs: FileSystem + Clone + 'static> HmrStageInput<'_, Fs> {
  pub fn module_table(&self) -> &ModuleTable {
    &self.cache.get_snapshot().module_table
  }

  pub fn index_ecma_ast(&self) -> &IndexEcmaAst {
    &self.cache.get_snapshot().index_ecma_ast
  }
}

pub struct HmrStage<'a, Fs: FileSystem + Clone + 'static> {
  pub(crate) input: HmrStageInput<'a, Fs>,
}

impl<'a, Fs: FileSystem + Clone + 'static> Deref for HmrStage<'a, Fs> {
  type Target = HmrStageInput<'a, Fs>;

  fn deref(&self) -> &Self::Target {
    &self.input
  }
}

impl<Fs: FileSystem + Clone + 'static> DerefMut for HmrStage<'_, Fs> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.input
  }
}

impl<'a, Fs: FileSystem + Clone + 'static> HmrStage<'a, Fs> {
  pub fn new(input: HmrStageInput<'a, Fs>) -> Self {
    Self { input }
  }

  pub async fn compute_hmr_update_for_file_changes(
    &mut self,
    changed_file_paths: &FxIndexMap<String, WatcherChangeKind>,
    clients: &[ClientHmrInput<'_>],
  ) -> BuildResult<Vec<ClientHmrUpdate>> {
    tracing::trace!(
      "[HmrStage] starts computing HMR updates\n - changed_file_paths: {:#?}\n - clients: {:#?}",
      changed_file_paths,
      clients.iter().map(|c| c.client_id).collect::<Vec<_>>(),
    );

    // 1. Identify changed modules
    let mut changed_modules = FxIndexSet::default();
    for (changed_file_path, event) in changed_file_paths {
      let changed_file_path = ArcStr::from(changed_file_path.to_slash());
      // Check if the file itself is a module
      if let Some(module_idx) = self.cache.module_idx_by_abs_path.get(&changed_file_path) {
        if *event == WatcherChangeKind::Delete {
          if let Some(importers) = self.cache.importers.get(*module_idx) {
            changed_modules.extend(importers.iter().map(|imp| imp.importer_idx));
          }
        } else {
          changed_modules.insert(*module_idx);
        }
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

    tracing::trace!(
      "[HmrStage] map changed file paths to module idxs\n - changed_modules: {:#?}",
      changed_modules
        .iter()
        .map(|module_idx| self.module_table().modules[*module_idx].stable_id())
        .collect::<Vec<_>>(),
    );

    if changed_modules.is_empty() {
      return Ok(
        clients
          .iter()
          .map(|client| ClientHmrUpdate {
            client_id: client.client_id.to_string(),
            update: HmrUpdate::Noop,
          })
          .collect(),
      );
    }

    // Files re-queued by an earlier failed scan (`pending_rescans`) get
    // re-fetched and merged by this update as well. Treat them as changed so
    // their recovered content reaches the clients' patches; otherwise only
    // the server-side graph would learn about their edits.
    for resolved_id in &self.cache.pending_rescans {
      if let Some(state) = self.cache.module_id_to_idx.get(&resolved_id.id) {
        changed_modules.insert(state.idx());
      }
    }

    // 1. Do ONE module refetch and cache merge — the update-superset walk (which
    // selects the factories to ship) runs on the post-rebuild table; boundary
    // decisions belong to the client's own walk.
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
        None,
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

      let plugin_driver = Arc::clone(&self.plugin_driver);
      self.cache.merge(module_loader_output.into(), &plugin_driver)?;

      let options = Arc::clone(&self.options);
      self.cache.update_defer_sync_data(&options).await?;
      new_added_modules
    };

    // 2. Collect the factories to ship. The client's walk may remove modules from its cache and re-run
    // anything up the changed ids' importer chains, and a re-run without a resident
    // factory forces a reload — so the server must ship a SUPERSET of any client's
    // possible update set, over-approximated on static truth (it never sees runtime
    // acceptance).
    let changed_ids = changed_modules
      .iter()
      .filter_map(|module_idx| {
        self.module_table().modules[*module_idx]
          .as_normal()
          .map(|module| module.stable_id.to_string())
      })
      .collect::<Vec<_>>();

    let mut client_updates = Vec::with_capacity(clients.len());
    match self.collect_client_update_superset(&changed_modules) {
      UpdateSupersetOutcome::FullReload { reason } => {
        for client in clients {
          client_updates.push(ClientHmrUpdate {
            client_id: client.client_id.to_string(),
            update: HmrUpdate::FullReload { reason: reason.clone() },
          });
        }
      }
      UpdateSupersetOutcome::Superset(mut affected) => {
        affected.extend(new_added_modules.iter().copied());
        affected.retain(|idx| self.module_table().modules[*idx].is_normal());

        // 3. Every client receives the full affected set. The per-client ship map
        // (`shipped[C]`) that narrows this to what each tab lacks lands in a
        // follow-up; until then the patch depends on no per-client input, so render
        // it once and fan the same payload out (`seq` is stamped per client after
        // compute).
        if !clients.is_empty() {
          let update = self.render_hmr_patch(affected, changed_ids).await?;
          for client in clients {
            client_updates.push(ClientHmrUpdate {
              client_id: client.client_id.to_string(),
              update: update.clone(),
            });
          }
        }
      }
    }

    Ok(client_updates)
  }

  /// Collect the superset of modules that need to be sent to a client, given the changed modules.
  fn collect_client_update_superset(
    &self,
    changed_modules: &FxIndexSet<ModuleIdx>,
  ) -> UpdateSupersetOutcome {
    let mut affected = FxIndexSet::default();
    for changed in changed_modules.iter().copied() {
      match self.propagate_update(changed, &mut vec![], &mut affected) {
        PropagateUpdateStatus::Circular(cycle_chain) => {
          return UpdateSupersetOutcome::FullReload {
            reason: format!(
              "circular import chain: {}",
              cycle_chain
                .iter()
                .map(|module_idx| self.module_table().modules[*module_idx].stable_id().as_str())
                .collect::<Vec<_>>()
                .join(" -> ")
            ),
          };
        }
        PropagateUpdateStatus::NoBoundary(idx) => {
          return UpdateSupersetOutcome::FullReload {
            reason: format!(
              "no hmr boundary found for module `{}`",
              self.module_table().modules[idx].stable_id()
            ),
          };
        }
        PropagateUpdateStatus::ReachHmrBoundary => {}
      }
    }
    UpdateSupersetOutcome::Superset(affected)
  }

  /// Compile a lazy entry module and return the compiled chunk.
  ///
  /// The chunk carries every reachable sync dependency's factory — never filtered by
  /// execution state. The per-client ship map that narrows this to what each
  /// tab lacks lands in a follow-up; re-shipped factories are idempotent.
  pub async fn compile_lazy_entry(
    &mut self,
    module_id: &str,
    _client_id: &str,
  ) -> BuildResult<HmrLazyChunkOutput> {
    tracing::debug!(
      target: "hmr",
      "compile_lazy_entry: module_id: {:?}",
      module_id,
    );

    // module_id is the proxy module ID (e.g., "/abs/path/async-entry-a.js?rolldown-lazy=1")
    // The proxy has been marked as fetched, so the lazy compilation plugin's load hook
    // will return the fetched template which imports the real module.

    // A failed full build leaves `module_id_to_idx` repopulated but the
    // snapshot unset; `module_table()` would panic. Surface an error the
    // client can handle instead.
    if !self.cache.has_snapshot() {
      return Err(
        vec![
          anyhow::anyhow!(
            "Cannot compile lazy entry `{module_id}`: the last full build failed, so there is no module graph to compile against."
          )
          .into(),
        ]
        .into(),
      );
    }

    // 1. Get the originative resolved_id from the cached module
    // The proxy module should already be in the cache from the initial build.
    let (entry_module_idx, resolved_id) = self
      .cache
      .module_id_to_idx
      .get(module_id)
      .and_then(|state| {
        let idx = state.idx();
        let module = &self.module_table().modules[idx];
        if let Module::Normal(module) = module {
          Some((idx, module.originative_resolved_id.clone()))
        } else {
          None
        }
      })
      .ok_or_else(|| {
        vec![anyhow::anyhow!("Lazy entry module not found in cache. module_id={module_id}").into()]
      })?;

    // 2. Trigger a partial scan to fetch the module and its dependencies
    let fetch_mode = ScanMode::Partial(vec![resolved_id]);

    let mut module_loader = ModuleLoader::new(
      self.fs.clone(),
      Arc::clone(&self.options),
      Arc::clone(&self.resolver),
      Arc::clone(&self.plugin_driver),
      self.cache,
      fetch_mode.is_full(),
      None,
    )?;

    let module_loader_output = module_loader.fetch_modules(fetch_mode).await?;
    drop(module_loader);

    let plugin_driver = Arc::clone(&self.plugin_driver);
    self
      .cache
      .merge(module_loader_output.into(), &plugin_driver)
      .map_err(|e| vec![anyhow::anyhow!(e).into()])?;

    let options = Arc::clone(&self.options);
    self.cache.update_defer_sync_data(&options).await?;

    // Collect all reachable sync dependencies. Overlapping lazy compiles re-ship
    // shared factories — duplicate idempotent bytes, never a missing factory.
    let mut modules_to_be_updated = FxIndexSet::default();
    self.collect_sync_dependencies_for_client(entry_module_idx, &mut modules_to_be_updated);

    // Remove external modules - no way to "compile" them
    modules_to_be_updated.retain(|idx| self.module_table().modules[*idx].is_normal());

    // Sort for stable output
    modules_to_be_updated
      .sort_by_cached_key(|module_idx| self.module_table().modules[*module_idx].id());

    // Prepare module render inputs
    let index_ecma_ast = self.index_ecma_ast();
    let module_render_inputs = modules_to_be_updated
      .iter()
      .copied()
      .map(|affected_module_idx| {
        let affected_module = &self.module_table().modules[affected_module_idx];
        let Module::Normal(affected_module) = affected_module else {
          unreachable!("Only normal modules should be rendered");
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

    // Render all modules
    let mut source_joiner = SourceJoiner::default();
    // Rows first — includes the proxy-id row (proxy → real entry), which replaces the
    // stub's edgeless row and commits the swap as data.
    if let Some(prelude) = crate::hmr::module_graph_delta::render_register_graph_source(
      self.module_table(),
      modules_to_be_updated.iter().copied(),
    ) {
      source_joiner.append_source(prelude);
    }
    let rendered_sources = module_render_inputs
      .into_par_iter()
      .enumerate()
      .flat_map(|(index, render_input)| {
        let ModuleRenderInput { idx: affected_module_idx, ecma_ast: mut ast } = render_input;

        let affected_module = &self.module_table().modules[affected_module_idx];
        let Module::Normal(affected_module) = affected_module else {
          unreachable!("Only normal modules should be rendered");
        };

        let enable_sourcemap = self.options.sourcemap.is_some() && !affected_module.is_virtual();
        let use_pife_for_module_wrappers =
          self.options.optimization.is_pife_for_module_wrappers_enabled();
        let modules = &self.module_table().modules;

        ast.program.with_mut(|fields| {
          // Re-running semantic re-stamps every NodeId. The NodeId-keyed side-table lookups
          // below still hit only because the clone is unmutated at this point: identical tree
          // shape re-derives exactly the scan-time ids (see internal-docs/ast-mutation/implementation.md).
          let scoping = EcmaAst::make_semantic(fields.program).into_scoping();

          let mut finalizer = HmrAstFinalizer {
            modules,
            ast_factory: AstFactory::new(fields.allocator),
            import_bindings: FxHashMap::default(),
            module: affected_module,
            exports: oxc::allocator::Vec::new_in(&fields.allocator),
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
            comments: PrintCommentsOptions {
              legal: false,
              annotation: self.options.comments.annotation,
              jsdoc: self.options.comments.jsdoc,
            },
            initial_indent: 0,
          },
        );

        let intro_comment: Box<dyn Source + Send> =
          Box::new(concat_string!("//#region ", affected_module.debug_id));
        let outro_comment: Box<dyn Source + Send> = Box::new(concat_string!("//#endregion"));

        let code_source: Box<dyn Source + Send> = if let Some(map) = codegen.map {
          Box::new(SourceMapSource::new(codegen.code, map.into_owned()))
        } else {
          Box::new(codegen.code)
        };

        [intro_comment, code_source, outro_comment]
      })
      .collect::<Vec<_>>();

    for source in rendered_sources {
      source_joiner.append_source_dyn(source);
    }

    // A lazy chunk is delivery + execute-entry — no walk, no cache removals. The tail is the
    // one uniform re-execution gate: the stub removed the proxy id from the cache, so this misses the
    // registry and runs the fetched-template factory.
    let entry_stable_id = self.module_table().modules[entry_module_idx].stable_id().as_str();
    source_joiner.append_source(format!(
      "__rolldown_runtime__.initModule({})",
      json_escape_simd::escape(entry_stable_id)
    ));

    let (mut code, mut map) = source_joiner.join();

    let lazy_patch_id = self.next_hmr_patch_id.fetch_add(1, Ordering::Relaxed);
    let filename = format!("lazy_compile_{lazy_patch_id}.js");

    let file_dir = self.options.cwd.as_path().join(&self.options.out_dir);

    if let Some(map) = map.as_mut() {
      process_code_and_sourcemap(
        &self.options,
        &mut code,
        map,
        &file_dir,
        filename.as_str(),
        0,
        /*is_css*/ false,
        None,
      )
      .await?;
    }

    Ok(HmrLazyChunkOutput { code, filename })
  }

  async fn render_hmr_patch(
    &self,
    mut carried_modules: FxIndexSet<ModuleIdx>,
    changed_ids: Vec<String>,
  ) -> BuildResult<HmrUpdate> {
    // Note: the carried set might include external modules. There's no way to "update" them, so we need to remove them.
    carried_modules.retain(|idx| self.module_table().modules[*idx].is_normal());

    // Sorting `carried_modules` is not strictly necessary, but it:
    // - Makes the snapshot more stable when we change logic that affects the order of modules.
    carried_modules
      .sort_by_cached_key(|module_idx| self.module_table().modules[*module_idx].id().as_str());

    let index_ecma_ast = self.index_ecma_ast();
    let module_render_inputs = carried_modules
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
    // The graph-rows manifest is the first source of every payload: pure topology the
    // client-side walk consumes, landing before any factory registers.
    if let Some(prelude) = crate::hmr::module_graph_delta::render_register_graph_source(
      self.module_table(),
      carried_modules.iter().copied(),
    ) {
      source_joiner.append_source(prelude);
    }
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
          // Re-running semantic re-stamps every NodeId. The NodeId-keyed side-table lookups
          // below still hit only because the clone is unmutated at this point: identical tree
          // shape re-derives exactly the scan-time ids (see internal-docs/ast-mutation/implementation.md).
          let scoping = EcmaAst::make_semantic(fields.program).into_scoping();

          let mut finalizer = HmrAstFinalizer {
            modules,
            ast_factory: AstFactory::new(fields.allocator),
            import_bindings: FxHashMap::default(),
            module: affected_module,
            exports: oxc::allocator::Vec::new_in(&fields.allocator),
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
            comments: PrintCommentsOptions {
              legal: false, // ignore hmr chunk comments
              annotation: self.options.comments.annotation,
              jsdoc: self.options.comments.jsdoc,
            },
            initial_indent: 0,
          },
        );

        let intro_comment: Box<dyn Source + Send> =
          Box::new(concat_string!("//#region ", affected_module.debug_id));
        let outro_comment: Box<dyn Source + Send> = Box::new(concat_string!("//#endregion"));

        let code_source: Box<dyn Source + Send> = if let Some(map) = codegen.map {
          Box::new(SourceMapSource::new(codegen.code, map.into_owned()))
        } else {
          Box::new(codegen.code)
        };

        [intro_comment, code_source, outro_comment]
      })
      .collect::<Vec<_>>();

    for source in rendered_sources {
      source_joiner.append_source_dyn(source);
    }

    // No driver tail: the client walks its own graph, removes from its cache, and re-runs from the
    // factory map. Importing this patch commits rows and factories, nothing more.

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
        None,
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
      changed_ids,
      // The envelope seq is a delivery-layer concern; the dev engine stamps it onto the
      // patches it actually sends (see `bundling_task`), so this is only a placeholder.
      seq: 0,
    }))
  }

  fn propagate_update(
    &self,
    module_idx: ModuleIdx,
    propagate_stack: &mut Vec<ModuleIdx>,
    modules_to_be_updated: &mut FxIndexSet<ModuleIdx>,
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
      tracing::trace!(
        "[HmrStage] module {} is self-accepting, stop propagation here",
        module.stable_id,
      );
      return PropagateUpdateStatus::ReachHmrBoundary;
    } else if module.importers_idx.is_empty() && module.dynamic_importers_idx.is_empty() {
      // This module is not self-accepting and doesn't have any potential importer that might accept its update
      return PropagateUpdateStatus::NoBoundary(module_idx);
    }

    // Static and dynamic `import()` importers are walked the same way for boundary
    // calculation — parity with Vite (`node.importers`) and webpack (`module.parents`),
    // neither of which distinguishes the edge kind. A module that both statically and
    // dynamically imports this one is deduped after the sort.
    let mut importers_idx = module
      .importers_idx
      .iter()
      .chain(module.dynamic_importers_idx.iter())
      .copied()
      .collect::<Vec<_>>();
    // FIXME(hyf0): In practice, the order of importers doesn't matter since we're going to traverse all of them.
    // However, non-deterministic order causes unstable snapshots.
    importers_idx
      .sort_unstable_by_key(|importer_idx| self.module_table().modules[*importer_idx].stable_id());
    importers_idx.dedup();

    for importer_idx in importers_idx {
      let Module::Normal(importer) = &self.module_table().modules[importer_idx] else {
        continue;
      };

      if importer.can_accept_hmr_dependency_for(&module.id) {
        tracing::trace!(
          "[HmrStage] importer {} can accept update for dependency {}, stop propagation here",
          importer.stable_id,
          module.stable_id,
        );
        // Edge boundary: the accepting importer is not re-run, so it joins no set.
        continue;
      }

      propagate_stack.push(module_idx);
      let status = self.propagate_update(importer_idx, propagate_stack, modules_to_be_updated);
      propagate_stack.pop();
      if !status.is_reach_hmr_boundary() {
        return status;
      }
    }

    PropagateUpdateStatus::ReachHmrBoundary
  }
}

enum UpdateSupersetOutcome {
  /// The superset of any client's possible update set
  Superset(FxIndexSet<ModuleIdx>),
  FullReload {
    reason: String,
  },
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

impl<Fs: FileSystem + Clone + 'static> HmrStage<'_, Fs> {
  fn collect_sync_dependencies_for_client(
    &self,
    proxy_entry_idx: ModuleIdx,
    result: &mut FxIndexSet<ModuleIdx>,
  ) {
    let modules = &self.module_table().modules;
    let mut stack = vec![proxy_entry_idx];

    while let Some(module_idx) = stack.pop() {
      if !result.insert(module_idx) {
        continue;
      }

      let Module::Normal(module) = &modules[module_idx] else {
        continue;
      };

      for rec in &module.import_records {
        // For the proxy entry module, also follow dynamic imports.
        // The proxy's fetched template has `import($MODULE_ID)` pointing to the real module.
        // We need to include the real module and its sync dependencies in the patch.
        let should_follow = rec.kind.is_static()
          || (module_idx == proxy_entry_idx && rec.kind == ImportKind::DynamicImport);

        if should_follow && let Some(dep_idx) = rec.resolved_module {
          stack.push(dep_idx);
        }
      }
    }
  }
}
