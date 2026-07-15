use std::{
  ops::{Deref, DerefMut},
  sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
  },
};

use arcstr::ArcStr;
use oxc::ast::builder::AstBuilder;
use oxc_traverse::traverse_mut;
use rolldown_common::{
  ClientHmrInput, ClientHmrUpdate, HmrLazyChunkOutput, HmrPatch, HmrStampTable, HmrUpdate,
  ImportKind, Module, ModuleIdx, ModuleTable, ScanMode, WatcherChangeKind,
};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler, PrintCommentsOptions, PrintOptions};
use rolldown_error::BuildResult;
use rolldown_fs::FileSystem;
use rolldown_plugin::SharedPluginDriver;
use rolldown_sourcemap::{Source, SourceJoiner, SourceMap, SourceMapSource};
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

/// Module ids the `hotUpdate` hook must not see or return: lazy-compilation proxies are internal
/// artifacts (cf. the dynamic-importer exclusion in `EcmaView`) and runtime modules are never
/// re-fetched. Runtime ids use both prefixes (cf. the same pair in `ChunkGraph`); the `\0` one
/// covers `RUNTIME_MODULE_KEY`.
fn is_hidden_from_hot_update_hook(id: &str) -> bool {
  id.contains("?rolldown-lazy=") || id.starts_with("rolldown:") || id.starts_with("\0rolldown/")
}

impl<'a, Fs: FileSystem + Clone + 'static> HmrStage<'a, Fs> {
  pub fn new(input: HmrStageInput<'a, Fs>) -> Self {
    Self { input }
  }

  /// Stage order is documented in `internal-docs/dev-engine/implementation.md`
  /// ("Inside `compute_hmr_update_for_file_changes`").
  #[expect(clippy::too_many_lines)]
  pub async fn compute_hmr_update_for_file_changes(
    &mut self,
    changed_file_paths: &FxIndexMap<String, WatcherChangeKind>,
    clients: &[ClientHmrInput<'_>],
    stamp_table: &mut HmrStampTable,
    last_build_errored: bool,
  ) -> BuildResult<Vec<ClientHmrUpdate>> {
    tracing::trace!(
      "[HmrStage] starts computing HMR updates\n - changed_file_paths: {:#?}\n - clients: {:#?}",
      changed_file_paths,
      clients.iter().map(|c| c.client_id).collect::<Vec<_>>(),
    );

    // 1. Identify changed modules — per changed file: compute the default affected set, then (if
    // any plugin registered `hotUpdate`) let the plugin replace-chain edit it before re-fetching.
    let hot_update_hook_registered = self.plugin_driver.has_hot_update_hook();
    let mut changed_modules = FxIndexSet::default();
    // Modules a `hotUpdate` hook explicitly selected (a plugin returned a replacement set).
    // They are exempt from the unchanged-output suppression below — like `last_build_errored`,
    // by skipping the pre-rebuild capture. An explicit return is a directive to re-run the
    // module in clients, and what changed can live outside the module's own code (e.g. a
    // watched non-module file the module reads at runtime), so identical output does not make
    // the update empty. Vite ships hook-returned modules unconditionally.
    let mut hook_selected_modules = FxHashSet::default();
    for (changed_file_path, event) in changed_file_paths {
      let changed_file_path = ArcStr::from(changed_file_path.to_slash());

      // Default affected set: the file's own module (kept even for deletes — the hook contract
      // passes the deleted module itself; importer expansion happens after the chain) plus every
      // module that has this file as a transform dependency.
      let own_module_idx = self.cache.module_idx_by_abs_path.get(&changed_file_path).copied();
      let mut affected_modules = FxIndexSet::default();
      if let Some(module_idx) = own_module_idx {
        affected_modules.insert(module_idx);
      }
      for entry in self.plugin_driver.transform_dependencies.iter() {
        let module_idx = *entry.key();
        if entry.value().contains(&changed_file_path) {
          affected_modules.insert(module_idx);
        }
      }

      let mut hook_replaced = false;
      if hot_update_hook_registered {
        // Plugins receive raw module ids and may replace the set; an empty return suppresses
        // this file's update. The chain also runs when the default set is empty — content
        // plugins claim files no module maps to. Ids the graph doesn't know are dropped.
        let default_ids = affected_modules
          .iter()
          .filter_map(|module_idx| match &self.module_table().modules[*module_idx] {
            Module::Normal(module) if !is_hidden_from_hot_update_hook(module.id.as_arc_str()) => {
              Some(module.id.as_arc_str().clone())
            }
            _ => None,
          })
          .collect::<Vec<_>>();
        let final_ids =
          self.plugin_driver.hot_update(*event, &changed_file_path, default_ids).await?;
        if let Some(final_ids) = final_ids {
          hook_replaced = true;
          affected_modules.clear();
          for id in final_ids {
            if is_hidden_from_hot_update_hook(&id) {
              continue;
            }
            // The map keys are `to_slash`ed module ids while hook ids are raw — without the same
            // normalization here, raw Windows ids would never round-trip back to their modules.
            if let Some(module_idx) = self.cache.module_idx_by_abs_path.get(&*id.to_slash()) {
              affected_modules.insert(*module_idx);
            } else {
              tracing::debug!(
                "[HmrStage] dropped unknown module id returned from the hotUpdate hook: {id}"
              );
            }
          }
        }
      }

      for module_idx in affected_modules {
        if *event == WatcherChangeKind::Delete && Some(module_idx) == own_module_idx {
          // A deleted module cannot be re-fetched — start the update from its importers.
          if let Some(importers) = self.cache.importers.get(module_idx) {
            changed_modules.extend(importers.iter().map(|imp| imp.importer_idx));
          }
        } else {
          changed_modules.insert(module_idx);
          if hook_replaced {
            hook_selected_modules.insert(module_idx);
          }
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

    // Files re-queued by an earlier failed scan (`pending_rescans`) get
    // re-fetched and merged by this update as well. Treat them as changed so
    // their recovered content reaches the clients' patches; otherwise only
    // the server-side graph would learn about their edits.
    //
    // This fold must come BEFORE the empty early-return below. A watched path
    // that maps to no module would otherwise return `Noop` without retrying the
    // rescans, and that empty success clears `last_task_errored` — so a later
    // restore of the broken file to its pre-break bytes would be suppressed as
    // unchanged, leaving clients stuck on the error overlay. Folding first
    // makes the empty event re-fetch the broken file, which keeps failing (and
    // keeps the latch set) until the file is actually fixed. The same ordering
    // also keeps a `hotUpdate` hook that suppresses an update from starving
    // the recovery.
    for resolved_id in &self.cache.pending_rescans {
      if let Some(state) = self.cache.module_id_to_idx.get(&resolved_id.id) {
        changed_modules.insert(state.idx());
      }
    }

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

    // After an errored build (`last_build_errored`) the capture is skipped, so
    // every changed module ships. A failed scan merges nothing, so undoing the
    // broken edit rebuilds byte-identical output — the graph can't tell "broke,
    // then fixed" from "nothing happened", but clients are stuck on the error
    // (overlay / fallback page) and a suppressed update would leave them there.
    let pre_rebuild_renders = if last_build_errored {
      FxHashMap::default()
    } else {
      let pre_rebuild_inputs = changed_modules
        .iter()
        .filter(|module_idx| !hook_selected_modules.contains(*module_idx))
        .filter_map(|module_idx| {
          self.module_table().modules[*module_idx].as_normal()?;
          let ecma_ast = self.index_ecma_ast()[*module_idx].as_ref()?;
          Some(ModuleRenderInput {
            idx: *module_idx,
            ecma_ast: ecma_ast.clone_with_another_arena(),
          })
        })
        .collect::<Vec<_>>();
      pre_rebuild_inputs
        .into_par_iter()
        .map(|render_input| {
          let module_idx = render_input.idx;
          (module_idx, self.render_module_code(render_input, 0, false).0)
        })
        .collect::<Vec<_>>()
        .into_iter()
        .collect::<FxHashMap<_, _>>()
    };

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

    // Drop the changed modules whose post-rebuild render is byte-identical to the
    // pre-rebuild capture: their output didn't change, so stamping and shipping
    // them would only make every client re-run code it already holds. A module
    // that can't be compared (no pre-rebuild render, or no longer normal) stays
    // in. Dep identity is part of the compared code (internal imports print as
    // `loadExports("<dep stable id>")`), so a resolution shift under an unchanged
    // source still ships.
    let recheck_inputs = changed_modules
      .iter()
      .filter(|module_idx| pre_rebuild_renders.contains_key(*module_idx))
      .filter_map(|module_idx| {
        self.module_table().modules[*module_idx].as_normal()?;
        let ecma_ast = self.index_ecma_ast()[*module_idx].as_ref()?;
        Some(ModuleRenderInput { idx: *module_idx, ecma_ast: ecma_ast.clone_with_another_arena() })
      })
      .collect::<Vec<_>>();
    let output_unchanged_modules = recheck_inputs
      .into_par_iter()
      .filter_map(|render_input| {
        let module_idx = render_input.idx;
        let (code, _) = self.render_module_code(render_input, 0, false);
        (pre_rebuild_renders[&module_idx] == code).then_some(module_idx)
      })
      .collect::<Vec<_>>()
      .into_iter()
      .collect::<FxHashSet<_>>();
    if !output_unchanged_modules.is_empty() {
      tracing::debug!(
        target: "hmr",
        "skip modules whose rebuilt output is unchanged: {:?}",
        output_unchanged_modules
          .iter()
          .map(|module_idx| self.module_table().modules[*module_idx].stable_id())
          .collect::<Vec<_>>(),
      );
      changed_modules.retain(|module_idx| !output_unchanged_modules.contains(module_idx));
    }

    // 2. Stamp the rebuild: `latest[m] = rebuild_seq` for every changed or newly added
    // module — the versioned ship map's staleness source.
    let rebuild_seq = stamp_table.begin_rebuild();
    for module_idx in changed_modules.iter().chain(new_added_modules.iter()) {
      if let Module::Normal(module) = &self.module_table().modules[*module_idx] {
        stamp_table.stamp(module.stable_id.as_arc_str(), rebuild_seq);
      }
    }

    // 3. Collect the factories to ship. The client's walk may remove modules from its cache and re-run
    // anything up the changed ids' importer chains, and a re-run without a resident
    // factory forces a reload — so the server must ship a SUPERSET of any client's
    // possible update set, over-approximated on static truth (it never sees runtime
    // acceptance). The ship map below subtracts what each tab already holds.
    let changed_ids = changed_modules
      .iter()
      .filter_map(|module_idx| {
        self.module_table().modules[*module_idx]
          .as_normal()
          .map(|module| module.stable_id.to_string())
      })
      .collect::<Vec<_>>();

    let mut affected = self.collect_client_update_superset(&changed_modules);
    affected.extend(new_added_modules.iter().copied());
    affected.retain(|idx| self.module_table().modules[*idx].is_normal());

    // Client-invariant per-module data, resolved once instead of per client:
    // `(idx, stable id, latest stamp)` for every affected module.
    let affected_with_stamps = affected
      .iter()
      .map(|module_idx| {
        let stable_id = self.module_table().modules[*module_idx].stable_id().as_str();
        (*module_idx, stable_id, stamp_table.render_time_stamp(stable_id))
      })
      .collect::<Vec<_>>();

    // 4. Per client: `need[C] = (affected ∖ shipped[C]) ∪ stale sweep`. The sweep
    // covers everything the client holds, not just `affected` — a parked factory can
    // go stale behind a skipped patch in either graph direction. It iterates the
    // stamp table (modules ever changed this session) rather than `shipped[C]`
    // (modules ever delivered): only stamped modules can be stale, and `latest`
    // stays far smaller than a tab's full delivery record.
    let mut client_updates = Vec::with_capacity(clients.len());
    for client in clients {
      let mut carried = FxIndexSet::default();
      for (module_idx, stable_id, latest_stamp) in &affected_with_stamps {
        match client.shipped.get(*stable_id) {
          None => {
            carried.insert(*module_idx);
          }
          Some(stamp) => {
            if *latest_stamp > *stamp {
              carried.insert(*module_idx);
            }
          }
        }
      }
      for (stable_id, latest_stamp) in stamp_table.iter_latest() {
        if client.shipped.get(stable_id.as_str()).is_some_and(|stamp| latest_stamp > *stamp) {
          if let Some(module_idx) =
            self.cache.module_idx_by_stable_id.get(stable_id.as_str()).copied()
          {
            if self.module_table().modules[module_idx].is_normal() {
              carried.insert(module_idx);
            }
          }
        }
      }

      let update = self.render_hmr_patch(carried, changed_ids.clone(), stamp_table).await?;
      client_updates.push(ClientHmrUpdate { client_id: client.client_id.to_string(), update });
    }

    Ok(client_updates)
  }

  /// Collect the superset of modules any client's walk may re-run for these changes:
  /// pure reachability over the importer graph (static ∪ dynamic edges), stopping at
  /// statically self-accepting modules and at accepting importer edges.
  fn collect_client_update_superset(
    &self,
    changed_modules: &FxIndexSet<ModuleIdx>,
  ) -> FxIndexSet<ModuleIdx> {
    let mut affected = FxIndexSet::default();
    let mut stack: Vec<ModuleIdx> = changed_modules.iter().copied().collect();
    while let Some(module_idx) = stack.pop() {
      if !affected.insert(module_idx) {
        // Already walked via another path (also breaks cycles).
        continue;
      }

      let Module::Normal(module) = &self.module_table().modules[module_idx] else {
        // Non-normal modules can't be re-run; they are filtered out of the patch later.
        continue;
      };

      if module.is_hmr_self_accepting_module() {
        tracing::trace!(
          "[HmrStage] module {} is self-accepting, stop propagation here",
          module.stable_id,
        );
        continue;
      }

      // Static and dynamic `import()` importers are walked the same way — parity with
      // Vite (`node.importers`) and webpack (`module.parents`), neither of which
      // distinguishes the edge kind. Duplicate pushes are fine: the `affected.insert`
      // check at pop time dedups them.
      for importer_idx in
        module.importers_idx.iter().chain(module.dynamic_importers_idx.iter()).copied()
      {
        let Module::Normal(importer) = &self.module_table().modules[importer_idx] else {
          continue;
        };
        if importer.can_accept_hmr_dependency_for(&module.id) {
          // Edge boundary: the accepting importer is not re-run, so it joins no set.
          continue;
        }
        stack.push(importer_idx);
      }
    }
    // Deterministic order keeps snapshots stable: one sort of the final set replaces a
    // per-node importer sort (an alloc plus O(deg log deg) comparisons per visit).
    affected.sort_unstable_by(|a, b| {
      self.module_table().modules[*a].stable_id().cmp(self.module_table().modules[*b].stable_id())
    });
    affected
  }

  /// Compile a lazy entry module and return compiled code plus the pending-payload
  /// entry (`carried`) the delivery-time ship-map write consumes.
  ///
  /// A lazy chunk is pure first-evaluation demand: nothing already evaluated ever
  /// re-runs, so factory selection subtracts BOTH per-client records — the ship map
  /// (`shipped[C]`, factory resident) and the top-level-evaluated map (exports live from
  /// entry-chunk execution; `initModule` returns them without a factory). Both are
  /// server-derived; selection never reads client-reported runtime state. Contrast
  /// with HMR patches, whose affected set must re-run and therefore subtracts the
  /// ship map only. The ship map itself is written only when the serving middleware
  /// observes the response complete.
  pub async fn compile_lazy_entry(
    &mut self,
    module_id: &str,
    _client_id: &str,
    shipped: &FxHashMap<ArcStr, u32>,
    evaluated: &FxHashMap<ArcStr, u32>,
    stamp_table: &HmrStampTable,
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

    // Collect all sync dependencies, stopping at modules whose current copy this client
    // already holds — factory resident per the ship map, or exports live per the
    // top-level-evaluated map. Overlapping concurrent lazy compiles both see an
    // unmarked ship map and re-ship shared factories — duplicate idempotent bytes, never
    // a missing factory.
    let mut modules_to_be_updated = FxIndexSet::default();
    self.collect_sync_dependencies_for_client(
      entry_module_idx,
      &mut modules_to_be_updated,
      shipped,
      evaluated,
      stamp_table,
    );

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
        let affected_module_idx = render_input.idx;
        let (code, map) = self.render_module_code(render_input, index, true);

        let affected_module = &self.module_table().modules[affected_module_idx];
        let Module::Normal(affected_module) = affected_module else {
          unreachable!("Only normal modules should be rendered");
        };

        let intro_comment: Box<dyn Source + Send> =
          Box::new(concat_string!("//#region ", affected_module.debug_id));
        let outro_comment: Box<dyn Source + Send> = Box::new(concat_string!("//#endregion"));

        let code_source: Box<dyn Source + Send> = if let Some(map) = map {
          Box::new(SourceMapSource::new(code, map))
        } else {
          Box::new(code)
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

    let carried = modules_to_be_updated
      .iter()
      .map(|module_idx| {
        let stable_id = self.module_table().modules[*module_idx].stable_id();
        (stable_id.as_arc_str().clone(), stamp_table.render_time_stamp(stable_id.as_str()))
      })
      .collect();

    Ok(HmrLazyChunkOutput { code, filename, carried })
  }

  async fn render_hmr_patch(
    &self,
    mut carried_modules: FxIndexSet<ModuleIdx>,
    changed_ids: Vec<String>,
    stamp_table: &HmrStampTable,
  ) -> BuildResult<HmrUpdate> {
    // Note: the carried set might include external modules. There's no way to "update" them, so we need to remove them.
    carried_modules.retain(|idx| self.module_table().modules[*idx].is_normal());

    // Nothing to ship and nothing for the client's walk to re-run — every changed
    // module rendered byte-identical (dropped upstream) and the stale sweep
    // carried nothing. Say so explicitly instead of sending an empty patch.
    if carried_modules.is_empty() && changed_ids.is_empty() {
      return Ok(HmrUpdate::Noop);
    }

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
        let affected_module_idx = render_input.idx;
        let (code, map) = self.render_module_code(render_input, index, true);

        let affected_module = &self.module_table().modules[affected_module_idx];
        let Module::Normal(affected_module) = affected_module else {
          unreachable!("HMR only supports normal module");
        };

        let intro_comment: Box<dyn Source + Send> =
          Box::new(concat_string!("//#region ", affected_module.debug_id));
        let outro_comment: Box<dyn Source + Send> = Box::new(concat_string!("//#endregion"));

        let code_source: Box<dyn Source + Send> = if let Some(map) = map {
          Box::new(SourceMapSource::new(code, map))
        } else {
          Box::new(code)
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

    let carried = carried_modules
      .iter()
      .map(|module_idx| {
        let stable_id = self.module_table().modules[*module_idx].stable_id();
        (stable_id.as_arc_str().clone(), stamp_table.render_time_stamp(stable_id.as_str()))
      })
      .collect();

    Ok(HmrUpdate::Patch(HmrPatch {
      code,
      filename,
      sourcemap_filename: sourcemap_asset.as_ref().map(|asset| asset.filename.to_string()),
      sourcemap: sourcemap_asset.map(|asset| asset.source.try_into_string()).transpose()?,
      changed_ids,
      // The envelope seq is a delivery-layer concern; the dev engine stamps it onto the
      // patches it actually sends (see `bundling_task`), so this is only a placeholder.
      seq: 0,
      carried,
    }))
  }

  /// Finalize and print one module into its HMR payload form (factory-registration
  /// snippet, without the `//#region` framing).
  ///
  /// `unique_index` seeds the payload-position-dependent binding suffixes, so two
  /// renders of the same module compare equal only when they pin it to the same
  /// value. `with_sourcemap: false` skips sourcemap generation even when the
  /// options ask for one.
  fn render_module_code(
    &self,
    render_input: ModuleRenderInput,
    unique_index: usize,
    with_sourcemap: bool,
  ) -> (String, Option<SourceMap>) {
    let ModuleRenderInput { idx: module_idx, ecma_ast: mut ast } = render_input;

    let Module::Normal(module) = &self.module_table().modules[module_idx] else {
      unreachable!("HMR only supports normal module");
    };

    let enable_sourcemap =
      with_sourcemap && self.options.sourcemap.is_some() && !module.is_virtual();
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
        ast_builder: AstBuilder::new(fields.allocator),
        import_bindings: FxHashMap::default(),
        module,
        exports: oxc::allocator::Vec::new_in(&fields.allocator),
        use_pife_for_module_wrappers,
        dependencies: FxIndexSet::default(),
        imports: FxHashSet::default(),
        generated_static_import_infos: FxHashMap::default(),
        re_export_all_dependencies: FxIndexSet::default(),
        generated_static_import_stmts_from_external: FxIndexMap::default(),
        unique_index,
        named_exports: FxHashMap::default(),
      };

      traverse_mut(&mut finalizer, fields.allocator, fields.program, scoping, ());
    });

    let codegen = EcmaCompiler::print_with(
      &ast,
      PrintOptions {
        sourcemap: enable_sourcemap,
        filename: module.id.to_string(),
        comments: PrintCommentsOptions {
          legal: false, // ignore hmr chunk comments
          annotation: self.options.comments.annotation,
          jsdoc: self.options.comments.jsdoc,
        },
        initial_indent: 0,
      },
    );

    match codegen.map {
      Some(map) => (codegen.code, Some(map.into_owned())),
      None => (codegen.code, None),
    }
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
    shipped: &FxHashMap<ArcStr, u32>,
    evaluated: &FxHashMap<ArcStr, u32>,
    stamp_table: &HmrStampTable,
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
          // A module with N importers hits this edge check N times; the cheap
          // visited test spares the ship-map string hashing for all but the first.
          if result.contains(&dep_idx) {
            continue;
          }
          if let Module::Normal(normal_dep) = &modules[dep_idx] {
            // Skip deps whose current copy this client already holds: factory
            // resident per the ship map, or exports live per the top-level-evaluated
            // map (a lazy import never re-runs an evaluated module, so
            // `initModule` serves it without a factory).
            let stable_id = normal_dep.stable_id.as_str();
            let holds_current = |map: &FxHashMap<ArcStr, u32>| {
              map.get(stable_id).is_some_and(|stamp| !stamp_table.is_stale(stable_id, *stamp))
            };
            if holds_current(shipped) || holds_current(evaluated) {
              continue;
            }
          }
          stack.push(dep_idx);
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::is_hidden_from_hot_update_hook;

  #[test]
  fn hidden_ids_cover_runtime_and_lazy_proxy_modules() {
    assert!(is_hidden_from_hot_update_hook(rolldown_common::RUNTIME_MODULE_KEY));
    assert!(is_hidden_from_hot_update_hook("rolldown:hmr"));
    assert!(is_hidden_from_hot_update_hook("/app/main.js?rolldown-lazy=1"));
    assert!(!is_hidden_from_hot_update_hook("/app/main.js"));
    assert!(!is_hidden_from_hot_update_hook("\0virtual:team"));
  }
}
