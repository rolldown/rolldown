#[cfg(feature = "experimental")]
use super::Bundler;
#[cfg(feature = "experimental")]
use crate::hmr::hmr_stage::{HmrStage, HmrStageInput};
#[cfg(feature = "experimental")]
use arcstr::ArcStr;
#[cfg(feature = "experimental")]
use rolldown_common::WatcherChangeKind;
#[cfg(feature = "experimental")]
use rolldown_common::{
  ClientHmrInput, ClientHmrUpdate, HmrLazyChunkOutput, HmrStampTable, ImportKind, ImportRecordIdx,
  ImportRecordMeta, Module,
};
#[cfg(feature = "experimental")]
use rolldown_error::BuildResult;
#[cfg(feature = "experimental")]
use rolldown_utils::indexmap::FxIndexMap;
#[cfg(feature = "experimental")]
use rustc_hash::{FxHashMap, FxHashSet};
#[cfg(feature = "experimental")]
use std::sync::{Arc, atomic::AtomicU32};

/// What the records of a user-defined entry evaluate, by stable id; see
/// [`Bundler::compute_top_level_evaluated_modules`].
#[cfg(feature = "experimental")]
pub type TopLevelEvaluation = (FxHashMap<ArcStr, u32>, FxHashSet<ArcStr>, FxHashSet<ArcStr>);

/// What the records of each user-defined entry evaluate and hold; see
/// [`Bundler::compute_entry_evaluated_modules`].
#[cfg(feature = "experimental")]
pub type EntryEvaluations = (
  Vec<(ArcStr, TopLevelEvaluation)>,
  FxHashSet<ArcStr>,
  FxHashMap<ArcStr, FxHashSet<ArcStr>>,
  FxHashMap<ArcStr, FxHashMap<ArcStr, u32>>,
);

/// The modules that a walk of [`Bundler::compute_entry_evaluated_modules`] visited: a flag for
/// every module of the graph, or, for the many small walks of
/// [`Bundler::compute_record_evaluations`], a set of those that it reached.
#[cfg(feature = "experimental")]
enum Visited {
  Dense(Vec<bool>),
  Sparse(FxHashSet<rolldown_common::ModuleIdx>),
}

#[cfg(feature = "experimental")]
impl Visited {
  fn new(sparse: bool, modules: usize) -> Self {
    if sparse { Self::Sparse(FxHashSet::default()) } else { Self::Dense(vec![false; modules]) }
  }

  /// Marks `idx` visited, and whether it wasn't before.
  fn insert(&mut self, idx: rolldown_common::ModuleIdx) -> bool {
    match self {
      Self::Dense(visited) => !std::mem::replace(&mut visited[idx.index()], true),
      Self::Sparse(visited) => visited.insert(idx),
    }
  }

  fn contains(&self, idx: rolldown_common::ModuleIdx) -> bool {
    match self {
      Self::Dense(visited) => visited[idx.index()],
      Self::Sparse(visited) => visited.contains(&idx),
    }
  }
}

#[cfg(feature = "experimental")]
impl Bundler {
  #[tracing::instrument(level = "debug", skip_all)]
  /// `last_build_errored` disables the unchanged-output suppression: a recovery
  /// that rebuilds byte-identical output must still reach clients stuck on the
  /// error — see `HmrStage::compute_hmr_update_for_file_changes`.
  pub async fn compute_hmr_update_for_file_changes(
    &mut self,
    changed_file_paths: &FxIndexMap<String, WatcherChangeKind>,
    clients: &[ClientHmrInput<'_>],
    stamp_table: &mut HmrStampTable,
    next_hmr_patch_id: Arc<AtomicU32>,
    last_build_errored: bool,
    hot_update_hook_enabled: bool,
  ) -> BuildResult<Vec<ClientHmrUpdate>> {
    // HMR partial scans use the shared rayon pool without passing through
    // `BundleFactory::build_bundle`; wait for any deferred drops here too.
    crate::utils::defer_drop::drain();

    let Some(plugin_driver) = self.last_bundle_handle.as_ref().map(|ctx| &ctx.plugin_driver) else {
      return Err(anyhow::format_err!(
        "HMR requires to run at least one bundle before invalidation"
      ))?;
    };
    let mut hmr_stage = HmrStage::new(HmrStageInput {
      fs: self.bundle_factory.fs.clone(),
      options: Arc::clone(&self.bundle_factory.options),
      resolver: Arc::clone(&self.bundle_factory.resolver),
      plugin_driver: Arc::clone(plugin_driver),
      cache: &mut self.cache,
      lost_hmr_update: &mut self.lost_hmr_update,
      next_hmr_patch_id,
    });
    hmr_stage
      .compute_hmr_update_for_file_changes(
        changed_file_paths,
        clients,
        stamp_table,
        last_build_errored,
        hot_update_hook_enabled,
      )
      .await
  }

  /// Compute the top-level-evaluated set of the current snapshot: the modules whose
  /// evaluation is unconditionally triggered by entry-chunk top-level execution.
  /// These are the modules reachable from the user-defined entry through
  /// `ImportKind::Import` edges only — static `import` / `export … from` statements,
  /// whose init calls the renderer hoists unconditionally into the importer.
  /// `Require` and dynamic edges are excluded: a `require()` site may be
  /// conditional, so counting it could mark a module "evaluated" that never ran —
  /// the one unsafe direction. Missing a module here only costs re-shipped bytes.
  /// Nor does an import that development output drops as an unused import of a module without
  /// side effects (`hmr::utils::drops_import`) certainly evaluate its target: the importer's
  /// record imports it bare only where the output gives it a record of its own, as a record does
  /// a module that it names in `import.meta.hot.accept()` or `new URL()`, which the second value
  /// counts as one that a client may or may not have evaluated; and an `import defer` evaluates
  /// its target only once its module reads the namespace (`hmr::utils::deferred_records`): the
  /// walk skips both, as the HMR stage's demand check does.
  /// An import that the scan decides as a native cell bypass evaluates only the cells and
  /// literal records of the bindings that its module reads, also when its donor only forwards
  /// them through re-exports (`native_cell_routes::evaluation_routes`).
  /// An import whose native cell bypass the scan cannot decide
  /// (`native_cell_routes::Verdict::Uncertain`) is excluded for the same reason: the
  /// full build may have bound the importer straight to the cells, without evaluating
  /// the donor's closure. The second value names, by stable id, the modules that only
  /// such imports reach, which a client may or may not have evaluated.
  /// An `export *` whose evaluation edge a native build omitted
  /// (`native_records::omittable_star_evaluation`) doesn't evaluate its donor either: the
  /// walk skips it, as the cached link output decides, and counts a pure leaf that only such
  /// a star reaches as uncertain when that output can't decide. The third value names, by
  /// stable id, the donors that only such stars reach, which no client ran through the
  /// entry: a factory that a patch ships for one runs on first demand.
  ///
  /// Values are the modules' render-time stamps at computation, so consumers
  /// compare currency with `HmrStampTable::is_stale` exactly like the ship map.
  ///
  /// Returns empty (inert) results when there is no snapshot or when the build has
  /// several user-defined entries: the server cannot yet tell which entry a given
  /// client loaded, and a union would mark modules evaluated for clients that
  /// never loaded them. [`Self::compute_entry_evaluated_modules`] evaluates each entry
  /// by itself.
  pub fn compute_top_level_evaluated_modules(
    &self,
    stamp_table: &HmrStampTable,
  ) -> TopLevelEvaluation {
    let (mut entries, ..) =
      self.compute_entry_evaluated_modules(stamp_table, self.recorded_modules().as_ref());
    match entries.pop() {
      Some((_, evaluation)) if entries.is_empty() => evaluation,
      _ => Default::default(),
    }
  }

  /// [`Self::compute_top_level_evaluated_modules`] of each user-defined entry by itself, by
  /// the entry's module id. The second value names, by module id, the donors of the imports
  /// that the scan decides as native cell bypasses or can't decide, and the targets of the
  /// imports that the walk skips: a module that imports one may have bound its cells instead, or
  /// not run it yet, so a client ran such a module only where the evaluation of an entry that it
  /// loaded says so. The third names, by the entry's module id, the modules, by stable id, that
  /// only an `import defer` reaches, of a module that the entry ran or holds, through what the
  /// deferred modules import or require, which the page's records may still run once the module
  /// reads the namespace: a factory of one would run a second instance.
  /// The fourth names, by the entry's module id, what the page's records hold, by stable id with
  /// render-time stamps: the modules that they ran, and the records that their `require`s
  /// import, which register their modules only once those `require`s run, with what such a
  /// record imports or requires in turn, also past a route or an `export *` that the scan can't
  /// decide, but not past an `import defer`, whose targets the third value names.
  /// A record imports a module that has a record of its own also where it binds nothing of it,
  /// through an unused import that development output drops, which the first walk skips above,
  /// an `import.meta.hot.accept()` of it or a `new URL()` of it (`compute_cross_chunk_links`
  /// imports what any record but an `import()` names), and evaluates the module as it loads. A
  /// page of an output that gave the module no record ran nothing of it, so the walks count such
  /// a module as one that the records may or may not have run where they ran or may have run its
  /// importer, hold where they hold its importer, and may run later where they may run its
  /// importer later, unless `recorded`, the modules that the outputs that pages may have loaded
  /// gave records of their own ([`Self::recorded_modules`]), lacks it.
  pub fn compute_entry_evaluated_modules(
    &self,
    stamp_table: &HmrStampTable,
    recorded: Option<&std::collections::BTreeSet<String>>,
  ) -> EntryEvaluations {
    self.evaluate_records(stamp_table, recorded, None)
  }

  /// What the record of each of these modules (by stable id) certainly evaluates as it
  /// evaluates, by the module's stable id, itself included, with the modules' render-time stamps:
  /// the first value of [`Self::compute_entry_evaluated_modules`]'s walk from the module as from
  /// an entry, which passes no `import defer`, `require` or `import()`, no import that development
  /// output drops, no `export *` whose evaluation edge a native build omitted, and no native cell
  /// bypass, but for the modules of the module's import cycles, which its record may run before,
  /// and what only they reach.
  pub fn compute_record_evaluations<'a>(
    &self,
    stamp_table: &HmrStampTable,
    recorded: Option<&std::collections::BTreeSet<String>>,
    stable_ids: impl IntoIterator<Item = &'a str>,
  ) -> FxHashMap<ArcStr, FxHashMap<ArcStr, u32>> {
    let roots = stable_ids
      .into_iter()
      .filter_map(|stable_id| self.cache.module_idx_by_stable_id.get(stable_id).copied())
      .collect::<Vec<_>>();
    if roots.is_empty() {
      return FxHashMap::default();
    }
    let (entries, ..) = self.evaluate_records(stamp_table, recorded, Some(roots.clone()));
    let Some(snapshot) = self.cache.snapshot() else {
      return FxHashMap::default();
    };
    let modules = &snapshot.module_table.modules;
    // The walks give one evaluation for each root, in order.
    roots
      .into_iter()
      .zip(entries)
      .map(|(root, (_, (evaluated, ..)))| {
        (modules[root].stable_id().as_arc_str().clone(), evaluated)
      })
      .collect()
  }

  /// The walks of [`Self::compute_entry_evaluated_modules`], from `roots`, or from the
  /// user-defined entries.
  fn evaluate_records(
    &self,
    stamp_table: &HmrStampTable,
    recorded: Option<&std::collections::BTreeSet<String>>,
    roots: Option<Vec<rolldown_common::ModuleIdx>>,
  ) -> EntryEvaluations {
    let Some(snapshot) = self.cache.snapshot() else {
      return Default::default();
    };

    let modules = &snapshot.module_table.modules;
    let options = &self.bundle_factory.options;
    let native_records = crate::utils::native_records::enabled(options);
    // The link output of the build that the page loaded, which fast relinks keep up to date.
    let link_output = self.cache.cached_native_relink.as_ref().map(|cached| &cached.link_output);
    // Whether a native build omitted the evaluation edge of a module's `export *` record
    // (`native_records::omittable_star_evaluation`), so that the record never evaluated its
    // donor: the link's own decision, or `None` for a pure leaf's star when the cached link
    // output can't decide it.
    let omitted_star = |module: &rolldown_common::NormalModule, rec_idx: ImportRecordIdx| {
      let rec = &module.import_records[rec_idx];
      if !native_records || !rec.meta.contains(ImportRecordMeta::IsExportStar) {
        return Some(false);
      }
      if let Some(link_output) = link_output
        && let Some(linked) =
          link_output.module_table.modules.get(module.idx).and_then(Module::as_normal)
        && linked.stable_id.as_str() == module.stable_id.as_str()
        && linked.import_records.len() == module.import_records.len()
      {
        return Some(crate::utils::native_records::omittable_star_evaluation(
          options,
          link_output,
          linked,
          rec_idx,
        ));
      }
      let pure_leaf =
        rec.resolved_module.and_then(|idx| modules[idx].as_normal()).is_some_and(|donor| {
          donor.import_records.is_empty()
            && !donor.side_effects.has_side_effects()
            && !donor.meta.has_eval()
        });
      (!pure_leaf || module.meta.has_eval()).then_some(false)
    };
    // Whether a module has no side effects as the link of the page's records gave them
    // (`LinkStage::determine_side_effects`), with those that its imports reach, which keep an
    // unused import of it that the scan's own classification would drop (#697). The cached link
    // output gives them only while it links the snapshot's scan, before any scan rescans or
    // appends a module: a rollback to the base of a superseded build rescans what that build
    // changed without relinking it, which leaves the superseded build's link in the output.
    // Otherwise, as for a module that the output doesn't describe, the scan's own classification
    // decides, which only drops more.
    let linked = link_output.filter(|_| {
      self.cache.dirty_native_modules.is_empty()
        && self.cache.added_native_modules.is_empty()
        && self.cache.native_importer_changes.is_empty()
    });
    let pure = |idx: rolldown_common::ModuleIdx| {
      let Some(module) = modules[idx].as_normal() else {
        return false;
      };
      let side_effects = linked
        .and_then(|link_output| link_output.module_table.modules.get(idx)?.as_normal())
        .filter(|linked| linked.stable_id.as_str() == module.stable_id.as_str())
        .map_or(&module.side_effects, |linked| &linked.side_effects);
      !side_effects.has_side_effects()
    };
    // Whether no output that a page may have loaded gave a module a record of its own, so that no
    // record imports it bare.
    let recordless = |idx: rolldown_common::ModuleIdx| {
      recorded.is_some_and(|recorded| {
        !modules[idx]
          .as_normal()
          .is_some_and(|module| recorded.contains(module.stable_id.as_str()))
      })
    };
    // A bypassed import evaluates the native cells and literal records of the bindings that its
    // module reads, never its donor, which may only forward them.
    let routes = if !self.cache.native_cell_modules.is_empty()
      && crate::utils::native_records::enabled(&self.bundle_factory.options)
    {
      crate::hmr::native_cell_routes::evaluation_routes(
        modules,
        &snapshot.stmt_infos,
        &snapshot.index_ecma_ast,
        &snapshot.user_defined_entry_modules,
      )
    } else {
      Vec::new()
    };
    let undecidable = routes
      .iter()
      .any(|route| route.verdict == crate::hmr::native_cell_routes::Verdict::Uncertain);
    let mut bypassable: FxHashSet<_> = routes
      .iter()
      .filter(|route| route.verdict != crate::hmr::native_cell_routes::Verdict::Evaluate)
      .map(|route| modules[route.donor].id().as_arc_str().clone())
      .collect();
    let mut postponed = FxHashMap::default();
    let mut holds = FxHashMap::default();
    // Each importer's routes, which the walk of every entry reads.
    let mut importer_routes: FxHashMap<_, Vec<_>> = FxHashMap::default();
    for route in routes {
      importer_routes.entry(route.importer).or_default().push(route);
    }
    let deferred_by_module: FxHashMap<_, _> = modules
      .iter()
      .filter_map(Module::as_normal)
      .filter_map(|module| {
        let ast = snapshot.index_ecma_ast[module.idx].as_ref()?;
        let records = crate::hmr::utils::deferred_records(module, ast.program());
        (!records.is_empty()).then_some((module.idx, records))
      })
      .collect();
    let has_deferred_imports = !deferred_by_module.is_empty();
    // The walks from given roots, as `compute_record_evaluations` asks for, are many small ones,
    // whose state grows with what they reach rather than with the graph.
    let sparse = roots.is_some();
    let roots =
      roots.unwrap_or_else(|| snapshot.user_defined_entry_modules.iter().copied().collect());
    let mut entries = Vec::with_capacity(roots.len());
    for &entry in &roots {
      let mut visited = Visited::new(sparse, modules.len());
      let mut evaluated = FxHashMap::with_capacity_and_hasher(
        if sparse { 0 } else { modules.len() },
        Default::default(),
      );
      let mut uncertain = FxHashSet::default();
      let mut stack = vec![entry];
      // The donors of the undecided bypasses that a certainly evaluated module imports, and the
      // modules that a record may or may not evaluate as it loads.
      let mut deferred = Vec::new();
      // The donors of the omitted stars that an evaluated module re-exports.
      let mut omitted_donors = Vec::new();
      // The targets of the `import defer` records of the modules that the walks reach.
      let mut later = Vec::new();
      // The targets of the `require` records of the modules that the entry evaluates or may
      // evaluate, whose records may run them later.
      let mut required = Vec::new();
      // The targets of the `import()` records that the entry reaches, whose records a page of
      // the entry may open and hold.
      let mut dynamic = Vec::new();
      // The modules that each certainly evaluated module's record certainly evaluates as it
      // evaluates, for a walk from given roots.
      let mut certain_edges = FxHashMap::default();
      // First the modules that the entry certainly evaluates, then those that only the
      // deferred donors reach, with the donors that those leave undecided in turn.
      for certain in [true, false] {
        if !certain {
          stack = std::mem::take(&mut deferred);
        }
        while let Some(module_idx) =
          stack.pop().or_else(|| if certain { None } else { deferred.pop() })
        {
          if !visited.insert(module_idx) {
            continue;
          }
          let Module::Normal(module) = &modules[module_idx] else {
            continue;
          };
          let stable_id = module.stable_id.as_arc_str();
          if certain {
            evaluated.insert(stable_id.clone(), stamp_table.render_time_stamp(stable_id.as_str()));
          } else {
            uncertain.insert(stable_id.clone());
          }
          let routes = importer_routes.get(&module_idx).map_or(&[][..], Vec::as_slice);
          let bypassed = crate::hmr::native_cell_routes::bypass_evaluations(routes, module_idx);
          let undecided = if certain && undecidable {
            crate::hmr::native_cell_routes::uncertain_records(routes, module_idx)
          } else {
            FxHashSet::default()
          };
          let scoping = crate::hmr::utils::import_scoping(
            &snapshot.symbol_ref_db,
            self.cache.cached_native_relink.as_deref(),
            module,
            false,
          );
          let deferred_imports = deferred_by_module.get(&module_idx);
          // The modules that the module's record imports bare, binding nothing of them, which it
          // evaluates as it loads where they have records of their own: a page may or may not
          // have run one.
          let mut unbound = Vec::new();
          let pushed = stack.len();
          for (rec_idx, rec) in module.import_records.iter_enumerated() {
            if let Some(owners) = bypassed.get(&rec_idx) {
              stack.extend(owners.iter().copied());
            } else if rec.kind == ImportKind::Import
              && let Some(dep_idx) = rec.resolved_module
            {
              if deferred_imports.is_some_and(|records| records.contains(&rec_idx)) {
                later.push(dep_idx);
              } else if crate::hmr::utils::drops_import(module, rec_idx, &pure, scoping) {
                bypassable.insert(modules[dep_idx].id().as_arc_str().clone());
                unbound.push(dep_idx);
              } else {
                match omitted_star(module, rec_idx) {
                  Some(true) => omitted_donors.push(dep_idx),
                  None => deferred.push(dep_idx),
                  Some(false) if undecided.contains(&rec_idx) => deferred.push(dep_idx),
                  Some(false) => stack.push(dep_idx),
                }
              }
            } else if rec.kind == ImportKind::Require
              && let Some(dep_idx) = rec.resolved_module
            {
              required.push(dep_idx);
            } else if has_deferred_imports
              && rec.kind == ImportKind::DynamicImport
              && let Some(dep_idx) = rec.resolved_module
              && !recordless(dep_idx)
            {
              dynamic.push(dep_idx);
            } else if matches!(rec.kind, ImportKind::HotAccept | ImportKind::NewUrl)
              && let Some(dep_idx) = rec.resolved_module
            {
              unbound.push(dep_idx);
            }
          }
          if sparse && certain {
            certain_edges.insert(module_idx, stack[pushed..].to_vec());
          }
          deferred.extend(unbound.into_iter().filter(|&idx| !recordless(idx)));
        }
      }
      // A record that runs as one of the modules of an import cycle runs before the other modules
      // of the cycle through which the evaluation entered it finish, which they may never do, as
      // where a later import of theirs throws. So what a module's record certainly evaluated
      // leaves out the modules that its records may reach, through any import but `import()`,
      // that may import the module back in turn, with what only those reach (#749).
      if sparse {
        let mut targets_of = FxHashMap::default();
        let mut pending = vec![entry];
        while let Some(module_idx) = pending.pop() {
          if targets_of.contains_key(&module_idx) {
            continue;
          }
          let targets = modules[module_idx].as_normal().map_or_else(Vec::new, |module| {
            let routes = importer_routes.get(&module_idx).map_or(&[][..], Vec::as_slice);
            let bypassed = crate::hmr::native_cell_routes::bypass_evaluations(routes, module_idx);
            module
              .import_records
              .iter_enumerated()
              .flat_map(|(rec_idx, rec)| {
                bypassed
                  .get(&rec_idx)
                  .copied()
                  .into_iter()
                  .flatten()
                  .copied()
                  .chain(rec.resolved_module.filter(|_| rec.kind != ImportKind::DynamicImport))
              })
              .collect::<Vec<_>>()
          });
          pending.extend(targets.iter().copied());
          targets_of.insert(module_idx, targets);
        }
        let mut importers_of = FxHashMap::<_, Vec<_>>::default();
        for (&importer, targets) in &targets_of {
          for &target in targets {
            importers_of.entry(target).or_default().push(importer);
          }
        }
        let mut cyclic = FxHashSet::default();
        let mut pending = importers_of.get(&entry).cloned().unwrap_or_default();
        while let Some(module_idx) = pending.pop() {
          if module_idx != entry && cyclic.insert(module_idx) {
            pending.extend(importers_of.get(&module_idx).into_iter().flatten().copied());
          }
        }
        if !cyclic.is_empty() {
          let mut kept = FxHashSet::default();
          let mut pending = vec![entry];
          while let Some(module_idx) = pending.pop() {
            if !cyclic.contains(&module_idx) && kept.insert(module_idx) {
              pending.extend(certain_edges.get(&module_idx).into_iter().flatten().copied());
            }
          }
          let kept = kept
            .into_iter()
            .filter_map(|idx| modules[idx].as_normal())
            .map(|module| module.stable_id.as_str())
            .collect::<FxHashSet<_>>();
          evaluated.retain(|stable_id, _| kept.contains(stable_id.as_str()));
        }
      }
      let omitted = omitted_donors
        .into_iter()
        .filter(|idx| !visited.contains(*idx))
        .filter_map(|idx| modules[idx].as_normal())
        .map(|module| module.stable_id.as_arc_str().clone())
        .collect();
      // What the records hold besides what they ran: the records that their `require`s import,
      // which register their modules only once those `require`s run, and what such a record
      // imports or requires in turn, which it runs with it, also past a route or an `export *`
      // that the scan can't decide, or bare, as a target that has a record of its own. Its
      // `import defer`s reach what it may run later still.
      let mut held = evaluated.clone();
      while let Some(module_idx) = required.pop() {
        let Module::Normal(module) = &modules[module_idx] else {
          continue;
        };
        let stable_id = module.stable_id.as_arc_str();
        if held.contains_key(stable_id.as_str()) {
          continue;
        }
        held.insert(stable_id.clone(), stamp_table.render_time_stamp(stable_id.as_str()));
        let routes = importer_routes.get(&module_idx).map_or(&[][..], Vec::as_slice);
        let bypassed = crate::hmr::native_cell_routes::bypass_evaluations(routes, module_idx);
        let scoping = crate::hmr::utils::import_scoping(
          &snapshot.symbol_ref_db,
          self.cache.cached_native_relink.as_deref(),
          module,
          false,
        );
        let deferred_imports = deferred_by_module.get(&module_idx);
        for (rec_idx, rec) in module.import_records.iter_enumerated() {
          if let Some(owners) = bypassed.get(&rec_idx) {
            required.extend(owners.iter().copied());
          } else if let Some(dep_idx) = rec.resolved_module {
            match rec.kind {
              ImportKind::Require => required.push(dep_idx),
              ImportKind::DynamicImport if has_deferred_imports && !recordless(dep_idx) => {
                dynamic.push(dep_idx);
              }
              ImportKind::Import
                if deferred_imports.is_some_and(|records| records.contains(&rec_idx)) =>
              {
                later.push(dep_idx);
              }
              // The record imports bare a target that has a record of its own, or may.
              ImportKind::Import
                if crate::hmr::utils::drops_import(module, rec_idx, &pure, scoping) =>
              {
                if !recordless(dep_idx) {
                  required.push(dep_idx);
                }
              }
              ImportKind::HotAccept | ImportKind::NewUrl if !recordless(dep_idx) => {
                required.push(dep_idx);
              }
              ImportKind::Import if omitted_star(module, rec_idx) != Some(true) => {
                required.push(dep_idx);
              }
              _ => {}
            }
          }
        }
      }
      // What only those `import defer` records reach, through what its modules import, bare
      // too, or require, which the records run once a module reads the namespace.
      let mut reached = FxHashSet::default();
      while let Some(idx) = later.pop() {
        let Some(module) = modules[idx].as_normal() else {
          continue;
        };
        let stable_id = module.stable_id.as_arc_str();
        if evaluated.contains_key(stable_id.as_str()) || !reached.insert(stable_id.clone()) {
          continue;
        }
        bypassable.insert(module.id.as_arc_str().clone());
        later.extend(
          module
            .import_records
            .iter()
            .filter_map(|rec| match rec.kind {
              ImportKind::Import | ImportKind::Require => rec.resolved_module,
              ImportKind::HotAccept | ImportKind::NewUrl => {
                rec.resolved_module.filter(|&idx| !recordless(idx))
              }
              ImportKind::DynamicImport => {
                if let Some(dep_idx) = rec.resolved_module.filter(|&idx| !recordless(idx)) {
                  dynamic.push(dep_idx);
                }
                None
              }
              _ => None,
            }),
        );
      }
      // A route whose native records a page may open and hold defers what only its `import defer`
      // records reach, which its own static imports don't evaluate as the route loads (#696).
      // A walk from given roots needs only what their records certainly evaluate
      // (`compute_record_evaluations`), and opens no route.
      if sparse {
        dynamic.clear();
      }
      let mut visited_routes = FxHashSet::default();
      while let Some(route_idx) = dynamic.pop() {
        if !visited_routes.insert(route_idx) {
          continue;
        }
        let mut route_visited = vec![false; modules.len()];
        let mut route_evaluated = FxHashSet::default();
        let mut route_stack = vec![route_idx];
        let mut route_deferred = Vec::new();
        let mut route_required = Vec::new();
        let mut route_later = Vec::new();
        for certain in [true, false] {
          if !certain {
            route_stack = std::mem::take(&mut route_deferred);
          }
          while let Some(module_idx) =
            route_stack.pop().or_else(|| if certain { None } else { route_deferred.pop() })
          {
            if std::mem::replace(&mut route_visited[module_idx.index()], true) {
              continue;
            }
            let Module::Normal(module) = &modules[module_idx] else {
              continue;
            };
            let stable_id = module.stable_id.as_arc_str();
            if evaluated.contains_key(stable_id.as_str()) {
              continue;
            }
            if certain {
              route_evaluated.insert(stable_id.clone());
            }
            let routes = importer_routes.get(&module_idx).map_or(&[][..], Vec::as_slice);
            let bypassed = crate::hmr::native_cell_routes::bypass_evaluations(routes, module_idx);
            let undecided = if certain && undecidable {
              crate::hmr::native_cell_routes::uncertain_records(routes, module_idx)
            } else {
              FxHashSet::default()
            };
            let scoping = crate::hmr::utils::import_scoping(
              &snapshot.symbol_ref_db,
              self.cache.cached_native_relink.as_deref(),
              module,
              false,
            );
            let deferred_imports = deferred_by_module.get(&module_idx);
            let mut unbound = Vec::new();
            for (rec_idx, rec) in module.import_records.iter_enumerated() {
              if let Some(owners) = bypassed.get(&rec_idx) {
                route_stack.extend(owners.iter().copied());
              } else if rec.kind == ImportKind::Import
                && let Some(dep_idx) = rec.resolved_module
              {
                if deferred_imports.is_some_and(|records| records.contains(&rec_idx)) {
                  route_later.push(dep_idx);
                } else if crate::hmr::utils::drops_import(module, rec_idx, &pure, scoping) {
                  bypassable.insert(modules[dep_idx].id().as_arc_str().clone());
                  unbound.push(dep_idx);
                } else {
                  match omitted_star(module, rec_idx) {
                    Some(true) => {}
                    None => route_deferred.push(dep_idx),
                    Some(false) if undecided.contains(&rec_idx) => route_deferred.push(dep_idx),
                    Some(false) => route_stack.push(dep_idx),
                  }
                }
              } else if rec.kind == ImportKind::Require
                && let Some(dep_idx) = rec.resolved_module
              {
                route_required.push(dep_idx);
              } else if rec.kind == ImportKind::DynamicImport
                && let Some(dep_idx) = rec.resolved_module
                && !recordless(dep_idx)
              {
                dynamic.push(dep_idx);
              } else if matches!(rec.kind, ImportKind::HotAccept | ImportKind::NewUrl)
                && let Some(dep_idx) = rec.resolved_module
              {
                unbound.push(dep_idx);
              }
            }
            route_deferred.extend(unbound.into_iter().filter(|&idx| !recordless(idx)));
          }
        }
        let mut route_held = route_evaluated.clone();
        while let Some(module_idx) = route_required.pop() {
          let Module::Normal(module) = &modules[module_idx] else {
            continue;
          };
          let stable_id = module.stable_id.as_arc_str();
          if evaluated.contains_key(stable_id.as_str()) || !route_held.insert(stable_id.clone()) {
            continue;
          }
          let routes = importer_routes.get(&module_idx).map_or(&[][..], Vec::as_slice);
          let bypassed = crate::hmr::native_cell_routes::bypass_evaluations(routes, module_idx);
          let scoping = crate::hmr::utils::import_scoping(
            &snapshot.symbol_ref_db,
            self.cache.cached_native_relink.as_deref(),
            module,
            false,
          );
          let deferred_imports = deferred_by_module.get(&module_idx);
          for (rec_idx, rec) in module.import_records.iter_enumerated() {
            if let Some(owners) = bypassed.get(&rec_idx) {
              route_required.extend(owners.iter().copied());
            } else if let Some(dep_idx) = rec.resolved_module {
              match rec.kind {
                ImportKind::Require => route_required.push(dep_idx),
                ImportKind::DynamicImport if !recordless(dep_idx) => dynamic.push(dep_idx),
                ImportKind::Import
                  if deferred_imports.is_some_and(|records| records.contains(&rec_idx)) =>
                {
                  route_later.push(dep_idx);
                }
                ImportKind::Import
                  if crate::hmr::utils::drops_import(module, rec_idx, &pure, scoping) =>
                {
                  if !recordless(dep_idx) {
                    route_required.push(dep_idx);
                  }
                }
                ImportKind::HotAccept | ImportKind::NewUrl if !recordless(dep_idx) => {
                  route_required.push(dep_idx);
                }
                ImportKind::Import if omitted_star(module, rec_idx) != Some(true) => {
                  route_required.push(dep_idx);
                }
                _ => {}
              }
            }
          }
        }
        let mut route_reached = FxHashSet::default();
        while let Some(idx) = route_later.pop() {
          let Some(module) = modules[idx].as_normal() else {
            continue;
          };
          let stable_id = module.stable_id.as_arc_str();
          if evaluated.contains_key(stable_id.as_str())
            || route_evaluated.contains(stable_id.as_str())
            || !route_reached.insert(stable_id.clone())
          {
            continue;
          }
          bypassable.insert(module.id.as_arc_str().clone());
          route_later.extend(module.import_records.iter().filter_map(|rec| match rec.kind {
            ImportKind::Import | ImportKind::Require => rec.resolved_module,
            ImportKind::HotAccept | ImportKind::NewUrl => {
              rec.resolved_module.filter(|&idx| !recordless(idx))
            }
            ImportKind::DynamicImport => {
              if let Some(dep_idx) = rec.resolved_module.filter(|&idx| !recordless(idx)) {
                dynamic.push(dep_idx);
              }
              None
            }
            _ => None,
          }));
        }
        reached.extend(route_reached);
      }
      let id = modules[entry].id().as_arc_str().clone();
      postponed.insert(id.clone(), reached);
      holds.insert(id.clone(), held);
      entries.push((id, (evaluated, uncertain, omitted)));
    }
    (entries, bypassable, postponed, holds)
  }

  /// The modules, by stable id, that the native records of the last link's output give a record
  /// of their own, which a record that imports such a module evaluates as it loads, also where
  /// it imports the module bare, binding nothing of it (`compute_cross_chunk_links`). A module
  /// that the graph reaches only through `import.meta.hot.accept()` has none (#679). `None`
  /// where that link no longer describes the scan, as after a rescan that no link followed.
  pub fn recorded_modules(&self) -> Option<std::collections::BTreeSet<String>> {
    let cached = self.cache.cached_native_relink.as_ref()?;
    if !crate::utils::native_records::enabled(&self.bundle_factory.options)
      || !self.cache.dirty_native_modules.is_empty()
      || !self.cache.added_native_modules.is_empty()
      || !self.cache.native_importer_changes.is_empty()
    {
      return None;
    }
    let chunks = &cached.generate_state.chunk_graph.module_to_chunk;
    Some(
      cached
        .link_output
        .module_table
        .modules
        .iter()
        .filter_map(Module::as_normal)
        .filter(|module| chunks.get(module.idx).copied().flatten().is_some())
        .map(|module| module.stable_id.as_str().to_owned())
        .collect(),
    )
  }

  /// The modules, by module id, that name each module, by its module id, in an
  /// `import.meta.hot.accept()`, whose records import it bare where the output gives it a record
  /// of its own ([`Self::recorded_modules`]).
  pub fn hot_accepting_modules(&self) -> FxHashMap<ArcStr, Vec<ArcStr>> {
    let mut accepting: FxHashMap<ArcStr, Vec<ArcStr>> = FxHashMap::default();
    let Some(snapshot) = self.cache.snapshot() else {
      return accepting;
    };
    let modules = &snapshot.module_table.modules;
    for module in modules.iter().filter_map(Module::as_normal) {
      for rec in module.import_records.iter().filter(|rec| rec.kind == ImportKind::HotAccept) {
        if let Some(target) = rec.resolved_module {
          accepting
            .entry(modules[target].id().as_arc_str().clone())
            .or_default()
            .push(module.id.as_arc_str().clone());
        }
      }
    }
    accepting
  }

  /// Whether any of these modules (by stable id) uses top-level await. Such a module cannot
  /// run as a synchronous registry factory.
  pub fn uses_top_level_await<'a>(&self, stable_ids: impl IntoIterator<Item = &'a str>) -> bool {
    let Some(snapshot) = self.cache.snapshot() else {
      return false;
    };
    stable_ids.into_iter().any(|stable_id| {
      self.cache.module_idx_by_stable_id.get(stable_id).is_some_and(|idx| {
        snapshot.module_table.modules[*idx].as_normal().is_some_and(|module| {
          module.ast_usage.contains(rolldown_common::EcmaModuleAstUsage::TopLevelAwait)
        })
      })
    })
  }

  /// The module ids this module (by stable id) loads with `import()`.
  pub fn dynamic_importees(&self, stable_id: &str) -> Vec<ArcStr> {
    let Some(snapshot) = self.cache.snapshot() else {
      return Vec::new();
    };
    let modules = &snapshot.module_table.modules;
    self
      .cache
      .module_idx_by_stable_id
      .get(stable_id)
      .and_then(|idx| modules[*idx].as_normal())
      .map(|module| {
        module
          .import_records
          .iter()
          .filter(|record| record.kind == ImportKind::DynamicImport)
          .filter_map(|record| record.resolved_module)
          .map(|idx| modules[idx].id().as_arc_str().clone())
          .collect()
      })
      .unwrap_or_default()
  }

  /// Whether a module of the current scan imports a module with `import defer`
  /// (`hmr::utils::deferred_records`), also one that a plugin wrote where the module's source
  /// holds none.
  pub fn has_deferred_imports(&self) -> bool {
    let Some(snapshot) = self.cache.snapshot() else {
      return false;
    };
    snapshot.module_table.modules.iter().filter_map(Module::as_normal).any(|module| {
      snapshot.index_ecma_ast[module.idx].as_ref().is_some_and(|ast| {
        !crate::hmr::utils::deferred_records(module, ast.program()).is_empty()
      })
    })
  }

  /// The imports of each of these modules (by stable id) that a factory of the module may run, as
  /// `(importer, target, kind, deferred, kept)`, by the HMR stage's model of their factories
  /// (`HmrStage::static_import_phases`).
  pub fn static_import_phases<'a>(
    &mut self,
    stable_ids: impl IntoIterator<Item = &'a str>,
  ) -> Vec<(ArcStr, ArcStr, ImportKind, bool, bool)> {
    if !self.cache.has_snapshot() {
      return Vec::new();
    }
    let Some(plugin_driver) =
      self.last_bundle_handle.as_ref().map(|ctx| Arc::clone(&ctx.plugin_driver))
    else {
      return Vec::new();
    };
    let mut lost_hmr_update = false;
    HmrStage::new(HmrStageInput {
      fs: self.bundle_factory.fs.clone(),
      options: Arc::clone(&self.bundle_factory.options),
      resolver: Arc::clone(&self.bundle_factory.resolver),
      plugin_driver,
      cache: &mut self.cache,
      next_hmr_patch_id: Arc::default(),
      lost_hmr_update: &mut lost_hmr_update,
    })
    .static_import_phases(stable_ids)
  }

  /// Compile a lazy entry module and return compiled code plus the pending-payload
  /// entry the delivery-time ship-map write consumes.
  ///
  /// This is called when a dynamically imported module is first requested at runtime.
  /// The module was previously stubbed with a proxy, and now we need to compile the
  /// actual module and its dependencies.
  #[expect(clippy::too_many_arguments)]
  pub async fn compile_lazy_entry(
    &mut self,
    module_id: String,
    client_id: &str,
    shipped: &FxHashMap<ArcStr, u32>,
    rerun: &FxHashSet<ArcStr>,
    evaluated: &FxHashMap<ArcStr, u32>,
    entry_evaluated: Option<&FxHashMap<ArcStr, u32>>,
    partly_run: Option<&FxHashSet<ArcStr>>,
    eager: Option<&mut FxHashSet<ArcStr>>,
    routed: Option<&mut Vec<(ArcStr, ArcStr)>>,
    stamp_table: &HmrStampTable,
    next_hmr_patch_id: Arc<AtomicU32>,
  ) -> BuildResult<HmrLazyChunkOutput> {
    // HMR partial scans use the shared rayon pool without passing through
    // `BundleFactory::build_bundle`; wait for any deferred drops here too.
    crate::utils::defer_drop::drain();

    let Some(plugin_driver) = self.last_bundle_handle.as_ref().map(|ctx| &ctx.plugin_driver) else {
      panic!("Lazy compilation requires at least one bundle to be built first");
    };
    let mut hmr_stage = HmrStage::new(HmrStageInput {
      fs: self.bundle_factory.fs.clone(),
      options: Arc::clone(&self.bundle_factory.options),
      resolver: Arc::clone(&self.bundle_factory.resolver),
      plugin_driver: Arc::clone(plugin_driver),
      cache: &mut self.cache,
      lost_hmr_update: &mut self.lost_hmr_update,
      next_hmr_patch_id,
    });
    hmr_stage
      .compile_lazy_entry(
        &module_id,
        client_id,
        shipped,
        rerun,
        evaluated,
        entry_evaluated,
        partly_run,
        eager,
        routed,
        stamp_table,
      )
      .await
  }

  /// Forgets how the factories that patches and lazy chunks shipped so far read CommonJS
  /// modules, before an update renders its own ([`Self::changed_commonjs_read`]).
  pub fn clear_commonjs_reads(&mut self) {
    self.cache.commonjs_reads.clear();
  }

  /// The first CommonJS module, by stable id, that a factory which a patch or a lazy chunk
  /// shipped since [`Self::clear_commonjs_reads`] reads otherwise than the link of the current
  /// scan decides, as after a lazy compile's partial scan adds an importer of the module's
  /// default, which makes the link convert its exports for every importer (#748).
  pub fn changed_commonjs_read(&mut self) -> Option<ArcStr> {
    if self.cache.commonjs_reads.is_empty() || !self.cache.has_snapshot() {
      return None;
    }
    let plugin_driver = Arc::clone(&self.last_bundle_handle.as_ref()?.plugin_driver);
    let mut lost_hmr_update = false;
    HmrStage::new(HmrStageInput {
      fs: self.bundle_factory.fs.clone(),
      options: Arc::clone(&self.bundle_factory.options),
      resolver: Arc::clone(&self.bundle_factory.resolver),
      plugin_driver,
      cache: &mut self.cache,
      next_hmr_patch_id: Arc::default(),
      lost_hmr_update: &mut lost_hmr_update,
    })
    .changed_commonjs_read()
  }
}
