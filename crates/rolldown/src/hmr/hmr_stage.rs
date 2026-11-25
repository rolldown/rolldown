use std::{
  ops::{Deref, DerefMut},
  path::Path,
  sync::Arc,
  time::Instant,
};

use arcstr::ArcStr;
use itertools::{Either, Itertools};
use oxc::ast::builder::AstBuilder;
use oxc_traverse::traverse_mut;
use rolldown_common::{
  ClientHave, ExportsKind, HmrCatchUp, HmrPatch, HmrPullResponse, HmrRingEntry, HmrSession,
  HmrUpdate, Module, ModuleIdx, ModuleTable, NormalModule, ResolvedId, ScanMode, SharedHmrSession,
  SourceMapType, WatcherChangeKind, is_lazy_proxy, lazy_proxy_target, write_lazy_proxy_id,
};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler, PrintCommentsOptions, PrintOptions};
use rolldown_error::BuildResult;
use rolldown_fs::FileSystem;
use rolldown_plugin::SharedPluginDriver;
use rolldown_plugin_lazy_compilation::LazyCompilationContext;
use rolldown_sourcemap::{SourceJoiner, SourceMap};
use rolldown_utils::{
  concat_string,
  indexmap::{FxIndexMap, FxIndexSet},
  rayon::{IntoParallelIterator, ParallelIterator},
};
use rustc_hash::{FxHashMap, FxHashSet};
use sugar_path::SugarPath;

use crate::{
  SharedOptions, SharedResolver,
  hmr::{
    hmr_ast_finalizer::HmrAstFinalizer,
    module_graph_delta::{RowsSession, assign_payload_nums, render_register_graph_source},
    render_cache::{
      CachedFactorySource, HmrRenderCache, PayloadKey, RenderedFactory, render_inputs,
    },
  },
  module_loader::{ModuleLoader, module_loader::ModuleLoaderOutput},
  type_alias::IndexEcmaAst,
  types::scan_stage_cache::ScanStageCache,
  utils::{
    process_code_and_sourcemap::prepare_sourcemap, render_ecma_module::collapse_module_sourcemap,
  },
};

/// The file name the sourcemap of a pulled payload is prepared for. Payloads are served next to
/// the output files.
const HMR_PATCH_FILENAME: &str = "__rolldown_hmr_patch.js";
/// Up to this many changed modules, the unchanged-output check renders them with their
/// sourcemaps and caches the factories, so the first pull does not render them again. For a
/// larger change set (a branch switch), it would build many maps before the push, and no pull needs
/// most of them.
const MAX_CACHED_RECHECK_RENDERS: usize = 32;

pub struct HmrStageInput<'a, Fs: FileSystem + Clone + 'static> {
  pub options: SharedOptions,
  pub fs: Fs,
  pub resolver: SharedResolver<Fs>,
  pub plugin_driver: SharedPluginDriver,
  pub cache: &'a mut ScanStageCache,
  pub lost_hmr_update: &'a mut bool,
  pub session: SharedHmrSession,
  pub render_cache: &'a mut HmrRenderCache,
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
/// artifacts and runtime modules are never re-fetched.
fn is_hidden_from_hot_update_hook(id: &str) -> bool {
  is_lazy_proxy(id) || id.starts_with("\0rolldown/")
}

impl<'a, Fs: FileSystem + Clone + 'static> HmrStage<'a, Fs> {
  pub fn new(input: HmrStageInput<'a, Fs>) -> Self {
    Self { input }
  }

  /// See `internal-docs/hmr/design.md` for the principles and invariants this
  /// stage implements. Stage order is documented in
  /// `internal-docs/dev-engine/implementation.md`
  /// ("Inside `compute_hmr_update_for_file_changes`").
  #[expect(clippy::too_many_lines)]
  pub async fn compute_hmr_update_for_file_changes(
    &mut self,
    changed_file_paths: &FxIndexMap<String, WatcherChangeKind>,
    last_build_errored: bool,
    hot_update_hook_enabled: bool,
  ) -> BuildResult<HmrUpdate> {
    tracing::trace!(
      changed_files = %changed_file_paths
        .iter()
        .map(|(path, kind)| format!("{path}:{kind}"))
        .join(", "),
      "[HmrStage] starts computing HMR updates"
    );
    // An earlier lost update stays lost: this one does not carry its edit.
    let lost_before = *self.lost_hmr_update;

    // 1. Identify changed modules — per changed file: compute the default affected set, then (if
    // the hook is enabled and any plugin registered `hotUpdate`) let the plugin replace-chain
    // edit it before re-fetching. The hook is off by default. Either `DevOptions::hot_update`
    // (`hot_update_hook_enabled`) or `DevModeOptions::hot_update` turns it on.
    let hot_update_hook_enabled = hot_update_hook_enabled
      || self.options.experimental.dev_mode.as_ref().is_some_and(|d| d.hot_update == Some(true));
    let hot_update_hook_registered =
      hot_update_hook_enabled && self.plugin_driver.has_hot_update_hook();
    let mut changed_modules = FxIndexSet::default();
    // Modules a `hotUpdate` hook explicitly selected (a plugin returned a replacement set).
    // They are exempt from the unchanged-output suppression below — like `last_build_errored`,
    // by skipping the pre-rebuild capture. An explicit return is a directive to re-run the
    // module in clients, and what changed can live outside the module's own code (e.g. a
    // watched non-module file the module reads at runtime), so identical output does not make
    // the update empty. Vite ships hook-returned modules unconditionally.
    let mut hook_selected_modules = FxHashSet::default();
    for (changed_file_path, event) in changed_file_paths {
      let changed_path = Path::new(changed_file_path);
      let changed_file_path = ArcStr::from(changed_file_path.to_slash());

      // Default affected set: the file's own module (kept even for deletes — the hook contract
      // passes the deleted module itself; importer expansion happens after the chain) plus every
      // module that has this file as a transform dependency.
      let own_module_idx = self.cache.module_idx_by_abs_path.get(&changed_file_path).copied();
      let mut affected_modules = FxIndexSet::default();
      if let Some(module_idx) = own_module_idx {
        affected_modules.insert(module_idx);
      }
      // `transform_dependencies` is a DashMap, so its iteration order can differ between runs.
      // The `hotUpdate` hook makes this order plugin-visible via `args.modules`, so give the
      // set a stable order: own module first, then registrants sorted by stable id.
      let mut transform_dep_modules = self
        .plugin_driver
        .transform_dependencies
        .iter()
        .filter_map(|entry| {
          changed_path
            .ancestors()
            .any(|ancestor| entry.value().contains(ancestor))
            .then_some(*entry.key())
        })
        .collect::<Vec<_>>();
      transform_dep_modules
        .sort_unstable_by_key(|module_idx| self.module_table().modules[*module_idx].stable_id());
      affected_modules.extend(transform_dep_modules);

      let mut hook_replaced = false;
      if hot_update_hook_registered {
        // Plugins receive plain module ids and may replace the set; an empty return suppresses
        // this file's update. The chain also runs when the default set is empty — content
        // plugins claim files no module maps to. Ids the graph doesn't know are dropped.
        // The ids are `to_slash`ed like `args.file`, so one args object never mixes path
        // conventions on Windows.
        let default_ids = affected_modules
          .iter()
          .filter_map(|module_idx| match &self.module_table().modules[*module_idx] {
            Module::Normal(module) if !is_hidden_from_hot_update_hook(module.id.as_arc_str()) => {
              Some(ArcStr::from(module.id.as_arc_str().to_slash()))
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
            // The map keys are `to_slash`ed module ids, but a returned id may be plugin-built
            // with native separators (e.g. via `path.join` on Windows) — normalize it the same
            // way so it still round-trips back to its module.
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
      changed_modules = %changed_modules
        .iter()
        .map(|module_idx| self.module_table().modules[*module_idx].stable_id())
        .join(", "),
      "[HmrStage] mapped changed file paths to modules"
    );

    // Files re-queued by an earlier failed scan (`pending_rescans`) get
    // re-fetched and merged by this update as well. Treat them as changed so
    // their recovered content reaches the clients' updates; otherwise only
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
      return Ok(HmrUpdate::Noop);
    }

    // After an errored build (`last_build_errored`) the capture is skipped, so
    // every changed module is pushed. A failed scan merges nothing, so undoing the
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
          (module_idx, self.render_module_code(render_input, false).0)
        })
        .collect::<Vec<_>>()
        .into_iter()
        .collect::<FxHashMap<_, _>>()
    };

    let modules = &self.module_table().modules;
    let pre_rebuild_interop = changed_modules
      .iter()
      .filter_map(|module_idx| {
        Some((*module_idx, importee_interop(modules[*module_idx].as_normal()?)))
      })
      .collect::<FxHashMap<_, _>>();

    // 1. Do ONE module refetch and cache merge. Boundary decisions belong to the client's own
    // walk, and the client pulls the code it needs afterwards (`render_patch`).
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

      // `changed_modules` holds the queued rescans too.
      let input = &mut self.input;
      input.render_cache.forget(input.cache, changed_modules.iter().copied());

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
        new_modules = %new_added_modules
          .iter()
          .map(|module_idx| module_loader_output.module_table.get(*module_idx).stable_id())
          .join(", "),
        "added modules"
      );

      // From here on, a failure leaves the graph with an edit that no client received.
      *self.lost_hmr_update = true;
      let plugin_driver = Arc::clone(&self.plugin_driver);
      self.cache.merge(module_loader_output.into(), &plugin_driver)?;

      let options = Arc::clone(&self.options);
      self
        .cache
        .apply_module_side_effects_overrides(&options, &plugin_driver.module_side_effects_overrides)
        .await?;
      new_added_modules
    };

    // Drop the changed modules whose post-rebuild render is byte-identical to the
    // pre-rebuild capture: their output didn't change, so stamping and pushing
    // them would only make every client re-run code it already holds. A module
    // that can't be compared (no pre-rebuild render, or no longer normal) stays
    // in. Dep identity is part of the compared code (internal imports print as
    // `loadExports("<dep stable id>")`), so a resolution shift under an unchanged
    // source is still pushed.
    let recheck_inputs = changed_modules
      .iter()
      .filter(|module_idx| pre_rebuild_renders.contains_key(*module_idx))
      .filter_map(|module_idx| {
        self.module_table().modules[*module_idx].as_normal()?;
        let ecma_ast = self.index_ecma_ast()[*module_idx].as_ref()?;
        Some(ModuleRenderInput { idx: *module_idx, ecma_ast: ecma_ast.clone_with_another_arena() })
      })
      .collect::<Vec<_>>();
    let cache_renders = recheck_inputs.len() <= MAX_CACHED_RECHECK_RENDERS;
    let (output_unchanged_modules, rerendered_factories): (FxHashSet<_>, Vec<_>) = recheck_inputs
      .into_par_iter()
      .map(|render_input| {
        let module_idx = render_input.idx;
        let (code, map) = self.render_module_code(render_input, cache_renders);
        if pre_rebuild_renders[&module_idx] == code {
          Either::Left(module_idx)
        } else {
          Either::Right(cache_renders.then_some((module_idx, RenderedFactory { code, map })))
        }
      })
      .collect::<Vec<_>>()
      .into_iter()
      .partition_map(|rendered| rendered);
    if !output_unchanged_modules.is_empty() {
      tracing::debug!(
        target: "hmr",
        unchanged_modules = %output_unchanged_modules
          .iter()
          .map(|module_idx| self.module_table().modules[*module_idx].stable_id())
          .join(", "),
        "skip modules whose rebuilt output is unchanged"
      );
      changed_modules.retain(|module_idx| !output_unchanged_modules.contains(module_idx));
    }

    // An importer's factory reads its importees' interop facts, so it changes with them even
    // when its own source did not. Stamp such importers like changed modules: a
    // stamp must name one rendered factory (see internal-docs/hmr/design.md, "Registration
    // keeps the newest copy").
    let modules = &self.module_table().modules;
    let interop_changed = changed_modules
      .iter()
      .copied()
      .filter(|module_idx| {
        let before = pre_rebuild_interop.get(module_idx);
        let after = modules[*module_idx].as_normal().map(importee_interop);
        before.is_some_and(|before| after.is_some_and(|after| *before != after))
      })
      .collect::<FxHashSet<_>>();
    // A lazy proxy is left out: it never runs in the client, because `requestLazy` runs the real
    // module. The modules that `import()` it emit `requestLazy`, which reads only ids, so they
    // are not re-rendered either.
    let rerendered_importers = if interop_changed.is_empty() {
      FxIndexSet::default()
    } else {
      modules
        .iter()
        .filter_map(Module::as_normal)
        .filter(|module| !changed_modules.contains(&module.idx))
        .filter(|module| lazy_proxy_target(module.stable_id.as_str()).is_none())
        .filter(|module| {
          module
            .import_records
            .iter()
            .any(|rec| rec.resolved_module.is_some_and(|dep| interop_changed.contains(&dep)))
        })
        .map(|module| module.idx)
        .collect::<FxIndexSet<_>>()
    };

    // 2. Stamp the rebuild: `latest[m] = generation` for every changed, newly added or
    // re-rendered module. These are the stale modules: a client drops its factory of each.
    let modules = &self.module_table().modules;
    let stamped_ids = changed_modules
      .iter()
      .chain(new_added_modules.iter())
      .chain(rerendered_importers.iter())
      .filter_map(|module_idx| modules[*module_idx].as_normal())
      .map(|module| module.stable_id.as_arc_str().clone())
      .collect::<FxIndexSet<_>>();
    // A lazy proxy never runs in the client, so no client holds a factory of it or walks from
    // it.
    let stale_ids = stamped_ids
      .iter()
      .filter(|id| lazy_proxy_target(id.as_str()).is_none())
      .cloned()
      .collect::<Vec<_>>();
    let changed_ids = changed_modules
      .iter()
      .filter_map(|module_idx| modules[*module_idx].as_normal())
      .map(|module| module.stable_id.as_arc_str().clone())
      .filter(|id| lazy_proxy_target(id.as_str()).is_none())
      .collect::<Vec<_>>();

    *self.lost_hmr_update = lost_before;
    // Every changed module rendered byte-identical (dropped above), so no client needs to do
    // anything.
    if stale_ids.is_empty() {
      return Ok(HmrUpdate::Noop);
    }

    let mut session = self.session.lock();
    let generation = session.begin_rebuild();
    for id in &stamped_ids {
      session.stamp(id, generation);
    }
    session.record_update(HmrRingEntry {
      generation,
      changed_ids: changed_ids.clone(),
      stale_ids: stale_ids.clone(),
    });
    drop(session);

    for (module_idx, factory) in rerendered_factories.into_iter().flatten() {
      let Module::Normal(module) = &self.module_table().modules[module_idx] else { continue };
      let inputs = render_inputs(&self.module_table().modules, module);
      let stable_id = module.stable_id.as_arc_str().clone();
      self.input.render_cache.insert(stable_id, generation, inputs, Arc::new(factory));
    }

    Ok(HmrUpdate::Changed {
      changed_ids: changed_ids.iter().map(ToString::to_string).collect(),
      stale_ids: stale_ids.iter().map(ToString::to_string).collect(),
      generation,
    })
  }

  /// Answers one pull: the factories a client needs to run its update `U` (`have.update`) and
  /// its lazy `import()` targets (`have.lazy`). The client describes what it holds in the
  /// request, so the server keeps no state per client.
  ///
  /// Carries each root the client does not hold a current factory of, and the static deps that
  /// `collect_unheld_sync_deps` finds. When the client is behind the session, the payload tells
  /// it about the updates it missed (`render_catch_up`).
  ///
  /// A lazy target that is not in the graph yet, or whose own proxy waits in `pending_rescans`,
  /// is scanned first (`scan_lazy_routes`).
  /// `merged` becomes `true` once that scan merges into the graph. It stays `true` when a later
  /// step fails, because the graph keeps the route.
  /// See internal-docs/lazy-compilation/design.md ("Repeat Visits").
  pub async fn render_patch(
    &mut self,
    have: &ClientHave,
    lazy_ctx: Option<&LazyCompilationContext>,
    merged: &mut bool,
  ) -> BuildResult<HmrPullResponse<HmrPatch>> {
    if !self.cache.has_snapshot() {
      return Ok(HmrPullResponse::Reload {
        reason: "the last full build failed, so there is no module graph".to_owned(),
      });
    }

    // A client that must reload changes nothing on the server, so this check comes before any
    // scan.
    let catch_up = catch_up_statement(&self.session.lock(), have);
    let catch_up = match catch_up {
      Ok(catch_up) => catch_up,
      Err(reason) => return Ok(HmrPullResponse::Reload { reason }),
    };

    let mut scanned = false;
    if !have.lazy.is_empty() {
      let routes = self.lazy_routes(have)?;
      let to_scan = routes
        .iter()
        .filter_map(|route| {
          let proxy = route.proxy?;
          let queued = || {
            let proxy_id =
              &self.module_table().modules[proxy].as_normal()?.originative_resolved_id.id;
            Some(self.cache.pending_rescans.iter().any(|queued| queued.id == *proxy_id))
          };
          (route.target.is_none() || queued() == Some(true)).then_some(proxy)
        })
        .collect::<Vec<_>>();
      if !to_scan.is_empty() {
        self.scan_lazy_routes(&to_scan, lazy_ctx, merged).await?;
        scanned = true;
      }
    }

    let (carried, catch_up) = {
      let session = self.session.lock();
      let modules = &self.module_table().modules;
      let module_of = |id: &str| {
        self
          .cache
          .module_idx_by_stable_id
          .get(id)
          .copied()
          .filter(|module_idx| modules[*module_idx].is_normal())
      };
      // An id that left the graph is skipped. The client then still cannot run its update, and
      // it reloads.
      let update = have
        .update
        .iter()
        .filter_map(|num| session.id_of(num))
        .filter_map(|id| module_of(id.as_str()))
        .collect::<Vec<_>>();
      let client = ClientView { have, session: &session };
      let mut carried = update
        .iter()
        .copied()
        .filter(|module_idx| !client.holds_current(modules[*module_idx].stable_id().as_str()))
        .collect::<FxIndexSet<_>>();
      let mut walk_from = update;
      // A lazy target the client ran keeps its exports: `requestLazy` only reads them.
      for module_idx in have
        .lazy
        .iter()
        .filter_map(|num| session.id_of(num))
        .filter(|id| !client.keeps_exports(id.as_str()))
        .filter_map(|id| module_of(id.as_str()))
      {
        if !client.holds_current(modules[module_idx].stable_id().as_str()) {
          carried.insert(module_idx);
        }
        walk_from.push(module_idx);
      }
      self.collect_unheld_sync_deps(&walk_from, &client, &mut carried);
      (carried, catch_up)
    };

    let modules = carried.len();
    let phase_start = Instant::now();
    let (code, sourcemap) = self.render_payload(carried, catch_up).await?;
    if !have.lazy.is_empty() {
      tracing::debug!(
        target: "hmr",
        scanned,
        modules,
        render_time = ?phase_start.elapsed(),
        "render_patch: lazy request done"
      );
    }
    Ok(HmrPullResponse::Payload(HmrPatch { code, sourcemap }))
  }

  /// Maps each number of `have.lazy` to its target and its proxy. The numbers come from the
  /// session and the modules from the graph, so a request can never make the server resolve a
  /// path it sends (#9969).
  fn lazy_routes(&self, have: &ClientHave) -> BuildResult<Vec<LazyRoute>> {
    let session = self.session.lock();
    let modules = &self.module_table().modules;
    let module_of = |id: &str| {
      self
        .cache
        .module_idx_by_stable_id
        .get(id)
        .copied()
        .filter(|module_idx| modules[*module_idx].is_normal())
    };
    let mut proxy_id = String::new();
    have
      .lazy
      .iter()
      .map(|num| {
        // `ClientHave::parse_query` rejected every number the session did not give out.
        let id = session.id_of(num).expect("the query names only known numbers");
        write_lazy_proxy_id(&mut proxy_id, id);
        let route = LazyRoute { target: module_of(id), proxy: module_of(&proxy_id) };
        if route.target.is_none() && route.proxy.is_none() {
          return Err(
            vec![anyhow::anyhow!("Lazy entry module not found in cache. module_id={id}").into()]
              .into(),
          );
        }
        Ok(route)
      })
      .collect()
  }

  /// Scans the routes of the lazy proxies `proxies` into the graph. The lazy plugin's `load`
  /// returns the fetched template of a proxy marked as fetched, and that template's `import()`
  /// reaches the real module. Only the proxies' own entries leave `pending_rescans`: a broken
  /// file of another route must not fail this one.
  async fn scan_lazy_routes(
    &mut self,
    proxies: &[ModuleIdx],
    lazy_ctx: Option<&LazyCompilationContext>,
    merged: &mut bool,
  ) -> BuildResult<()> {
    let resolved_ids = proxies
      .iter()
      .filter_map(|proxy| self.module_table().modules[*proxy].as_normal())
      .map(|proxy| proxy.originative_resolved_id.clone())
      .collect::<Vec<_>>();
    let newly_fetched = lazy_ctx.map_or_else(Vec::new, |lazy_ctx| {
      resolved_ids
        .iter()
        .filter(|resolved_id| lazy_ctx.mark_as_fetched(&resolved_id.id))
        .map(|resolved_id| resolved_id.id.clone())
        .collect()
    });

    let phase_start = Instant::now();
    let module_loader_output = match self.fetch_lazy_routes(resolved_ids).await {
      Ok(output) => output,
      Err(errors) => {
        if let Some(lazy_ctx) = lazy_ctx {
          for id in &newly_fetched {
            lazy_ctx.unmark_fetched(id);
          }
        }
        return Err(errors);
      }
    };

    *merged = true;
    let plugin_driver = Arc::clone(&self.plugin_driver);
    self
      .cache
      .merge(module_loader_output.into(), &plugin_driver)
      .map_err(|e| vec![anyhow::anyhow!(e).into()])?;

    let options = Arc::clone(&self.options);
    self
      .cache
      .apply_module_side_effects_overrides(&options, &plugin_driver.module_side_effects_overrides)
      .await?;
    tracing::debug!(
      target: "hmr",
      routes = proxies.len(),
      scan_time = ?phase_start.elapsed(),
      "render_patch: scanned lazy routes"
    );
    Ok(())
  }

  async fn fetch_lazy_routes(
    &mut self,
    resolved_ids: Vec<ResolvedId>,
  ) -> BuildResult<ModuleLoaderOutput> {
    let fetch_mode = ScanMode::Partial(resolved_ids);
    let mut module_loader = ModuleLoader::new(
      self.fs.clone(),
      Arc::clone(&self.options),
      Arc::clone(&self.resolver),
      Arc::clone(&self.plugin_driver),
      self.cache,
      fetch_mode.is_full(),
      None,
    )?;
    module_loader.set_retry_pending_rescans(false);
    module_loader.fetch_modules(fetch_mode).await
  }

  /// Renders a pulled payload:
  ///
  /// 1. the prelude `const __rolldown_payload__ = __rolldown_runtime__.beginPayload(G)`, where `G`
  ///    is the session's generation. Every factory and row of the payload is as of `G`.
  /// 2. the catch-up statement, when the client is behind
  /// 3. the graph rows
  /// 4. one factory registration per carried module
  ///
  /// No driver tail: the client walks its own graph, removes from its cache, and re-runs from
  /// the factory map. Importing the payload commits rows and factories, nothing more.
  ///
  /// The caller holds the bundler, so no stamp or merge runs while this renders, and `G` names
  /// the state of every module it renders. See internal-docs/hmr/design.md, principle 7 ("On the
  /// wire every number is a generation").
  async fn render_payload(
    &mut self,
    mut carried_modules: FxIndexSet<ModuleIdx>,
    catch_up: Option<String>,
  ) -> BuildResult<(String, Option<String>)> {
    // External modules have no factory to send.
    carried_modules.retain(|idx| self.module_table().modules[*idx].is_normal());
    // Sorting makes the snapshot more stable when we change logic that affects the order of
    // modules.
    carried_modules
      .sort_by_cached_key(|module_idx| self.module_table().modules[*module_idx].id().as_str());

    let (generation, rows, plans) = {
      let mut session = self.session.lock();
      assign_payload_nums(&mut session, self.module_table(), carried_modules.iter().copied());
      let rows = render_register_graph_source(
        self.module_table(),
        carried_modules.iter().copied(),
        &self.cache.get_snapshot().user_defined_entry_modules,
        RowsSession::Payload(&session),
      );
      let modules = &self.module_table().modules;
      let plans = carried_modules
        .iter()
        .map(|module_idx| {
          let module = modules[*module_idx].as_normal().expect("only normal modules are carried");
          FactoryPlan {
            idx: *module_idx,
            stamp: session.render_time_stamp(module.stable_id.as_str()),
            inputs: render_inputs(modules, module),
          }
        })
        .collect::<Vec<_>>();
      (session.generation(), rows, plans)
    };

    // Many clients ask for the same modules after one edit, so a factory is rendered once and
    // kept, and so is a finished payload. See internal-docs/hmr/design.md, "Render cache".
    let modules = &self.input.cache.get_snapshot().module_table.modules;
    let payload_key = PayloadKey {
      generation,
      catch_up: catch_up.clone(),
      rows: rows.clone(),
      factories: plans
        .iter()
        .map(|plan| (modules[plan.idx].stable_id().as_arc_str().clone(), plan.stamp, plan.inputs))
        .collect(),
    };
    let cached_payload = self.input.render_cache.get_payload(&payload_key);
    // Debug builds render the payload again and compare it below.
    #[cfg(not(debug_assertions))]
    if let Some(payload) = cached_payload {
      return Ok((payload.code.clone(), payload.sourcemap.clone()));
    }
    let cached = plans
      .iter()
      .map(|plan| {
        let stable_id = modules[plan.idx].stable_id().as_str();
        self.input.render_cache.get(stable_id, plan.stamp, plan.inputs)
      })
      .collect::<Vec<_>>();

    let index_ecma_ast = self.index_ecma_ast();
    let render_input = |module_idx: ModuleIdx| {
      let ecma_ast = index_ecma_ast[module_idx].as_ref().expect("Normal module should have an AST");
      ModuleRenderInput { idx: module_idx, ecma_ast: ecma_ast.clone_with_another_arena() }
    };
    let fresh = plans
      .iter()
      .zip(&cached)
      .filter(|(_, cached)| cached.is_none())
      .map(|(plan, _)| render_input(plan.idx))
      .collect::<Vec<_>>()
      .into_par_iter()
      .map(|input| {
        let (code, map) = self.render_module_code(input, true);
        Arc::new(RenderedFactory { code, map })
      })
      .collect::<Vec<_>>();

    // The test suites run in debug builds, so they check that the cache keys are complete.
    #[cfg(debug_assertions)]
    for (plan, cached) in plans.iter().zip(&cached) {
      let Some(cached) = cached else { continue };
      let (code, map) = self.render_module_code(render_input(plan.idx), true);
      debug_assert_eq!(cached.code, code, "a cached HMR factory differs from a fresh render");
      debug_assert_eq!(
        cached.map.as_ref().map(SourceMap::to_json_string),
        map.as_ref().map(SourceMap::to_json_string),
        "the sourcemap of a cached HMR factory differs from a fresh render"
      );
    }

    let mut fresh = fresh.into_iter();
    let factories = plans
      .iter()
      .zip(cached)
      .map(|(plan, cached)| {
        cached.unwrap_or_else(|| {
          let factory = fresh.next().expect("one fresh render per cache miss");
          let stable_id = modules[plan.idx].stable_id().as_arc_str().clone();
          self.input.render_cache.insert(stable_id, plan.stamp, plan.inputs, Arc::clone(&factory));
          factory
        })
      })
      .collect::<Vec<_>>();

    let mut source_joiner = SourceJoiner::default();
    source_joiner.append_source(concat_string!(
      "const __rolldown_payload__ = __rolldown_runtime__.beginPayload(",
      itoa::Buffer::new().format(generation),
      ");"
    ));
    if let Some(catch_up) = catch_up {
      source_joiner.append_source(catch_up);
    }
    if let Some(rows) = rows {
      source_joiner.append_source(rows);
    }
    for (plan, factory) in plans.iter().zip(factories) {
      let Module::Normal(module) = &modules[plan.idx] else {
        unreachable!("HMR only supports normal module");
      };
      source_joiner.append_source(concat_string!("//#region ", module.debug_id));
      source_joiner.append_source_dyn(Box::new(CachedFactorySource(factory)));
      source_joiner.append_source(concat_string!("//#endregion"));
    }

    let (mut code, map) = source_joiner.join();
    let sourcemap = self.finish_payload_sourcemap(&mut code, map).await?;
    #[cfg(debug_assertions)]
    if let Some(cached) = cached_payload {
      debug_assert_eq!(cached.code, code, "a cached HMR payload differs from a fresh render");
      debug_assert_eq!(
        cached.sourcemap, sourcemap,
        "the sourcemap of a cached HMR payload differs from a fresh render"
      );
      return Ok((code, sourcemap));
    }
    self.input.render_cache.insert_payload(payload_key, &code, sourcemap.as_deref());
    Ok((code, sourcemap))
  }

  /// `Inline` appends the map to the code as a data URL. `File` and `Hidden` return the map as
  /// JSON, and the code has no `sourceMappingURL` comment: no file is kept for a pulled payload,
  /// so the consumer serves the map itself and adds the comment that points to it.
  async fn finish_payload_sourcemap(
    &self,
    code: &mut String,
    map: Option<SourceMap>,
  ) -> BuildResult<Option<String>> {
    let Some(mut map) = map else {
      return Ok(None);
    };
    let file_dir = self.options.cwd.as_path().join(&self.options.out_dir);
    prepare_sourcemap(&self.options, &mut map, &file_dir, HMR_PATCH_FILENAME).await?;
    match self.options.sourcemap {
      Some(SourceMapType::File | SourceMapType::Hidden) => Ok(Some(map.to_json_string())),
      Some(SourceMapType::Inline) => {
        code.push_str("\n//# sourceMappingURL=");
        code.push_str(&map.to_data_url());
        Ok(None)
      }
      None => Ok(None),
    }
  }

  /// Finalize and print one module into its HMR payload form (factory-registration
  /// snippet, without the `//#region` framing).
  ///
  /// The code depends only on the module and its importees, not on the payload it goes into.
  /// `with_sourcemap: false` skips sourcemap generation even when the options ask for one; the
  /// code is the same either way.
  fn render_module_code(
    &self,
    render_input: ModuleRenderInput,
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
        generated_load_exports_stmts: FxHashMap::default(),
        generated_static_import_infos: FxHashMap::default(),
        re_export_all_dependencies: FxIndexSet::default(),
        generated_static_import_stmts_from_external: FxIndexMap::default(),
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
        ascii_only: self.options.charset.is_ascii(),
      },
    );

    // The codegen map is the last element of the module's sourcemap chain, so on its
    // own it maps back to what the plugins produced rather than to the original file.
    // Collapsing the chain is what lets a position in a pulled payload resolve
    // to the source the user wrote, the same way `render_ecma_module` does for a
    // module rendered into a chunk.
    //
    // Warnings are dropped: a module rendered here was rendered by the full build too,
    // which reports a broken chain for it already.
    // Hand the codegen map to the collapse as is. `into_owned` here would
    // copy sourcesContent/names that the collapse immediately copies again
    // (or drops). The returned map is owned.
    let code = codegen.code;
    // Guarded, because a chain collapses to a map even when the codegen step
    // contributed none — which would hand back a sourcemap for a render that asked
    // not to have one.
    let map = enable_sourcemap
      .then(|| {
        collapse_module_sourcemap(
          &module.sourcemap_chain,
          codegen.map,
          module.id.as_str(),
          &mut Vec::new(),
        )
      })
      .flatten();

    (code, map)
  }
}

struct ModuleRenderInput {
  pub idx: ModuleIdx,
  pub ecma_ast: EcmaAst,
}

/// How `render_payload` renders one carried module, and its render cache key.
struct FactoryPlan {
  idx: ModuleIdx,
  stamp: u32,
  inputs: u64,
}

/// The modules behind one number of `ClientHave::lazy`. At least one of them is in the graph.
struct LazyRoute {
  /// The module the client runs. `None` before the first visit.
  target: Option<ModuleIdx>,
  /// The lazy proxy that stands for `target`. `None` when nothing calls `import()` on it.
  proxy: Option<ModuleIdx>,
}

/// A client's have-set read against the session.
struct ClientView<'a> {
  have: &'a ClientHave,
  session: &'a HmrSession,
}

impl ClientView<'_> {
  /// An executed module that the client's update does not re-run keeps its exports, so the
  /// modules below it need nothing.
  fn keeps_exports(&self, stable_id: &str) -> bool {
    self
      .session
      .num_of(stable_id)
      .is_some_and(|num| self.have.executed.contains(num) && !self.have.update.contains(num))
  }

  fn holds_current(&self, stable_id: &str) -> bool {
    self.session.holds_current(self.have, stable_id)
  }
}

/// What an importer's render reads from a normal importee (`HmrAstFinalizer`), apart from
/// `def_format`, which comes from the path and `package.json`, not from the module's source.
fn importee_interop(module: &NormalModule) -> (ExportsKind, bool) {
  (module.exports_kind, module.meta.has_lazy_export())
}

impl<Fs: FileSystem + Clone + 'static> HmrStage<'_, Fs> {
  /// Adds to `out` the static deps of `walk_from` that the client cannot run without a new
  /// factory. A dep whose exports the client keeps stops the walk. A dep the client holds a
  /// current factory of is walked through but not carried. `walk_from` modules are never
  /// carried. See internal-docs/hmr/design.md, principle 7.
  fn collect_unheld_sync_deps(
    &self,
    walk_from: &[ModuleIdx],
    client: &ClientView<'_>,
    out: &mut FxIndexSet<ModuleIdx>,
  ) {
    let modules = &self.module_table().modules;
    let mut visited = walk_from.iter().copied().collect::<FxHashSet<_>>();
    let mut to_walk = walk_from.to_vec();
    let mut to_sort = Vec::new();

    loop {
      while let Some(module_idx) = to_sort.pop() {
        if !visited.insert(module_idx) {
          continue;
        }
        let Module::Normal(module) = &modules[module_idx] else {
          continue;
        };
        let stable_id = module.stable_id.as_str();
        if client.keeps_exports(stable_id) {
          continue;
        }
        if !client.holds_current(stable_id) {
          out.insert(module_idx);
        }
        to_walk.push(module_idx);
      }

      let Some(module_idx) = to_walk.pop() else {
        break;
      };
      let Module::Normal(module) = &modules[module_idx] else {
        continue;
      };
      for rec in &module.import_records {
        if rec.kind.is_static()
          && let Some(dep_idx) = rec.resolved_module
        {
          to_sort.push(dep_idx);
        }
      }
    }
  }
}

/// The catch-up statement for the client `have`: `None` when it is up to date, and the reload
/// reason when the session cannot serve it.
fn catch_up_statement(session: &HmrSession, have: &ClientHave) -> Result<Option<String>, String> {
  match session.catch_up(have) {
    HmrCatchUp::Reload { reason } => Err(reason),
    HmrCatchUp::UpToDate => Ok(None),
    HmrCatchUp::Behind { missed } => Ok(Some(render_catch_up(&missed))),
  }
}

/// The statement that tells a client behind the session about the updates it missed, oldest
/// first. Each entry acts like the push the client did not get:
/// `__rolldown_payload__.catchUp([{gen:4,changedIds:["a.js"],staleIds:["a.js","x.js"]}]);`.
/// `changedIds` is left out when it is empty.
fn render_catch_up(missed: &[HmrRingEntry]) -> String {
  let push_ids = |source: &mut String, ids: &[ArcStr]| {
    source.push('[');
    for (i, id) in ids.iter().enumerate() {
      if i > 0 {
        source.push(',');
      }
      source.push_str(&json_escape_simd::escape(id));
    }
    source.push(']');
  };
  let mut source = String::from("__rolldown_payload__.catchUp([");
  for (i, entry) in missed.iter().enumerate() {
    if i > 0 {
      source.push(',');
    }
    source.push_str("{gen:");
    source.push_str(itoa::Buffer::new().format(entry.generation));
    if !entry.changed_ids.is_empty() {
      source.push_str(",changedIds:");
      push_ids(&mut source, &entry.changed_ids);
    }
    source.push_str(",staleIds:");
    push_ids(&mut source, &entry.stale_ids);
    source.push('}');
  }
  source.push_str("]);");
  source
}

#[cfg(test)]
mod tests {
  use rolldown_common::HmrRingEntry;

  use super::{is_hidden_from_hot_update_hook, render_catch_up};

  #[test]
  fn hidden_ids_cover_runtime_and_lazy_proxy_modules() {
    assert!(is_hidden_from_hot_update_hook(rolldown_common::RUNTIME_MODULE_KEY));
    assert!(is_hidden_from_hot_update_hook("/app/main.js?rolldown-lazy=1"));
    assert!(!is_hidden_from_hot_update_hook("/app/main.js"));
    assert!(!is_hidden_from_hot_update_hook("\0virtual:team"));
  }

  #[test]
  fn catch_up_leaves_out_empty_changed_ids() {
    let missed = [
      HmrRingEntry { generation: 4, changed_ids: vec![], stale_ids: vec!["x.js".into()] },
      HmrRingEntry {
        generation: 5,
        changed_ids: vec!["b.js".into()],
        stale_ids: vec!["b.js".into(), "a.js".into()],
      },
    ];
    assert_eq!(
      render_catch_up(&missed),
      r#"__rolldown_payload__.catchUp([{gen:4,staleIds:["x.js"]},{gen:5,changedIds:["b.js"],staleIds:["b.js","a.js"]}]);"#
    );
  }
}
