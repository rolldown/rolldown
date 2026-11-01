use std::{
  ops::{Deref, DerefMut},
  path::Path,
  sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
  },
};

use arcstr::ArcStr;
use itertools::Itertools;
use oxc::ast::builder::AstBuilder;
use oxc_index::IndexVec;
use oxc_traverse::traverse_mut;
use rolldown_common::{
  ClientHmrInput, ClientHmrUpdate, ExportsKind, HmrLazyChunkOutput, HmrPatch, HmrStampTable,
  HmrUpdate, HybridIndexVec, ImportKind, ImportRecordIdx, ImportRecordMeta, ImporterRecord,
  IndexModules, Module, ModuleIdx, ModuleTable, NormalModule, ScanMode, Specifier,
  WatcherChangeKind, side_effects::DeterminedSideEffects,
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
  SharedOptions, SharedResolver,
  hmr::hmr_ast_finalizer::HmrAstFinalizer,
  module_loader::{ModuleLoader, module_loader::ModuleLoaderOutput},
  stages::{link_stage::Reachability, scan_stage::NormalizedScanStageOutput},
  type_alias::IndexEcmaAst,
  types::scan_stage_cache::ScanStageCache,
  utils::{
    process_code_and_sourcemap::process_code_and_sourcemap,
    render_ecma_module::collapse_module_sourcemap,
  },
};

pub struct HmrStageInput<'a, Fs: FileSystem + Clone + 'static> {
  pub options: SharedOptions,
  pub fs: Fs,
  pub resolver: SharedResolver<Fs>,
  pub plugin_driver: SharedPluginDriver,
  pub cache: &'a mut ScanStageCache,
  pub lost_hmr_update: &'a mut bool,
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
  /// What a link of the current module table reaches, which a rebuild of the table changes.
  reachability: LazyReachability,
}

/// What a link of a module table reaches from its user-defined entries and its runtime module
/// (`LinkStage::exclude_unreachable_modules`), computed on first use: few updates need it.
#[derive(Default)]
struct LazyReachability(std::sync::OnceLock<Reachability>);

impl LazyReachability {
  /// The reachability of `snapshot`'s module table, which this one describes.
  fn of(&self, snapshot: &NormalizedScanStageOutput) -> &Reachability {
    self.0.get_or_init(|| {
      Reachability::new(
        &snapshot.module_table.modules,
        snapshot
          .entry_points
          .iter()
          .filter(|entry| entry.kind.is_user_defined())
          .map(|entry| entry.idx)
          .chain(snapshot.user_defined_entry_modules.iter().copied())
          .chain(std::iter::once(snapshot.runtime.id())),
      )
    })
  }
}

/// Whether a link of a module table reaches a module: the importers that it doesn't reach
/// import nothing that a fresh graph binds (#660).
#[derive(Clone, Copy)]
struct Reach<'a> {
  reachability: &'a LazyReachability,
  snapshot: &'a NormalizedScanStageOutput,
}

impl Reach<'_> {
  fn reaches(self, idx: ModuleIdx) -> bool {
    self.reachability.of(self.snapshot).reaches(idx)
  }

  fn discovery_order(self, idx: ModuleIdx) -> u32 {
    self.reachability.of(self.snapshot).discovery_order(idx)
  }
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

/// Whether a partial scan that re-fetched only the module at `idx` found it as `cached` holds
/// it: the same source, side effects and resolved import records, and nothing new. Merging such
/// a scan changes nothing but the module's scan generation.
fn refetched_unchanged(cached: &ModuleTable, idx: ModuleIdx, output: &ModuleLoaderOutput) -> bool {
  let HybridIndexVec::Map(modules) = &output.module_table else {
    return false;
  };
  if modules.len() != 1
    || !output.new_added_modules_from_partial_scan.is_empty()
    || !output.entry_points.is_empty()
    || !output.dynamic_import_exports_usage_map.is_empty()
    || !output.overrode_preserve_entry_signature_map.is_empty()
    || !output.entry_point_to_reference_ids.is_empty()
  {
    return false;
  }
  let (Some(Module::Normal(new)), Some(Module::Normal(old))) =
    (modules.get(&idx), cached.modules.get(idx))
  else {
    return false;
  };
  old.module_type == new.module_type
    && old.ecma_view.source == new.ecma_view.source
    && std::mem::discriminant(&old.ecma_view.side_effects)
      == std::mem::discriminant(&new.ecma_view.side_effects)
    && old.ecma_view.side_effects.has_side_effects()
      == new.ecma_view.side_effects.has_side_effects()
    && old.ecma_view.import_records.len() == new.ecma_view.import_records.len()
    && old.ecma_view.import_records.iter().zip(&new.ecma_view.import_records).all(|(old, new)| {
      old.kind == new.kind
        && old.resolved_module == new.resolved_module
        && old.meta.bits() == new.meta.bits()
    })
}

impl<'a, Fs: FileSystem + Clone + 'static> HmrStage<'a, Fs> {
  pub fn new(input: HmrStageInput<'a, Fs>) -> Self {
    Self { input, reachability: LazyReachability::default() }
  }

  /// See `internal-docs/hmr/design.md` for the principles and invariants this
  /// stage implements. Stage order is documented in
  /// `internal-docs/dev-engine/implementation.md`
  /// ("Inside `compute_hmr_update_for_file_changes`").
  #[expect(clippy::too_many_lines)]
  pub async fn compute_hmr_update_for_file_changes(
    &mut self,
    changed_file_paths: &FxIndexMap<String, WatcherChangeKind>,
    clients: &[ClientHmrInput<'_>],
    stamp_table: &mut HmrStampTable,
    last_build_errored: bool,
    hot_update_hook_enabled: bool,
  ) -> BuildResult<Vec<ClientHmrUpdate>> {
    self.cache.last_fast_native_relink = false;
    self.cache.last_native_relink_fallback = None;
    self.cache.last_rebuilt_chunk_filenames.clear();
    self.cache.last_rebuilt_chunk_outputs.clear();
    tracing::trace!(
      changed_files = %changed_file_paths
        .iter()
        .map(|(path, kind)| format!("{path}:{kind}"))
        .join(", "),
      clients = %clients.iter().map(|client| client.client_id).join(", "),
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
      if !self.plugin_driver.transform_dependencies.is_empty() {
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
      }

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
    // Without clients, the update only rescans and merges the changed modules: nothing renders
    // a patch, so nothing captures what a patch compares against.
    let rescan_only = clients.is_empty();
    let pre_rebuild_cell_routes =
      if rescan_only { Vec::new() } else { self.native_cell_routes() };
    let pre_rebuild_side_effects = self.linked_side_effects();
    let pre_rebuild_renders = if last_build_errored || rescan_only {
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
          let (code, _) = self.render_module_code(
            render_input,
            0,
            0,
            false,
            &pre_rebuild_cell_routes,
            &pre_rebuild_side_effects,
          );
          (module_idx, code)
        })
        .collect::<Vec<_>>()
        .into_iter()
        .collect::<FxHashMap<_, _>>()
    };

    // Where a forwarded export of `imported` from `target` (`request`, if it doesn't resolve)
    // leads, following the re-exports of ES modules: the binding that an ES module declares,
    // named by all the names that the module exports it under, so that equivalent aliases
    // agree and an alias that moves to another binding doesn't; an ES module's namespace; the
    // binding that a module holds for an import that the scan can't follow, of a module
    // without ES exports, which the link may make CommonJS, or of an external module, each of
    // which every importer wraps in its own interop binding, named by the module and its
    // aliases; or a name that a module doesn't declare, a star's export. The first hop names
    // the import itself.
    let forward_destination =
      |stage: &Self, target: Option<ModuleIdx>, request: &str, imported: &Specifier| {
        let modules = &stage.module_table().modules;
        let es_module = |idx: Option<ModuleIdx>| {
          idx
            .and_then(|idx| modules[idx].as_normal())
            .filter(|module| matches!(module.exports_kind, ExportsKind::Esm))
        };
        let aliases = |module: &rolldown_common::NormalModule, referenced| {
          let mut aliases = module
            .named_exports
            .iter()
            .filter(|(_, other)| other.referenced == referenced)
            .map(|(alias, _)| alias.to_string())
            .collect::<Vec<_>>();
          aliases.sort_unstable();
          aliases
        };
        let first = || {
          let id =
            target.map_or_else(|| request.to_owned(), |idx| modules[idx].stable_id().to_string());
          let imported = match imported {
            Specifier::Star => vec![],
            Specifier::Literal(name) => vec![name.to_string()],
          };
          (id, 4, imported)
        };
        let Some(mut module) = es_module(target) else {
          return first();
        };
        let mut imported = imported.clone();
        for _ in 0..64 {
          let Specifier::Literal(export_name) = &imported else {
            return (module.stable_id.to_string(), 1, vec![]);
          };
          let Some(export) = module.named_exports.get(export_name) else {
            return (module.stable_id.to_string(), 3, vec![export_name.to_string()]);
          };
          let Some(import) = module.named_imports.get(&export.referenced) else {
            return (module.stable_id.to_string(), 0, aliases(module, export.referenced));
          };
          let Some(next) = es_module(module.import_records[import.record_idx].resolved_module)
          else {
            // Every export that reads the same name through the same import record is an alias
            // of one interop binding, whichever local symbol each import declared.
            let mut aliases = module
              .named_exports
              .iter()
              .filter(|(_, other)| {
                module.named_imports.get(&other.referenced).is_some_and(|other| {
                  other.record_idx == import.record_idx && other.imported == import.imported
                })
              })
              .map(|(alias, _)| alias.to_string())
              .collect::<Vec<_>>();
            aliases.sort_unstable();
            return (module.stable_id.to_string(), 2, aliases);
          };
          module = next;
          imported = import.imported.clone();
        }
        first()
      };
    // A native record's namespace is created by the engine from the module's static export
    // names, so an update that changes them cannot be applied to the resident record. Nor can
    // one that changes where a forwarded export leads: a native record forwards an export of an
    // imported binding to the target's own, without a cell that a patch could write, and a
    // record without a native namespace keeps the getters that its importers bound.
    let native_export_shape = |stage: &Self, module_idx: ModuleIdx| {
      let modules = &stage.module_table().modules;
      modules[module_idx].as_normal().map(|module| {
        let mut names = module.named_exports.keys().map(ToString::to_string).collect::<Vec<_>>();
        names.sort_unstable();
        let mut stars = module
          .star_export_module_ids()
          .map(|idx| modules[idx].stable_id().to_string())
          .collect::<Vec<_>>();
        stars.sort_unstable();
        let mut forwards = module
          .named_exports
          .iter()
          .filter_map(|(name, export)| {
            let import = module.named_imports.get(&export.referenced)?;
            let record = &module.import_records[import.record_idx];
            let destination = forward_destination(
              stage,
              record.resolved_module,
              &record.module_request,
              &import.imported,
            );
            Some((name.to_string(), destination))
          })
          .collect::<Vec<_>>();
        forwards.sort_unstable();
        (module.stable_id.to_string(), names, stars, forwards)
      })
    };
    // A native record build may omit the evaluation edge of an `export *` from a pure leaf
    // that contributes no binding (`native_records::omittable_star_evaluation`); a patch
    // cannot add or drop that edge in a live page. The barrel and leaf of every star edge
    // that could carry the certificate, checked before and after the rebuild so both the
    // resident and the candidate graph count, and whether the graph reaches the barrel. A
    // superset of the certificate's link-time conditions: a leaf's star contributes a binding,
    // and keeps its evaluation edge, when the leaf exports a name other than `default` that
    // neither the barrel itself nor any of its other stars may export (#625).
    let star_omissions = |stage: &Self| {
      let reach = stage.reach();
      let mut omissions = Vec::new();
      let modules = &stage.module_table().modules;
      for &star_idx in &stage.cache.export_star_modules {
        let Some(module) = modules.get(star_idx).and_then(Module::as_normal) else {
          continue;
        };
        if module.meta.has_eval() {
          continue;
        }
        let stars = module
          .import_records
          .iter()
          .filter(|record| {
            record.kind == ImportKind::Import
              && record.meta.contains(ImportRecordMeta::IsExportStar)
          })
          .filter_map(|record| record.resolved_module)
          .collect::<Vec<_>>();
        for &donor_idx in &stars {
          let Some(donor) = modules.get(donor_idx).and_then(Module::as_normal) else {
            continue;
          };
          if !donor.import_records.is_empty()
            || donor.side_effects.has_side_effects()
            || donor.meta.has_eval()
          {
            continue;
          }
          let shadowed = |name: &str| {
            module.named_exports.contains_key(name)
              || stars.iter().any(|&other| {
                other != donor_idx
                  && modules.get(other).and_then(Module::as_normal).is_none_or(|other| {
                    other.star_export_module_ids().next().is_some()
                      || other.named_exports.contains_key(name)
                  })
              })
          };
          if donor.named_exports.keys().all(|name| name == "default" || shadowed(name)) {
            omissions.push((module.idx, donor.idx, reach.reaches(module.idx)));
          }
        }
      }
      omissions
    };
    let native_records = crate::utils::native_records::enabled(&self.options);
    // A named import of a native cell's re-export may also omit its donor's evaluation
    // (`native_records::native_cell_evaluation_dependencies`). Scan data decides each such
    // candidate as bypass, evaluate or uncertain (`native_cell_routes`): a patch routes a
    // bypass the way the full build did, an edit that changes any verdict reloads, and an
    // edit touching an uncertain candidate reloads. An evaluation of a donor that every page
    // ran before the edit loads the page's instance, which needs no certificate.
    let route_certificate = |stage: &Self,
                             routes: &[crate::hmr::native_cell_routes::Route],
                             resident: &FxHashSet<ModuleIdx>| {
      let modules = &stage.module_table().modules;
      (
        crate::hmr::native_cell_routes::signature(modules, routes, resident),
        crate::hmr::native_cell_routes::uncertain_ends(modules, routes),
      )
    };
    let (
      pre_rebuild_route_ends,
      pre_rebuild_route_signature,
      pre_rebuild_star_omissions,
      pre_rebuild_resident,
    ) = if native_records && !rescan_only {
      let resident = self.native_cell_resident(&pre_rebuild_cell_routes);
      let (signature, ends) = route_certificate(self, &pre_rebuild_cell_routes, &resident);
      (ends, signature, star_omissions(self), resident)
    } else {
      (FxHashSet::default(), Vec::new(), Vec::new(), FxHashSet::default())
    };
    // The modules that a module's native record evaluates as it evaluates, and those that it
    // imports with `import defer`, which links a target without evaluating it (#625). A native
    // record imports the record of every target but a dynamic `import()`'s, `require`'s and a
    // dependency `import.meta.hot.accept`'s included, unless development output drops it as
    // an unused import of a side-effect-free module (`unused_side_effect_free_import`). A page
    // that evaluated a module evaluated those records, so an edit that keeps importing the same
    // leaves changes no demand on them; one that starts or stops importing a leaf, or starts
    // or stops using one, does. A dynamic `import()` loads its target on demand, and a patch
    // ships the factory of a target that a page may not have run.
    //
    // That needs the module's scope tree (`hmr::utils::import_scoping`). Without one, an import
    // that may be unused counts as dropped, which can only add a reload.
    let record_targets = |stage: &Self,
                          module_idx: ModuleIdx,
                          rescanned: bool,
                          side_effects: &LinkedSideEffects<'_>| {
      let module = stage.module_table().modules[module_idx].as_normal()?;
      let scoping = crate::hmr::utils::import_scoping(
        &stage.cache.get_snapshot().symbol_ref_db,
        stage.cache.cached_native_relink.as_deref(),
        module,
        rescanned,
      );
      let deferred_records =
        stage.index_ecma_ast()[module_idx].as_ref().map_or_else(FxHashSet::default, |ast| {
          crate::hmr::utils::deferred_records(module, ast.program())
        });
      let mut eager = FxHashSet::default();
      let mut deferred = FxHashSet::default();
      for (record_idx, record) in module.import_records.iter_enumerated() {
        let Some(target) = record.resolved_module else {
          continue;
        };
        if record.kind == ImportKind::DynamicImport
          || crate::hmr::utils::drops_import(
            module,
            record_idx,
            |idx| side_effects.pure(idx),
            scoping,
          )
        {
          continue;
        }
        if deferred_records.contains(&record_idx) {
          deferred.insert(target);
        } else {
          eager.insert(target);
        }
      }
      Some((eager, deferred))
    };
    // A wrapped leaf has no native namespace, so a barrel's star can't omit its evaluation
    // (`has_native_namespace`), and an edit that starts or stops requiring a module upstream of
    // the leaf changes whether a fresh build omits it, whatever module the edit touched (#642).
    let pre_rebuild_wrapped =
      if native_records && !rescan_only && !self.cache.export_star_modules.is_empty() {
        let reach = self.reach();
        wrapped_modules(&self.module_table().modules, |idx| reach.reaches(idx))
      } else {
        FxHashSet::default()
      };
    let pre_rebuild_imports: FxHashMap<ModuleIdx, (FxHashSet<ModuleIdx>, FxHashSet<ModuleIdx>)> =
      if native_records && !rescan_only {
        changed_modules
          .iter()
          .filter_map(|module_idx| {
            Some((
              *module_idx,
              record_targets(self, *module_idx, false, &pre_rebuild_side_effects)?,
            ))
          })
          .collect()
      } else {
        FxHashMap::default()
      };
    let pre_rebuild_export_shapes = if native_records && !rescan_only {
      changed_modules
        .iter()
        .filter_map(|module_idx| Some((*module_idx, native_export_shape(self, *module_idx)?)))
        .collect::<Vec<_>>()
    } else {
      Vec::new()
    };
    // A patch publishes a native record's new values by copying them into the resident
    // record's binding cells once. A binding the new generation's own code assigns later would
    // diverge from the namespace, so an edit that makes an export assignable reloads. Exports
    // the resident record already assigns have no writable cell; the client refuses those.
    let assigned_exports = |stage: &Self, module_idx: ModuleIdx| {
      let snapshot = stage.cache.get_snapshot();
      snapshot.module_table.modules[module_idx].as_normal().map(|module| {
        module
          .named_exports
          .iter()
          .filter(|(_, export)| {
            !export.came_from_commonjs
              && export.referenced.owner == module_idx
              && !module.named_imports.contains_key(&export.referenced)
              && (module.meta.has_eval()
                || !export.referenced.is_not_reassigned(&snapshot.symbol_ref_db))
          })
          .map(|(name, _)| name.to_string())
          .collect::<FxHashSet<_>>()
      })
    };
    let pre_rebuild_assigned_exports = pre_rebuild_export_shapes
      .iter()
      .filter_map(|(module_idx, _)| Some((*module_idx, assigned_exports(self, *module_idx)?)))
      .collect::<Vec<_>>();
    // A JSON module's importers that the graph reaches decide whether its namespace holds its
    // properties (`import_options::json_namespace_has_properties`). A page's resident namespace
    // can't gain or lose them, so an edit that changes that for a JSON module that a page may
    // hold reloads, whichever importer the edit touched or the graph stopped reaching. Only a
    // JSON module that the graph reaches before and after the edit counts: one that the edit
    // retires, or appends, holds no namespace that a page has and a fresh page reads (#660).
    let json_namespace_shapes = |stage: &Self| {
      let modules = &stage.module_table().modules;
      let reach = stage.reach();
      modules
        .iter()
        .filter_map(Module::as_normal)
        .filter(|module| {
          crate::utils::import_options::is_json_module(module) && reach.reaches(module.idx)
        })
        .map(|module| {
          (
            module.idx,
            crate::utils::import_options::json_namespace_has_properties(module, modules, |idx| {
              reach.reaches(idx)
            }),
          )
        })
        .collect::<FxHashMap<_, _>>()
    };
    let pre_rebuild_json_namespace_shapes =
      if rescan_only { FxHashMap::default() } else { json_namespace_shapes(self) };
    // A page's importers, dynamic imports included, bound a CommonJS module through its
    // interop and any other module through its namespace, as the resident build linked it
    // (`linked_exports_kind`), and the module registered as that kind: a plain object for a
    // module without exports, a namespace for an ES module, its `module` for CommonJS. A patch
    // changes neither a native record's registration nor the namespaces that importers hold,
    // so an edit that changes a module's kind reloads, whichever importer or module the edit
    // touched, also when the update's patch registers the module anew, or when the graph stops
    // reaching the importer that decided it (#660). A module that nothing imports binds nothing,
    // so becoming or ceasing to be one doesn't count. Only a module without ES or CommonJS
    // syntax, or one that the update rescans, can change its kind (#638).
    let pre_rebuild_exports_kinds = if native_records && !rescan_only {
      self
        .module_table()
        .modules
        .iter()
        .filter_map(Module::as_normal)
        .filter(|module| {
          module.exports_kind == ExportsKind::None || changed_modules.contains(&module.idx)
        })
        .filter_map(|module| Some((module.idx, self.linked_exports_kind(module)?)))
        .collect::<FxHashMap<_, _>>()
    } else {
      FxHashMap::default()
    };
    let modules = &self.module_table().modules;
    let pre_rebuild_static_deps = if native_records {
      FxHashMap::default()
    } else {
      changed_modules
        .iter()
        .filter_map(|module_idx| {
          let module = modules[*module_idx].as_normal()?;
          let deps = static_deps(module).map(|dep_idx| modules[dep_idx].id().clone());
          Some((*module_idx, deps.collect::<FxHashSet<_>>()))
        })
        .collect::<FxHashMap<_, _>>()
    };
    let pre_rebuild_interop = changed_modules
      .iter()
      .filter_map(|module_idx| {
        Some((*module_idx, importee_interop(modules[*module_idx].as_normal()?)))
      })
      .collect::<FxHashMap<_, _>>();

    // 1. Do ONE module refetch and cache merge — the update-superset walk (which
    // selects the factories to ship) runs on the post-rebuild table; boundary
    // decisions belong to the client's own walk.
    // What the link of the resident build reached, once the update rebuilds the table.
    let mut pre_rebuild_reached = None;
    let (new_added_modules, pre_rebuild_linked_side_effects) = if changed_modules.is_empty() {
      (FxIndexSet::default(), None)
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
        new_modules = %new_added_modules
          .iter()
          .map(|module_idx| module_loader_output.module_table.get(*module_idx).stable_id())
          .join(", "),
        "added modules"
      );

      // Whether linking gave each module side effects before the edit, which decided whether the
      // page's records and factories dropped an unused import of it (#697), where the rescan
      // changes what linking derives them from: a rescanned module's side effects, kind or import
      // records, or the records of a barrel that the scan resolves further.
      let pre_rebuild_linked_side_effects = (native_records
        && !rescan_only
        && (!self.cache.barrel_state.resolved_barrel_modules.is_empty()
          || rescans_side_effect_inputs(
            &self.module_table().modules,
            &module_loader_output.module_table,
          )))
      .then(|| {
        LinkedSideEffects {
          modules: &self.module_table().modules,
          kinds: Some(LinkedKinds::Captured(&pre_rebuild_exports_kinds)),
          reach: self.reach(),
          propagated: std::sync::OnceLock::new(),
        }
        .propagate()
      });

      // From here on, a failure leaves the graph with an edit that no client received.
      *self.lost_hmr_update = true;
      let plugin_driver = Arc::clone(&self.plugin_driver);
      self.cache.merge(module_loader_output.into(), &plugin_driver)?;
      pre_rebuild_reached = self.reachability.0.take();

      let options = Arc::clone(&self.options);
      self.cache.update_defer_sync_data(&options).await?;
      if rescan_only {
        return Ok(Vec::new());
      }
      (new_added_modules, pre_rebuild_linked_side_effects)
    };

    // Checked after the export shape below, so an edit's own cause (top-level await, a new
    // cell membership) is the one reported.
    // The scan is final from here on: every later render and patch routes by these verdicts.
    let cell_routes = self.native_cell_routes();
    let side_effects = self.linked_side_effects();
    let certificate_reason = if native_records {
      let (post_rebuild_route_signature, post_rebuild_route_ends) =
        route_certificate(self, &cell_routes, &pre_rebuild_resident);
      let post_rebuild_star_omissions = star_omissions(self);
      // The barrels and leaves of the certificates that a page's records or a fresh build's hold.
      // The scan keeps a barrel that an update retires until a fresh graph (#660), and a page
      // prunes its record as the update's patch applies, so the leaf of a barrel that an earlier
      // update or this one retired is no end any more (#732). The barrel itself stays one. So
      // does the leaf where a page may hold the barrel past its retirement, as one that loaded it
      // through a dependency `import.meta.hot.accept()`, which registers no edge, or through an
      // `import()` loader or a read of an `import defer` namespace that it saved before an update
      // dropped the import, or one that loaded a graph input after its first patch, which it
      // never prunes from: a factory of the barrel that an edit there re-runs would run a leaf
      // that its record omitted.
      let reach = self.reach();
      let held = self.cache.late_records
        || self
          .cache
          .get_snapshot()
          .entry_points
          .iter()
          .filter(|entry| entry.kind.is_user_defined())
          .nth(1)
          .is_some();
      let star_ends = pre_rebuild_star_omissions
        .iter()
        .map(|&(barrel, leaf, reached)| (barrel, leaf, reached && reach.reaches(barrel)))
        .chain(post_rebuild_star_omissions.iter().copied())
        .flat_map(|(barrel, leaf, live)| {
          std::iter::once(barrel).chain((live || held).then_some(leaf))
        })
        .collect::<FxHashSet<_>>();
      // An appended module's record evaluates for the first time, as an edited module's does.
      let updated = || changed_modules.iter().chain(new_added_modules.iter());
      let affected = updated().find(|module_idx| {
        pre_rebuild_route_ends.contains(*module_idx)
          || post_rebuild_route_ends.contains(*module_idx)
          || star_ends.contains(*module_idx)
      });
      // The leaves of every barrel that the scan keeps: a page may not have run one that only a
      // barrel's omitted star reaches, and a module that starts importing it needs it.
      let leaves = |omissions: &[(ModuleIdx, ModuleIdx, bool)]| {
        omissions.iter().map(|&(_, leaf, _)| leaf).collect::<FxHashSet<_>>()
      };
      let donors = leaves(&post_rebuild_star_omissions);
      let pre_rebuild_donors = leaves(&pre_rebuild_star_omissions);
      // A changed or appended module that starts or stops importing a leaf, or that imports one
      // with `import defer`, which a patch's factory would evaluate eagerly. A changed module
      // that keeps importing the same leaves has them in the page already (#625); an appended
      // module imported none.
      let leaf =
        |target: &ModuleIdx| donors.contains(target) || pre_rebuild_donors.contains(target);
      let changed_demand = || {
        updated().find_map(|module_idx| {
          let (after, deferred_after) = record_targets(self, *module_idx, true, &side_effects)?;
          let (before, deferred_before) =
            pre_rebuild_imports.get(module_idx).cloned().unwrap_or_default();
          if let Some(donor) = deferred_after
            .iter()
            .chain(&deferred_before)
            .copied()
            .filter(|target| leaf(target))
            .min()
          {
            return Some((*module_idx, donor, "defers its import of"));
          }
          let after: FxHashSet<ModuleIdx> =
            after.into_iter().filter(|target| leaf(target)).collect();
          let before: FxHashSet<ModuleIdx> =
            before.into_iter().filter(|target| leaf(target)).collect();
          let donor = after.symmetric_difference(&before).min().copied()?;
          Some((
            *module_idx,
            donor,
            if after.contains(&donor) { "starts importing" } else { "stops importing" },
          ))
        })
      };
      // A leaf that the link starts or stops wrapping, which a fresh build's barrel then
      // evaluates or omits where the resident barrel didn't (#642).
      let changed_wrapping = || {
        if donors.is_empty() && pre_rebuild_donors.is_empty() {
          return None;
        }
        let reach = self.reach();
        let wrapped = wrapped_modules(&self.module_table().modules, |idx| reach.reaches(idx));
        // A leaf that the graph reaches only before or after the edit is no leaf of a star that a
        // page ran and a fresh build runs too (#660).
        let retained = |leaf: ModuleIdx| {
          reach.reaches(leaf)
            && pre_rebuild_reached.as_ref().is_none_or(|reached| reached.reaches(leaf))
        };
        donors
          .iter()
          .chain(&pre_rebuild_donors)
          .copied()
          .filter(|leaf| {
            retained(*leaf) && wrapped.contains(leaf) != pre_rebuild_wrapped.contains(leaf)
          })
          .min()
          .map(|leaf| (leaf, wrapped.contains(&leaf)))
      };
      let reason = if let Some(module_idx) = affected {
        Some(format!(
          "native evaluation omission certificate affected by {}",
          self.module_table().modules[*module_idx].stable_id()
        ))
      } else if let Some((module_idx, donor, demand)) = changed_demand() {
        Some(format!(
          "native evaluation omission certificate affected by {}, which {demand} {}",
          self.module_table().modules[module_idx].stable_id(),
          self.module_table().modules[donor].stable_id()
        ))
      } else if let Some((leaf, wrapped)) = changed_wrapping() {
        Some(format!(
          "native evaluation omission certificate affected by {}, which the link {} wraps",
          self.module_table().modules[leaf].stable_id(),
          if wrapped { "now" } else { "no longer" }
        ))
      } else if pre_rebuild_route_signature != post_rebuild_route_signature {
        Some(format!(
          "native evaluation omission certificate changed by {}",
          changed_modules
            .iter()
            .map(|module_idx| self.module_table().modules[*module_idx].stable_id().to_string())
            .collect::<Vec<_>>()
            .join(", ")
        ))
      } else {
        None
      };
      reason
    } else {
      None
    };

    if (!pre_rebuild_json_namespace_shapes.is_empty()
      || new_added_modules.iter().any(|&idx| {
        self
          .module_table()
          .modules
          .get(idx)
          .and_then(Module::as_normal)
          .is_some_and(crate::utils::import_options::is_json_module)
      }))
      && let Some(module_idx) =
        json_namespace_shapes(self).into_iter().find_map(|(module_idx, has_properties)| {
          pre_rebuild_json_namespace_shapes
            .get(&module_idx)
            .is_some_and(|before| *before != has_properties)
            .then_some(module_idx)
        })
    {
      let reason = format!(
        "the JSON namespace of {} gains or loses its properties; its namespace cannot be patched",
        self.module_table().modules[module_idx].stable_id()
      );
      return Ok(
        clients
          .iter()
          .map(|client| ClientHmrUpdate {
            client_id: client.client_id.to_string(),
            update: HmrUpdate::FullReload { reason: reason.clone() },
          })
          .collect(),
      );
    }

    if let Some((module_idx, before, after)) =
      pre_rebuild_exports_kinds.iter().find_map(|(module_idx, before)| {
        let module = self.module_table().modules.get(*module_idx)?.as_normal()?;
        let after = self.linked_exports_kind(module)?;
        (*before != after).then_some((*module_idx, *before, after))
      })
    {
      let kind = |kind: ExportsKind| match kind {
        ExportsKind::Esm => "an ES module",
        ExportsKind::CommonJs => "CommonJS",
        ExportsKind::None => "a module without exports",
      };
      let reason = format!(
        "the link makes {} {} instead of {}; its importers' bindings cannot be patched",
        self.module_table().modules[module_idx].stable_id(),
        kind(after),
        kind(before)
      );
      return Ok(
        clients
          .iter()
          .map(|client| ClientHmrUpdate {
            client_id: client.client_id.to_string(),
            update: HmrUpdate::FullReload { reason: reason.clone() },
          })
          .collect(),
      );
    }

    // The importers whose factories now drop an unused import that a page's records and factories
    // kept, which the update re-runs as changed modules (`flipped_unused_imports`).
    let dropping_importers = match &pre_rebuild_linked_side_effects {
      Some(before) => match self.flipped_unused_imports(before, &side_effects, &changed_modules) {
        Ok(importers) => importers,
        Err(reason) => {
          return Ok(
            clients
              .iter()
              .map(|client| ClientHmrUpdate {
                client_id: client.client_id.to_string(),
                update: HmrUpdate::FullReload { reason: reason.clone() },
              })
              .collect(),
          );
        }
      },
      None => FxIndexSet::default(),
    };

    if let Some((stable_id, ..)) = pre_rebuild_export_shapes
      .iter()
      .map(|(module_idx, shape)| (shape, native_export_shape(self, *module_idx)))
      .find_map(|(before, after)| (Some(before) != after.as_ref()).then_some(before))
    {
      // A native cell's exports follow its donor's eligibility: report the donor's own cause.
      let reason = if changed_modules.iter().any(|idx| {
        self.module_table().modules[*idx].as_normal().is_some_and(|module| {
          module.ast_usage.contains(rolldown_common::EcmaModuleAstUsage::TopLevelAwait)
        })
      }) {
        "top-level await requires a full reload; asynchronous patch factories are unsupported"
          .to_owned()
      } else if crate::utils::native_records::is_native_cell(stable_id) {
        format!("native cell membership changed ({stable_id}); its namespace cannot be patched")
      } else {
        format!(
          "native module export shape changed in {stable_id}; its namespace cannot be patched"
        )
      };
      return Ok(
        clients
          .iter()
          .map(|client| ClientHmrUpdate {
            client_id: client.client_id.to_string(),
            update: HmrUpdate::FullReload { reason: reason.clone() },
          })
          .collect(),
      );
    }

    let newly_assigned = pre_rebuild_assigned_exports.iter().find_map(|(module_idx, before)| {
      let mut added = assigned_exports(self, *module_idx)?
        .into_iter()
        .filter(|name| !before.contains(name))
        .collect::<Vec<_>>();
      added.sort_unstable();
      let name = added.into_iter().next()?;
      Some(format!(
        "{} reassigns its export {name}; a native namespace cannot follow a later generation's writes",
        self.module_table().modules[*module_idx].stable_id()
      ))
    });
    if let Some(reason) = certificate_reason.or(newly_assigned) {
      return Ok(
        clients
          .iter()
          .map(|client| ClientHmrUpdate {
            client_id: client.client_id.to_string(),
            update: HmrUpdate::FullReload { reason: reason.clone() },
          })
          .collect(),
      );
    }

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
    let recheck_results = recheck_inputs
      .into_par_iter()
      .map(|render_input| {
        let module_idx = render_input.idx;
        let (code, _) =
          self.render_module_code(render_input, 0, 0, false, &cell_routes, &side_effects);
        let unchanged = pre_rebuild_renders[&module_idx] == code;
        (module_idx, unchanged, (!unchanged).then_some(code))
      })
      .collect::<Vec<_>>();
    let mut output_unchanged_modules = FxHashSet::default();
    let mut post_rebuild_renders = FxHashMap::default();
    for (module_idx, unchanged, code) in recheck_results {
      if unchanged {
        output_unchanged_modules.insert(module_idx);
      } else if let Some(code) = code {
        post_rebuild_renders.insert(module_idx, code);
      }
    }
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
    changed_modules.extend(dropping_importers);

    // An importer's factory reads its importees' interop facts, so it changes with them even
    // when its own source did not. Stamp and carry such importers like changed modules: a
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
    // Import records, not `importers_idx`: that set leaves out lazy-compilation proxies, whose
    // `import()` of the real module also reads its interop facts.
    let rerendered_importers = if interop_changed.is_empty() {
      FxIndexSet::default()
    } else {
      modules
        .iter()
        .filter_map(Module::as_normal)
        .filter(|module| !changed_modules.contains(&module.idx))
        .filter(|module| {
          module
            .import_records
            .iter()
            .any(|rec| rec.resolved_module.is_some_and(|dep| interop_changed.contains(&dep)))
        })
        .map(|module| module.idx)
        .collect::<FxIndexSet<_>>()
    };

    // 2. Stamp the rebuild: `latest[m] = rebuild_seq` for every changed, newly added or
    // re-rendered module — the versioned ship map's staleness source.
    let rebuild_seq = stamp_table.begin_rebuild();
    for module_idx in
      changed_modules.iter().chain(new_added_modules.iter()).chain(rerendered_importers.iter())
    {
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

    // A re-run importer needs a factory for each import the rebuild added.
    let added_static_deps = if native_records {
      FxIndexSet::default()
    } else {
      let modules = &self.module_table().modules;
      changed_modules
        .iter()
        .chain(new_added_modules.iter())
        .filter_map(|module_idx| modules[*module_idx].as_normal())
        .flat_map(|module| {
          let before = pre_rebuild_static_deps.get(&module.idx);
          static_deps(module)
            .filter(move |dep| before.is_none_or(|before| !before.contains(modules[*dep].id())))
        })
        .collect::<FxIndexSet<_>>()
    };

    let mut affected = self.collect_client_update_superset(&changed_modules);
    affected.extend(new_added_modules.iter().copied());
    affected.extend(rerendered_importers.iter().copied());
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
    // What the patches' factories read of CommonJS modules, which a later scan of the update can
    // decide otherwise (`changed_commonjs_read`).
    let mut commonjs_reads = Vec::new();
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

      if !added_static_deps.is_empty() {
        let held = ClientHeldCopies {
          shipped: client.shipped,
          evaluated: client.top_level_evaluated,
          stamp_table,
        };
        self.collect_unheld_sync_deps(
          &[],
          added_static_deps.iter().copied(),
          None,
          &held,
          &mut carried,
        );
      }

      let reads = self.commonjs_reads(carried.iter().copied(), &side_effects);
      let update = self
        .render_hmr_patch(
          carried,
          changed_ids.clone(),
          stamp_table,
          &cell_routes,
          &side_effects,
          &post_rebuild_renders,
        )
        .await?;
      if matches!(update, HmrUpdate::Patch(_)) {
        commonjs_reads.extend(reads);
      }
      client_updates.push(ClientHmrUpdate { client_id: client.client_id.to_string(), update });
    }
    self.cache.commonjs_reads.extend(commonjs_reads);

    *self.lost_hmr_update = lost_before;
    Ok(client_updates)
  }

  /// Collect the superset of modules any client's walk may re-run for these changes:
  /// pure reachability over the importer graph (static ∪ dynamic edges), stopping at
  /// statically self-accepting modules and at accepting importer edges.
  ///
  /// The scan keeps an importer that the graph no longer reaches until a fresh graph (#660). A
  /// page prunes its record once the update that retires it applies, unless the page loaded it
  /// through an edge that the page doesn't register, a dependency `import.meta.hot.accept()`'s,
  /// so the walk still follows such an importer, but not one with top-level await, whose factory
  /// no patch can run: a page that pruned it never runs it again, and one that still holds it
  /// lacks its factory and reloads, as the whole update did before (#728).
  fn collect_client_update_superset(
    &self,
    changed_modules: &FxIndexSet<ModuleIdx>,
  ) -> FxIndexSet<ModuleIdx> {
    let reach = self.reach();
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
        if !reach.reaches(importer_idx)
          && importer.ast_usage.contains(rolldown_common::EcmaModuleAstUsage::TopLevelAwait)
        {
          continue;
        }
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
  /// ship map only. The ship map itself is written only by the delivery notification
  /// (`DevEngine::notify_payload_delivered`).
  ///
  /// A resident factory stands for its closure, except one that `rerun` names: an HMR
  /// patch's factory of a module whose imports the client may lack, such as a module of a
  /// lazy route that it hasn't opened, or one that the patch changes, which may import a
  /// module that the client never ran. Selection walks through such a factory to its imports,
  /// without shipping it again, the entry's too, whose dynamic imports it then leaves to their
  /// own compiles.
  /// It follows what each factory evaluates: for a record that the full build bound straight
  /// to native cells (`native_cell_routes::Verdict::Bypass`), the cells and literal records of
  /// the bindings that the module reads, never the donor, and nothing for an unused import of a
  /// module without side effects, which the factory drops (`hmr::utils::drops_import`, where
  /// the module's scope tree decides it).
  ///
  /// `evaluated` is what every client that may run a shipped factory evaluated. A client that
  /// runs the entry's own factory may have evaluated more, `entry_evaluated`, which selection
  /// subtracts from the entry's imports alone: a factory that it ships for them may run on any
  /// client.
  ///
  /// `partly_run`, where given, names what the records of some of the clients' inputs evaluated,
  /// or may evaluate later through a `require()`, an `import defer` namespace or an undecided
  /// route, which other clients may not have run. A module of it that a factory that selection
  /// ships or walks through imports, where the clients that run the factory may lack it, ships
  /// again, though it is resident, already selected or the entry, and the caller reads it as
  /// the inputs' disagreement.
  ///
  /// `eager`, where given, receives, by stable id, what a factory that selection ships or walks
  /// through, the entry's own included, imports with `import defer` where the clients that run
  /// the factory may not have evaluated it: the factory runs it at once, where their records run
  /// it only once the importer reads its namespace.
  ///
  /// `routed`, where given, receives, by stable id, each import that such a factory makes, as
  /// the factory routes it, of a module that the clients that run the factory may lack: those
  /// that selection goes on to, and those of a resident factory, which stands for its closure, so
  /// that the caller can tell which factories may run where the entry's factory runs, also
  /// across the compiles that share a factory.
  #[expect(clippy::too_many_arguments)]
  pub async fn compile_lazy_entry(
    &mut self,
    module_id: &str,
    _client_id: &str,
    shipped: &FxHashMap<ArcStr, u32>,
    rerun: &FxHashSet<ArcStr>,
    evaluated: &FxHashMap<ArcStr, u32>,
    entry_evaluated: Option<&FxHashMap<ArcStr, u32>>,
    partly_run: Option<&FxHashSet<ArcStr>>,
    eager: Option<&mut FxHashSet<ArcStr>>,
    routed: Option<&mut Vec<(ArcStr, ArcStr)>>,
    stamp_table: &HmrStampTable,
  ) -> BuildResult<HmrLazyChunkOutput> {
    tracing::debug!(
      target: "hmr",
      "compile_lazy_entry: module_id: {}",
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

    // 2. Trigger a partial scan to fetch the module and its dependencies when compiling a lazy
    // proxy (`?rolldown-lazy=1`) or non-native request; in native module HMR (`client_id == "native"`),
    // `compute_hmr_update_for_file_changes` has already scanned and merged all changed files.
    if _client_id != "native" || module_id.contains("rolldown-lazy") {
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

      // Native module HMR compiles a proxy on every patch that ships its target, and the proxy
      // loads the same source until a page activates it. Merging an unchanged proxy would mark
      // it rescanned, so that the next relink would link the whole graph.
      if _client_id != "native"
        || !refetched_unchanged(self.module_table(), entry_module_idx, &module_loader_output)
      {
        let plugin_driver = Arc::clone(&self.plugin_driver);
        self
          .cache
          .merge(module_loader_output.into(), &plugin_driver)
          .map_err(|e| vec![anyhow::anyhow!(e).into()])?;
        self.reachability = LazyReachability::default();

        let options = Arc::clone(&self.options);
        self.cache.update_defer_sync_data(&options).await?;
      }
    }

    // Collect all sync dependencies, stopping at modules whose current copy this client
    // already holds — factory resident per the ship map, or exports live per the
    // top-level-evaluated map. Overlapping concurrent lazy compiles both see an
    // unmarked ship map and re-ship shared factories — duplicate idempotent bytes, never
    // a missing factory. The walk routes imports as the chunk's factories do.
    let cell_routes = self.native_cell_routes();
    let side_effects = self.linked_side_effects();
    let mut modules_to_be_updated = FxIndexSet::default();
    let mut dropped = FxHashSet::default();
    let mut edges = routed.is_some().then(Vec::new);
    self.collect_sync_dependencies_for_client(
      entry_module_idx,
      &mut modules_to_be_updated,
      shipped,
      rerun,
      evaluated,
      entry_evaluated,
      partly_run,
      eager,
      edges.as_mut(),
      stamp_table,
      &cell_routes,
      &side_effects,
      &mut dropped,
    );
    if let (Some(routed), Some(edges)) = (routed, edges) {
      let modules = &self.module_table().modules;
      routed.extend(edges.into_iter().filter_map(|(importer, dep)| {
        let module = &modules[dep];
        let dep = module.as_normal()?;
        (module.id().as_str() != rolldown_common::RUNTIME_MODULE_KEY).then(|| {
          (modules[importer].stable_id().as_arc_str().clone(), dep.stable_id.as_arc_str().clone())
        })
      }));
    }

    // The compiler runtime is installed by the bootstrap, not by module patches.
    // A newly added helper dependency must reuse that instance and its state.
    modules_to_be_updated.retain(|idx| {
      let module = &self.module_table().modules[*idx];
      module.is_normal() && module.id().as_str() != rolldown_common::RUNTIME_MODULE_KEY
    });

    // Sort for stable output
    modules_to_be_updated
      .sort_by_cached_key(|module_idx| self.module_table().modules[*module_idx].id());

    let mut key_gens: Vec<(ModuleIdx, u64)> =
      Vec::with_capacity(modules_to_be_updated.len() * 2);
    for &idx in &modules_to_be_updated {
      key_gens.push((idx, self.cache.module_scan_generation(idx)));
      if let Some(normal) = self.module_table().modules[idx].as_normal() {
        for (rec_idx, rec) in normal.import_records.iter_enumerated() {
          if let Some(dep_idx) = rec.resolved_module {
            // Whether the factory drops the import, which the side effects of modules further
            // down decide (`LinkedSideEffects`).
            let generation = self.cache.module_scan_generation(dep_idx);
            let dropped = u64::from(dropped.contains(&(idx, rec_idx)));
            key_gens.push((dep_idx, generation << 1 | dropped));
          }
        }
      }
    }
    // The factories read CommonJS modules as the link of the current scan converts them, which
    // other importers decide, so a cached chunk serves only where it still reads them so (#748).
    let commonjs_reads = self.commonjs_reads(modules_to_be_updated.iter().copied(), &side_effects);
    let cached = if self.cache.native_cell_modules.is_empty()
      && let Some((cached_gens, cached_reads, code, filename, sourcemap_filename, sourcemap)) =
        self.cache.cached_lazy_chunks.get(module_id)
      && *cached_gens == key_gens
      && *cached_reads == commonjs_reads
    {
      let carried = modules_to_be_updated
        .iter()
        .map(|module_idx| {
          let stable_id = self.module_table().modules[*module_idx].stable_id();
          (stable_id.as_arc_str().clone(), stamp_table.render_time_stamp(stable_id.as_str()))
        })
        .collect();
      Some(HmrLazyChunkOutput {
        code: code.clone(),
        filename: filename.clone(),
        sourcemap_filename: sourcemap_filename.clone(),
        sourcemap: sourcemap.clone(),
        carried,
      })
    } else {
      None
    };
    if let Some(output) = cached {
      self.cache.commonjs_reads.extend(commonjs_reads);
      return Ok(output);
    }

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
      &self.cache.get_snapshot().user_defined_entry_modules,
      Some(stamp_table),
    ) {
      source_joiner.append_source(prelude);
    }
    let rendered_sources = module_render_inputs
      .into_par_iter()
      .enumerate()
      .flat_map(|(index, render_input)| {
        let affected_module_idx = render_input.idx;
        let stamp = stamp_table
          .render_time_stamp(self.module_table().modules[affected_module_idx].stable_id().as_str());
        let (code, map) =
          self.render_module_code(render_input, index, stamp, true, &cell_routes, &side_effects);

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

    // In native module HMR (`_client_id == "native"`), lazy chunks are registrations only —
    // `requestLazy` runs the module on demand. For standard Rolldown lazy compilation, the
    // proxy chunk executes `initModule` at the end.
    if _client_id != "native" {
      let entry_stable_id = self.module_table().modules[entry_module_idx].stable_id().as_str();
      source_joiner.append_source(format!(
        "__rolldown_runtime__.initModule({})",
        json_escape_simd::escape(entry_stable_id)
      ));
    }

    let (mut code, mut map) = source_joiner.join();

    let lazy_patch_id = self.next_hmr_patch_id.fetch_add(1, Ordering::Relaxed);
    let filename = format!("lazy_compile_{lazy_patch_id}.js");

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

    let carried = modules_to_be_updated
      .iter()
      .map(|module_idx| {
        let stable_id = self.module_table().modules[*module_idx].stable_id();
        (stable_id.as_arc_str().clone(), stamp_table.render_time_stamp(stable_id.as_str()))
      })
      .collect();

    let sourcemap_filename = sourcemap_asset.as_ref().map(|asset| asset.filename.to_string());
    let sourcemap = sourcemap_asset.map(|asset| asset.source.try_into_string()).transpose()?;
    if self.cache.native_cell_modules.is_empty() {
      self.cache.cached_lazy_chunks.insert(
        module_id.to_owned(),
        (
          key_gens,
          commonjs_reads.clone(),
          code.clone(),
          filename.clone(),
          sourcemap_filename.clone(),
          sourcemap.clone(),
        ),
      );
    }
    self.cache.commonjs_reads.extend(commonjs_reads);
    Ok(HmrLazyChunkOutput {
      code,
      filename,
      sourcemap_filename,
      sourcemap,
      carried,
    })
  }

  async fn render_hmr_patch(
    &self,
    mut carried_modules: FxIndexSet<ModuleIdx>,
    changed_ids: Vec<String>,
    stamp_table: &HmrStampTable,
    cell_routes: &[crate::hmr::native_cell_routes::Route],
    side_effects: &LinkedSideEffects<'_>,
    post_rebuild_renders: &FxHashMap<ModuleIdx, String>,
  ) -> BuildResult<HmrUpdate> {
    // Note: the carried set might include external modules. There's no way to "update" them, so we need to remove them.
    carried_modules.retain(|idx| self.module_table().modules[*idx].is_normal());

    // Nothing to ship and nothing for the client's walk to re-run — every changed
    // module rendered byte-identical (dropped upstream) and the stale sweep
    // carried nothing. Say so explicitly instead of sending an empty patch.
    if carried_modules.is_empty() && changed_ids.is_empty() {
      return Ok(HmrUpdate::Noop);
    }

    // The patch runtime executes synchronous factories. Do not emit an `await`
    // inside one, or report an asynchronous factory as applied before completion.
    if carried_modules.iter().any(|idx| {
      self.module_table().modules[*idx].as_normal().is_some_and(|module| {
        module.ast_usage.contains(rolldown_common::EcmaModuleAstUsage::TopLevelAwait)
      })
    }) {
      return Ok(HmrUpdate::FullReload {
        reason:
          "top-level await requires a full reload; asynchronous patch factories are unsupported"
            .into(),
      });
    }

    // Sorting `carried_modules` is not strictly necessary, but it:
    // - Makes the snapshot more stable when we change logic that affects the order of modules.
    carried_modules
      .sort_by_cached_key(|module_idx| self.module_table().modules[*module_idx].id().as_str());

    let index_ecma_ast = self.index_ecma_ast();
    let sourcemap_disabled = self.options.sourcemap.is_none();
    let module_render_inputs = carried_modules
      .iter()
      .copied()
      .enumerate()
      .map(|(index, affected_module_idx)| {
        let affected_module = &self.module_table().modules[affected_module_idx];
        let Module::Normal(affected_module) = affected_module else {
          unreachable!("HMR only supports normal module");
        };

        debug_assert_eq!(affected_module_idx, affected_module.idx);
        let stamp = stamp_table
          .render_time_stamp(affected_module.stable_id.as_str());
        if index == 0
          && stamp == 0
          && sourcemap_disabled
          && let Some(code) = post_rebuild_renders.get(&affected_module_idx)
        {
          return (affected_module.idx, Ok(code.clone()));
        }
        let ecma_ast =
          index_ecma_ast[affected_module_idx].as_ref().expect("Normal module should have an AST");

        (
          affected_module.idx,
          Err(ModuleRenderInput {
            idx: affected_module.idx,
            ecma_ast: ecma_ast.clone_with_another_arena(),
          }),
        )
      })
      .collect::<Vec<_>>();

    let mut source_joiner = SourceJoiner::default();
    // The graph-rows manifest is the first source of every payload: pure topology the
    // client-side walk consumes, landing before any factory registers.
    if let Some(prelude) = crate::hmr::module_graph_delta::render_register_graph_source(
      self.module_table(),
      carried_modules.iter().copied(),
      &self.cache.get_snapshot().user_defined_entry_modules,
      Some(stamp_table),
    ) {
      source_joiner.append_source(prelude);
    }
    let rendered_sources = module_render_inputs
      .into_par_iter()
      .enumerate()
      .flat_map(|(index, (affected_module_idx, cached_or_input))| {
        let (code, map) = match cached_or_input {
          Ok(code) => (code, None),
          Err(render_input) => {
            let stamp = stamp_table.render_time_stamp(
              self.module_table().modules[affected_module_idx].stable_id().as_str(),
            );
            self.render_module_code(render_input, index, stamp, true, cell_routes, side_effects)
          }
        };

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

  /// The kind that the link gives `module`, which its importers' records bind to
  /// ([`linked_exports_kind`]), by its importers that the graph reaches (#660).
  fn linked_exports_kind(&self, module: &rolldown_common::NormalModule) -> Option<ExportsKind> {
    let reach = self.reach();
    linked_exports_kind(
      module,
      &self.cache.importers,
      !self.options.code_splitting.is_disabled(),
      reach,
    )
  }

  /// Whether a link of the current module table reaches a module.
  fn reach(&self) -> Reach<'_> {
    Reach { reachability: &self.reachability, snapshot: self.cache.get_snapshot() }
  }

  /// Whether each module of the scan's table has side effects once linked
  /// ([`LinkedSideEffects`]).
  fn linked_side_effects(&self) -> LinkedSideEffects<'_> {
    LinkedSideEffects {
      modules: &self.module_table().modules,
      kinds: crate::utils::native_records::enabled(&self.options).then_some(
        LinkedKinds::Importers {
          importers: &self.cache.importers,
          code_splitting: !self.options.code_splitting.is_disabled(),
        },
      ),
      reach: self.reach(),
      propagated: std::sync::OnceLock::new(),
    }
  }

  /// The kind that a patch or a lazy chunk registers `module` as, which its factory follows:
  /// the link's for a module without ES or CommonJS syntax, as the page's records register it
  /// (#640), and the scan's for any other module, or for a lazy export, whose form the link
  /// stage writes.
  fn rendered_exports_kind(&self, module: &rolldown_common::NormalModule) -> ExportsKind {
    if module.exports_kind == ExportsKind::None && !module.meta.has_lazy_export() {
      self.linked_exports_kind(module).unwrap_or(ExportsKind::None)
    } else {
      module.exports_kind
    }
  }

  /// The modules that `module` imports that its scan found no ES or CommonJS syntax in and that
  /// the link makes CommonJS (#640), which the page's records bind through their interop.
  fn linked_commonjs(&self, module: &rolldown_common::NormalModule) -> FxHashSet<ModuleIdx> {
    let modules = &self.module_table().modules;
    module
      .import_records
      .iter()
      .filter_map(|record| modules[record.resolved_module?].as_normal())
      .filter(|importee| {
        importee.exports_kind == ExportsKind::None
          && self.linked_exports_kind(importee) == Some(ExportsKind::CommonJs)
      })
      .map(|importee| importee.idx)
      .collect()
  }

  /// The CommonJS modules that `module` imports, not with `export *`, sorted, each with whether
  /// the link's records read its exports through `__toESM` (#748): unless no import reads the
  /// default or the namespace, by each importer that the graph reaches, as
  /// `LinkStage::determine_safely_merge_cjs_ns` decides for every record, or, under strict
  /// execution order, where each record decides, by `module`'s own imports, whose patch binds the
  /// module once. `linked_commonjs` names those without ES or CommonJS syntax that the link makes
  /// CommonJS.
  fn commonjs_conversions(
    &self,
    module: &rolldown_common::NormalModule,
    linked_commonjs: &FxHashSet<ModuleIdx>,
  ) -> Vec<(ModuleIdx, bool)> {
    let modules = &self.module_table().modules;
    let reach = self.reach();
    let static_import = |record: &rolldown_common::ResolvedImportRecord| {
      record.kind == ImportKind::Import && !record.meta.contains(ImportRecordMeta::IsExportStar)
    };
    let converts = |importer: &rolldown_common::NormalModule, importee: ModuleIdx| {
      importer.import_records.iter_enumerated().any(|(record_idx, record)| {
        record.resolved_module == Some(importee)
          && static_import(record)
          && crate::utils::external_import_interop::import_record_needs_interop(
            importer, record_idx,
          )
      })
    };
    let strict = self.options.is_strict_execution_order_enabled();
    let mut conversions = module
      .import_records
      .iter()
      .filter(|record| static_import(record))
      .filter_map(|record| record.resolved_module)
      .filter(|&importee| {
        modules[importee].as_normal().is_some_and(|importee| {
          importee.exports_kind == ExportsKind::CommonJs || linked_commonjs.contains(&importee.idx)
        })
      })
      .map(|importee| {
        let converted = converts(module, importee)
          || (!strict
            && self.cache.importers.get(importee).is_some_and(|importers| {
              importers.iter().filter(|importer| reach.reaches(importer.importer_idx)).any(
                |importer| {
                  modules[importer.importer_idx]
                    .as_normal()
                    .is_some_and(|importer| converts(importer, importee))
                },
              )
            }));
        (importee, converted)
      })
      .collect::<Vec<_>>();
    conversions.sort_unstable();
    conversions.dedup();
    conversions
  }

  /// How the factories of `modules` read the CommonJS modules that they import, by importer and
  /// importee: whether through `__toESM` ([`Self::commonjs_conversions`]). A factory reads nothing
  /// of a module whose every import it drops as unused, which `side_effects` decides
  /// (`hmr::utils::drops_import`), or else, without a scope tree to decide it, may read it.
  fn commonjs_reads(
    &self,
    modules: impl IntoIterator<Item = ModuleIdx>,
    side_effects: &LinkedSideEffects<'_>,
  ) -> Vec<(ModuleIdx, ModuleIdx, bool)> {
    let symbols = &self.cache.get_snapshot().symbol_ref_db;
    modules
      .into_iter()
      .filter_map(|idx| self.module_table().modules[idx].as_normal())
      .flat_map(|module| {
        let scoping = crate::hmr::utils::import_scoping(
          symbols,
          self.cache.cached_native_relink.as_deref(),
          module,
          false,
        );
        let read = module
          .import_records
          .iter_enumerated()
          .filter(|(_, record)| {
            record.kind == ImportKind::Import
              && !record.meta.contains(ImportRecordMeta::IsExportStar)
          })
          .filter(|&(record_idx, _)| {
            !scoping.is_some_and(|scoping| {
              crate::hmr::utils::drops_import(
                module,
                record_idx,
                |idx| side_effects.pure(idx),
                Some(scoping),
              )
            })
          })
          .filter_map(|(_, record)| record.resolved_module)
          .collect::<FxHashSet<_>>();
        self
          .commonjs_conversions(module, &self.linked_commonjs(module))
          .into_iter()
          .filter(move |(importee, _)| read.contains(importee))
          .map(|(importee, converted)| (module.idx, importee, converted))
      })
      .collect()
  }

  /// The first CommonJS module, by stable id, that a factory of `ScanStageCache::commonjs_reads`
  /// reads otherwise than the link of the current scan decides ([`Self::commonjs_conversions`]):
  /// a lazy compile's partial scan can add an importer that reads the module's default, after
  /// which the link converts its exports for every importer, also for a factory that the update
  /// rendered before that scan (#748).
  pub fn changed_commonjs_read(&self) -> Option<ArcStr> {
    let modules = &self.module_table().modules;
    let mut reads = self.cache.commonjs_reads.iter().copied().collect::<Vec<_>>();
    reads.sort_unstable();
    let mut current = FxHashMap::default();
    reads.into_iter().find_map(|(importer, importee, converted)| {
      let now = current.entry(importer).or_insert_with(|| {
        modules
          .get(importer)
          .and_then(Module::as_normal)
          .map(|module| self.commonjs_conversions(module, &self.linked_commonjs(module)))
          .unwrap_or_default()
      });
      (!now.contains(&(importee, converted))).then(|| {
        modules
          .get(importee)
          .map_or_else(|| ArcStr::from("an unknown module"), |module| {
            module.stable_id().as_arc_str().clone()
          })
      })
    })
  }

  /// The imports of each of these modules (by stable id) that a factory of the module may run,
  /// as `(importer, target, kind, deferred, kept)`: the importer's module id, the target's stable
  /// id, the record's kind, a static import, a `require` or an `import()`, whether a static import
  /// is an `import defer` (`hmr::utils::deferred_records`), whose target a record evaluates once
  /// its module reads the namespace but a factory of the module at once, and whether a factory of
  /// the module keeps a static import, which it drops as an unused import of a module without side
  /// effects once linked, where the module's scope tree decides it, as the walk of a lazy compile
  /// does (`collect_sync_dependencies_for_client`). A `require` or an `import()` counts as kept.
  /// A factory binds the native cells and literal records of the bindings that it reads in place
  /// of the donor that a static import's bypass route passes (`native_cell_routes::bypass_bindings`),
  /// which the result names as kept targets of the import, and the donor as one that it drops.
  pub fn static_import_phases<'b>(
    &self,
    stable_ids: impl IntoIterator<Item = &'b str>,
  ) -> Vec<(ArcStr, ArcStr, ImportKind, bool, bool)> {
    let side_effects = self.linked_side_effects();
    let routes = self.native_cell_routes();
    let modules = &self.module_table().modules;
    let mut imports = Vec::new();
    for stable_id in stable_ids {
      let Some(module) =
        self.cache.module_idx_by_stable_id.get(stable_id).and_then(|idx| modules[*idx].as_normal())
      else {
        continue;
      };
      let scoping = crate::hmr::utils::import_scoping(
        &self.cache.get_snapshot().symbol_ref_db,
        self.cache.cached_native_relink.as_deref(),
        module,
        false,
      );
      let deferred =
        self.index_ecma_ast()[module.idx].as_ref().map_or_else(FxHashSet::default, |ast| {
          crate::hmr::utils::deferred_records(module, ast.program())
        });
      let bypassed = crate::hmr::native_cell_routes::bypass_evaluations(&routes, module.idx);
      for (record_idx, record) in module.import_records.iter_enumerated() {
        if !matches!(
          record.kind,
          ImportKind::Import | ImportKind::Require | ImportKind::DynamicImport
        ) {
          continue;
        }
        let Some(target) = record.resolved_module else {
          continue;
        };
        // JSON's and text's loader-owned classification markers are no executable dependency
        // (`collect_sync_dependencies_for_client`).
        if matches!(record.module_request.as_str(), "\0rolldown/json-module" | "\0rolldown/text-module")
          && modules[target].id().as_str() == rolldown_common::RUNTIME_MODULE_KEY
        {
          continue;
        }
        let deferred_import = deferred.contains(&record_idx);
        let owners = bypassed.get(&record_idx).filter(|_| !deferred_import);
        let kept = owners.is_none()
          && (record.kind != ImportKind::Import
            || !scoping.is_some_and(|scoping| {
              crate::hmr::utils::drops_import(
                module,
                record_idx,
                |idx| side_effects.pure(idx),
                Some(scoping),
              )
            }));
        imports.push((
          module.id.as_arc_str().clone(),
          modules[target].stable_id().as_arc_str().clone(),
          record.kind,
          deferred_import,
          kept,
        ));
        for owner in owners.into_iter().flat_map(|owners| owners.iter()) {
          imports.push((
            module.id.as_arc_str().clone(),
            modules[*owner].stable_id().as_arc_str().clone(),
            record.kind,
            false,
            true,
          ));
        }
      }
    }
    imports
  }

  /// Native-cell bypass verdicts on the current scan (`native_cell_routes`); empty without
  /// native records.
  fn native_cell_routes(&self) -> Vec<crate::hmr::native_cell_routes::Route> {
    if !crate::utils::native_records::enabled(&self.options)
      || self.cache.native_cell_modules.is_empty()
    {
      return Vec::new();
    }
    let snapshot = self.cache.get_snapshot();
    crate::hmr::native_cell_routes::routes(
      &snapshot.module_table.modules,
      &snapshot.stmt_infos,
      &snapshot.index_ecma_ast,
      &snapshot.user_defined_entry_modules,
    )
  }

  /// What every page ran along `routes`, by the scan's evaluation of every user-defined entry
  /// (`native_cell_routes::resident`); empty without native cells.
  fn native_cell_resident(
    &self,
    routes: &[crate::hmr::native_cell_routes::Route],
  ) -> FxHashSet<ModuleIdx> {
    if !crate::utils::native_records::enabled(&self.options)
      || self.cache.native_cell_modules.is_empty()
    {
      return FxHashSet::default();
    }
    let snapshot = self.cache.get_snapshot();
    crate::hmr::native_cell_routes::resident(
      &snapshot.module_table.modules,
      &snapshot.stmt_infos,
      &snapshot.index_ecma_ast,
      &snapshot.user_defined_entry_modules,
      routes,
    )
  }

  /// Finalize and print one module into its HMR payload form (factory-registration
  /// snippet, without the `//#region` framing).
  ///
  /// `unique_index` seeds the payload-position-dependent binding suffixes, so two
  /// renders of the same module compare equal only when they pin it to the same
  /// value, and likewise the same `stamp`. `with_sourcemap: false` skips sourcemap
  /// generation even when the options ask for one.
  fn render_module_code(
    &self,
    render_input: ModuleRenderInput,
    unique_index: usize,
    stamp: u32,
    with_sourcemap: bool,
    cell_routes: &[crate::hmr::native_cell_routes::Route],
    side_effects: &LinkedSideEffects<'_>,
  ) -> (String, Option<SourceMap>) {
    let ModuleRenderInput { idx: module_idx, ecma_ast: mut ast } = render_input;

    let Module::Normal(module) = &self.module_table().modules[module_idx] else {
      unreachable!("HMR only supports normal module");
    };

    let enable_sourcemap = with_sourcemap
      && self.options.sourcemap.is_some()
      && (!module.is_virtual() || crate::utils::native_records::is_native_cell(&module.id));
    let use_pife_for_module_wrappers =
      self.options.optimization.is_pife_for_module_wrappers_enabled();
    let modules = &self.module_table().modules;
    // The page's records bind a module that the link makes CommonJS through its interop, also
    // when its scan found no CommonJS syntax, and so must this module's (#640).
    let exports_kind = self.rendered_exports_kind(module);
    let linked_commonjs = self.linked_commonjs(module);
    let unconverted_commonjs = self
      .commonjs_conversions(module, &linked_commonjs)
      .into_iter()
      .filter_map(|(importee, converted)| (!converted).then_some(importee))
      .collect();
    let pure = |idx| side_effects.pure(idx);
    let reach = self.reach();
    let json_namespace_properties = crate::utils::import_options::is_json_module(module)
      && crate::utils::import_options::json_namespace_has_properties(module, modules, |idx| {
        reach.reaches(idx)
      });

    ast.program.with_mut(|fields| {
      if module.module_type == rolldown_common::ModuleType::Json
        && module.meta.has_lazy_export()
      {
        use oxc::allocator::ReplaceWith;
        use rolldown_ecmascript_utils::StatementFactoryExt as _;

        let builder = AstBuilder::new(fields.allocator);
        let statement = fields.program.body.first_mut().expect("JSON has one expression");
        statement.replace_with(|statement| {
          let oxc::ast::ast::Statement::ExpressionStatement(expression) = statement else {
            unreachable!("unlinked JSON is an expression");
          };
          let expression = expression.unbox().expression;
          if exports_kind.is_commonjs() {
            oxc::ast::ast::Statement::new_module_exports_stmt(expression, &builder)
          } else {
            oxc::ast::ast::Statement::new_export_default_stmt(expression, &builder)
          }
        });
      }
      // Re-running semantic re-stamps every NodeId. The NodeId-keyed side-table lookups
      // below still hit only because the clone is unmutated at this point: identical tree
      // shape re-derives exactly the scan-time ids (see internal-docs/ast-mutation/implementation.md).
      let scoping = EcmaAst::make_semantic(fields.program).into_scoping();

      let mut finalizer = HmrAstFinalizer {
        modules,
        pure: &pure,
        ast_builder: AstBuilder::new(fields.allocator),
        import_bindings: FxHashMap::default(),
        module,
        exports_kind,
        linked_commonjs,
        unconverted_commonjs,
        json_namespace_properties,
        exports: oxc::allocator::Vec::new_in(&fields.allocator),
        use_pife_for_module_wrappers,
        context: self.options.context.as_str(),
        dependencies: FxIndexSet::default(),
        generated_load_exports_stmts: FxHashMap::default(),
        generated_static_import_infos: FxHashMap::default(),
        re_export_all_dependencies: FxIndexSet::default(),
        generated_static_import_stmts_from_external: FxIndexMap::default(),
        unique_index,
        stamp,
        named_exports: FxHashMap::default(),
        native_cell_bindings: crate::hmr::native_cell_routes::bypass_bindings(
          cell_routes,
          module_idx,
        ),
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
    // Collapsing the chain is what lets a position in a patch or a lazy chunk resolve
    // to the source the user wrote, the same way `render_ecma_module` does for a
    // module rendered into a chunk.
    //
    // Warnings are dropped: a module rendered here was rendered by the full build too,
    // which reports a broken chain for it already.
    let (code, codegen_map) = match codegen.map {
      Some(map) => (codegen.code, Some(map.into_owned())),
      None => (codegen.code, None),
    };

    // Guarded, because a chain collapses to a map even when the codegen step
    // contributed none — which would hand back a sourcemap for a render that asked
    // not to have one.
    let map = enable_sourcemap
      .then(|| {
        collapse_module_sourcemap(
          &module.sourcemap_chain,
          codegen_map,
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

struct ClientHeldCopies<'a> {
  shipped: &'a FxHashMap<ArcStr, u32>,
  evaluated: &'a FxHashMap<ArcStr, u32>,
  stamp_table: &'a HmrStampTable,
}

impl ClientHeldCopies<'_> {
  fn holds_current(&self, map: &FxHashMap<ArcStr, u32>, stable_id: &str) -> bool {
    map.get(stable_id).is_some_and(|stamp| !self.stamp_table.is_stale(stable_id, *stamp))
  }
}

/// What an importer's render reads from a normal importee (`HmrAstFinalizer`), apart from
/// `def_format`, which comes from the path and `package.json`, not from the module's source.
fn importee_interop(module: &NormalModule) -> (ExportsKind, bool) {
  (module.exports_kind, module.meta.has_lazy_export())
}

fn static_deps(module: &NormalModule) -> impl Iterator<Item = ModuleIdx> + '_ {
  module
    .import_records
    .iter()
    .filter(|rec| rec.kind.is_static())
    .filter_map(|rec| rec.resolved_module)
}

/// Whether each module of a scan's table has side effects once linked, which decides, as in
/// development output, whether a factory drops an unused import of it, and so whether a walk of
/// what a factory imports follows the import (`hmr::utils::unused_pure_import`). The scan gives a
/// module whose own statements have none, `DeterminedSideEffects::Analyzed(false)`, none even
/// when its imports reach some, which linking gives it (`linked_side_effects`), also for a module
/// that a lazy compile scans anew (#697). Only such a module needs that propagation, which the
/// first question about one computes for every module.
struct LinkedSideEffects<'a> {
  modules: &'a IndexModules,
  /// With native records, which [`wrapped_modules`] models, the kinds that the link gives the
  /// modules; without them, `None`.
  kinds: Option<LinkedKinds<'a>>,
  /// Whether the link reaches a module: one that it doesn't reach decides no kind, wraps nothing
  /// and starts no walk of stars (#660).
  reach: Reach<'a>,
  propagated: std::sync::OnceLock<IndexVec<ModuleIdx, bool>>,
}

impl LinkedSideEffects<'_> {
  /// Whether module `idx` has no side effects once linked. An external module has some:
  /// development output keeps every import of one.
  fn pure(&self, idx: ModuleIdx) -> bool {
    let Some(module) = self.modules[idx].as_normal() else {
      return false;
    };
    if matches!(module.side_effects, DeterminedSideEffects::Analyzed(false)) {
      !self.propagated()[idx]
    } else {
      !module.side_effects.has_side_effects()
    }
  }

  /// Whether each module has side effects once linked.
  fn propagated(&self) -> &IndexVec<ModuleIdx, bool> {
    self.propagated.get_or_init(|| self.propagate())
  }

  /// The link's propagation (`propagate_side_effects`). An `export *` keeps the call of a module
  /// that the link wraps ([`wrapped_modules`]) or whose exports are dynamic, as the link decides
  /// them (`dynamic_exports`). Without native records, which [`wrapped_modules`] models, it keeps
  /// that of any module, which only keeps imports that the link may drop.
  fn propagate(&self) -> IndexVec<ModuleIdx, bool> {
    let modules = self.modules;
    let Some(kinds) = self.kinds else {
      return crate::stages::link_stage::propagate_side_effects(modules, |_| true);
    };
    let reach = self.reach;
    let stars = std::cell::OnceCell::new();
    crate::stages::link_stage::propagate_side_effects(modules, |target| {
      let (wrapped, dynamic) = stars.get_or_init(|| {
        let dynamic = crate::stages::link_stage::dynamic_exports(
          modules,
          |module| kinds.of(module, reach) == ExportsKind::CommonJs,
          |idx| reach.reaches(idx),
        );
        (wrapped_modules(modules, |idx| reach.reaches(idx)), dynamic)
      });
      wrapped.contains(&target) || dynamic[target]
    })
  }
}

/// Where [`LinkedSideEffects`] finds the kind that the link gives a module without ES or
/// CommonJS syntax ([`linked_exports_kind`]).
#[derive(Clone, Copy)]
enum LinkedKinds<'a> {
  /// The records of each module's importers that the cache holds, and whether code splitting is
  /// on.
  Importers { importers: &'a IndexVec<ModuleIdx, Vec<ImporterRecord>>, code_splitting: bool },
  /// The kinds that the update captured before its rescan changed those records.
  Captured(&'a FxHashMap<ModuleIdx, ExportsKind>),
}

impl LinkedKinds<'_> {
  /// The kind that the link gives `module`, by its importers that `reach` accepts.
  fn of(
    self,
    module: &rolldown_common::NormalModule,
    reach: Reach<'_>,
  ) -> ExportsKind {
    if module.exports_kind != ExportsKind::None {
      return module.exports_kind;
    }
    match self {
      Self::Importers { importers, code_splitting } => {
        linked_exports_kind(module, importers, code_splitting, reach)
      }
      Self::Captured(kinds) => kinds.get(&module.idx).copied(),
    }
    .unwrap_or(ExportsKind::None)
  }
}

/// The kind that the link gives `module`, which its importers' records bind to
/// (`determine_module_exports_kind`), by the records of the importers of each module,
/// `importers`. A module without ES or CommonJS syntax becomes an ES module when the first of its
/// importers' records, in discovery order from the entries (#681), imports it, and CommonJS when
/// that record requires it, or imports it with `import()` while code splitting is off; any other
/// module keeps the kind that its scan found. Only importers that `reach` accepts count, those that
/// the graph reaches, as the link passes over the others (#660). `None` for a module without such
/// syntax that nothing imports, which binds nothing.
fn linked_exports_kind(
  module: &rolldown_common::NormalModule,
  importers: &IndexVec<ModuleIdx, Vec<ImporterRecord>>,
  code_splitting: bool,
  reach: Reach<'_>,
) -> Option<ExportsKind> {
  if module.exports_kind != ExportsKind::None {
    return Some(module.exports_kind);
  }
  let mut importers = importers
    .get(module.idx)
    .map(|importers| {
      importers.iter().filter(|importer| reach.reaches(importer.importer_idx)).collect::<Vec<_>>()
    })
    .unwrap_or_default();
  if importers.is_empty() {
    return None;
  }
  importers.sort_by_key(|importer| {
    (reach.discovery_order(importer.importer_idx), importer.importer_idx.index())
  });
  Some(
    importers
      .iter()
      .find_map(|importer| match importer.kind {
        ImportKind::Import if !module.meta.has_lazy_export() => Some(ExportsKind::Esm),
        ImportKind::Require => Some(ExportsKind::CommonJs),
        ImportKind::DynamicImport if !code_splitting => Some(ExportsKind::CommonJs),
        _ => None,
      })
      .unwrap_or(ExportsKind::None),
  )
}

/// The modules that the link wraps (`determine_module_exports_kind`, `wrap_modules`): with native
/// records, which an ES output with code splitting renders, every module that a record requires,
/// every CommonJS module, and every module that a wrapped module's records reach, a dynamic
/// `import()`'s included. A module that `reaches` rejects, which the graph no longer reaches,
/// wraps nothing (#660).
fn wrapped_modules(
  modules: &IndexModules,
  reaches: impl Fn(ModuleIdx) -> bool,
) -> FxHashSet<ModuleIdx> {
  let mut stack = modules
    .iter()
    .filter_map(Module::as_normal)
    .filter(|module| reaches(module.idx))
    .flat_map(|module| {
      module
        .import_records
        .iter()
        .filter(|record| record.kind == ImportKind::Require)
        .filter_map(|record| record.resolved_module)
        .chain((module.exports_kind == ExportsKind::CommonJs).then_some(module.idx))
    })
    .collect::<Vec<_>>();
  let mut wrapped = FxHashSet::default();
  while let Some(module_idx) = stack.pop() {
    let Some(module) = modules.get(module_idx).and_then(Module::as_normal) else {
      continue;
    };
    if module.id.as_str() == rolldown_common::RUNTIME_MODULE_KEY || !wrapped.insert(module_idx) {
      continue;
    }
    stack.extend(module.import_records.iter().filter_map(|record| record.resolved_module));
  }
  wrapped
}

/// Whether the modules that a partial scan loaded, `loaded`, change what linking derives the
/// side effects of the scan's modules, `modules`, from ([`LinkedSideEffects`]): a module's own
/// side effects, its kind, and its import records with their targets. A module that the scan
/// appends changes them only through the records of a module that it rescans.
fn rescans_side_effect_inputs(
  modules: &IndexModules,
  loaded: &HybridIndexVec<ModuleIdx, Module>,
) -> bool {
  let inputs = |module: &Module| {
    module.as_normal().map(|module| {
      (
        std::mem::discriminant(&module.side_effects),
        module.side_effects.has_side_effects(),
        module.exports_kind,
        module.meta.has_lazy_export(),
        module
          .import_records
          .iter()
          .map(|record| {
            (
              record.kind,
              record.resolved_module,
              record.meta.contains(ImportRecordMeta::IsExportStar),
            )
          })
          .collect::<Vec<_>>(),
      )
    })
  };
  let HybridIndexVec::Map(loaded) = loaded else {
    return true;
  };
  loaded
    .iter()
    .any(|(idx, module)| modules.get(*idx).is_some_and(|before| inputs(before) != inputs(module)))
}

/// The modules that `entries` reach through import records of any kind.
fn reachable_modules(
  modules: &IndexModules,
  entries: impl IntoIterator<Item = ModuleIdx>,
) -> FxHashSet<ModuleIdx> {
  let mut reachable = FxHashSet::default();
  let mut stack = entries.into_iter().collect::<Vec<_>>();
  while let Some(module_idx) = stack.pop() {
    if !reachable.insert(module_idx) {
      continue;
    }
    if let Some(module) = modules.get(module_idx).and_then(Module::as_normal) {
      stack.extend(module.import_records.iter().filter_map(|record| record.resolved_module));
    }
  }
  reachable
}

impl<Fs: FileSystem + Clone + 'static> HmrStage<'_, Fs> {
  /// The importers whose unused import of a module the update changes whether development output
  /// keeps, which it keeps while linking gives the module side effects, with those that its
  /// imports reach (`hmr::utils::unused_side_effect_free_import`): `before` and `after` say
  /// whether linking gives each module some. Only an importer that a page may hold counts: a
  /// module that a user-defined entry reaches through any import, a route that an activation
  /// compiled included. A page whose records or factories dropped such an import never ran the
  /// module through it, and a patch re-runs only modules that a page ran, so an import that
  /// development output starts keeping reloads, with the reason as the error. One that it stops
  /// keeping changes the importer's factory, which the update then ships and re-runs as an edited
  /// module's; the module, which has no side effects any more, needs no more (#697).
  fn flipped_unused_imports(
    &self,
    before: &IndexVec<ModuleIdx, bool>,
    after: &LinkedSideEffects<'_>,
    rescanned: &FxIndexSet<ModuleIdx>,
  ) -> Result<FxIndexSet<ModuleIdx>, String> {
    let modules = &self.module_table().modules;
    let snapshot = self.cache.get_snapshot();
    let mut reachable = None;
    let mut dropping = FxIndexSet::default();
    for ((target, &had), &has) in before.iter_enumerated().zip(after.propagated().iter()) {
      // Development output keeps every import of an external module.
      if had == has || !modules[target].is_normal() {
        continue;
      }
      let importers = self
        .cache
        .importers
        .get(target)
        .into_iter()
        .flatten()
        .filter(|importer| importer.kind == ImportKind::Import)
        .map(|importer| importer.importer_idx)
        .collect::<std::collections::BTreeSet<_>>();
      for importer_idx in importers {
        let Some(importer) = modules.get(importer_idx).and_then(Module::as_normal) else {
          continue;
        };
        let scoping = crate::hmr::utils::import_scoping(
          &snapshot.symbol_ref_db,
          self.cache.cached_native_relink.as_deref(),
          importer,
          rescanned.contains(&importer_idx),
        );
        if !importer.import_records.iter_enumerated().any(|(record_idx, record)| {
          record.resolved_module == Some(target)
            && crate::hmr::utils::drops_import(importer, record_idx, |_| true, scoping)
        }) || !reachable
          .get_or_insert_with(|| {
            reachable_modules(modules, snapshot.user_defined_entry_modules.iter().copied())
          })
          .contains(&importer_idx)
        {
          continue;
        }
        if has {
          return Err(format!(
            "{} gains side effects, so development output keeps the unused import of it in {}, \
             which a page's records and factories dropped; a patch cannot run the module there",
            modules[target].stable_id(),
            importer.stable_id,
          ));
        }
        dropping.insert(importer_idx);
      }
    }
    Ok(dropping)
  }

  #[expect(clippy::too_many_arguments)]
  fn collect_sync_dependencies_for_client(
    &self,
    proxy_entry_idx: ModuleIdx,
    result: &mut FxIndexSet<ModuleIdx>,
    shipped: &FxHashMap<ArcStr, u32>,
    rerun: &FxHashSet<ArcStr>,
    evaluated: &FxHashMap<ArcStr, u32>,
    entry_evaluated: Option<&FxHashMap<ArcStr, u32>>,
    partly_run: Option<&FxHashSet<ArcStr>>,
    mut eager: Option<&mut FxHashSet<ArcStr>>,
    mut routed: Option<&mut Vec<(ModuleIdx, ModuleIdx)>>,
    stamp_table: &HmrStampTable,
    cell_routes: &[crate::hmr::native_cell_routes::Route],
    side_effects: &LinkedSideEffects<'_>,
    dropped: &mut FxHashSet<(ModuleIdx, ImportRecordIdx)>,
  ) {
    let modules = &self.module_table().modules;
    let holds_current = |map: &FxHashMap<ArcStr, u32>, stable_id: &str| {
      map.get(stable_id).is_some_and(|stamp| !stamp_table.is_stale(stable_id, *stamp))
    };
    let mut stack = vec![proxy_entry_idx];
    // The resident factories that `rerun` names, walked for their imports but not shipped: the
    // entry's too, whose dynamic imports the walk then leaves to their own compiles.
    let mut walked_through = FxHashSet::default();
    if let Module::Normal(entry) = &modules[proxy_entry_idx]
      && rerun.contains(entry.stable_id.as_str())
      && holds_current(shipped, entry.stable_id.as_str())
    {
      walked_through.insert(proxy_entry_idx);
    }
    // A factory binds the cells and literal records of the bindings that these records read,
    // never their donors (`native_cell_routes::bypass_bindings`).
    let bypasses = cell_routes
      .iter()
      .filter(|route| route.verdict == crate::hmr::native_cell_routes::Verdict::Bypass)
      .map(|route| ((route.importer, route.record), route.live_owners.as_slice()))
      .collect::<FxHashMap<_, _>>();

    while let Some(module_idx) = stack.pop() {
      if !result.insert(module_idx) {
        continue;
      }
      let Module::Normal(module) = &modules[module_idx] else {
        continue;
      };
      let evaluated =
        entry_evaluated.filter(|_| module_idx == proxy_entry_idx).unwrap_or(evaluated);
      // The scope tree that decides which of the module's imports its factory drops, as for the
      // evaluation of what an entry ran (`hmr::utils::import_scoping`).
      let scoping = crate::hmr::utils::import_scoping(
        &self.cache.get_snapshot().symbol_ref_db,
        self.cache.cached_native_relink.as_deref(),
        module,
        false,
      );
      // The module's `import defer` records, which a route's records evaluate only once the
      // module reads the namespace.
      let deferred =
        self.index_ecma_ast()[module_idx].as_ref().map_or_else(FxHashSet::default, |ast| {
          crate::hmr::utils::deferred_records(module, ast.program())
        });

      for (rec_idx, rec) in module.import_records.iter_enumerated() {
        // For the proxy entry module, also follow dynamic imports.
        // The proxy's fetched template has `import($MODULE_ID)` pointing to the real module.
        // We need to include the real module and its sync dependencies in the patch.
        let should_follow = rec.kind.is_static()
          || (module_idx == proxy_entry_idx
            && rec.kind == ImportKind::DynamicImport
            && !walked_through.contains(&module_idx));
        if !should_follow {
          continue;
        }
        // A factory drops an unused import of a module without side effects once linked
        // (`LinkedSideEffects`). Without a scope tree to decide it, the walk follows the import,
        // never to ship less than the factory runs.
        if let Some(scoping) = scoping
          && crate::hmr::utils::drops_import(
            module,
            rec_idx,
            |idx| side_effects.pure(idx),
            Some(scoping),
          )
        {
          dropped.insert((module_idx, rec_idx));
          continue;
        }
        // A factory runs what it imports with `import defer` at once.
        if deferred.contains(&rec_idx) {
          let Some(eager) = eager.as_deref_mut() else {
            continue;
          };
          if let Some(Module::Normal(target)) = rec.resolved_module.map(|idx| &modules[idx])
            && !evaluated.contains_key(target.stable_id.as_str())
          {
            eager.insert(target.stable_id.as_arc_str().clone());
          }
        }
        let cells = bypasses.get(&(module_idx, rec_idx));
        let dependencies = cells
          .into_iter()
          .flat_map(|owners| owners.iter().copied())
          .chain(rec.resolved_module.filter(|_| cells.is_none()));

        for dep_idx in dependencies {
          // JSON's loader-owned classification marker is not an executable dependency.
          if matches!(rec.module_request.as_str(), "\0rolldown/json-module" | "\0rolldown/text-module")
            && modules[dep_idx].id().as_str() == rolldown_common::RUNTIME_MODULE_KEY
          {
            continue;
          }
          // The factory runs what it imports wherever the clients that run it lack it.
          if let Some(routed) = routed.as_deref_mut()
            && !modules[dep_idx]
              .as_normal()
              .is_some_and(|dep| holds_current(evaluated, dep.stable_id.as_str()))
          {
            routed.push((module_idx, dep_idx));
          }
          // A module that some inputs ran or may run, which the clients that run this factory may
          // lack, ships again, though it is resident, already selected or the entry.
          if let Some(partly_run) = partly_run
            && let Module::Normal(normal_dep) = &modules[dep_idx]
            && partly_run.contains(normal_dep.stable_id.as_str())
            && !evaluated.contains_key(normal_dep.stable_id.as_str())
          {
            walked_through.remove(&dep_idx);
            if !result.contains(&dep_idx) {
              stack.push(dep_idx);
            }
            continue;
          }
          // A module with N importers hits this edge check N times; the cheap
          // visited test spares the ship-map string hashing for all but the first.
          if result.contains(&dep_idx) {
            continue;
          }
          if let Module::Normal(normal_dep) = &modules[dep_idx] {
            // Skip deps whose current copy this client already holds: factory
            // resident per the ship map, or exports live per the top-level-evaluated
            // map (a lazy import never re-runs an evaluated module, so
            // `initModule` serves it without a factory). A resident factory that
            // `rerun` names may lack its imports, which the walk goes on to.
            let stable_id = normal_dep.stable_id.as_str();
            if holds_current(evaluated, stable_id) {
              continue;
            }
            if holds_current(shipped, stable_id) {
              if !rerun.contains(stable_id) {
                continue;
              }
              walked_through.insert(dep_idx);
            }
          }
          stack.push(dep_idx);
        }
      }
    }
    result.retain(|module_idx| !walked_through.contains(module_idx));
  }

  /// Goes through shipped modules: a patch can carry a module the client never ran. See
  /// internal-docs/hmr/design.md, principle 2. `walk_from` modules are never carried.
  fn collect_unheld_sync_deps(
    &self,
    walk_from: &[ModuleIdx],
    targets: impl IntoIterator<Item = ModuleIdx>,
    follow_dynamic_of: Option<ModuleIdx>,
    held: &ClientHeldCopies<'_>,
    out: &mut FxIndexSet<ModuleIdx>,
  ) {
    let modules = &self.module_table().modules;
    let mut visited = walk_from.iter().copied().collect::<FxHashSet<_>>();
    let mut to_walk = walk_from.to_vec();
    let mut to_sort = targets.into_iter().collect::<Vec<_>>();

    loop {
      while let Some(module_idx) = to_sort.pop() {
        if !visited.insert(module_idx) {
          continue;
        }
        let Module::Normal(module) = &modules[module_idx] else {
          continue;
        };
        let stable_id = module.stable_id.as_str();
        if held.holds_current(held.evaluated, stable_id) {
          continue;
        }
        if !held.holds_current(held.shipped, stable_id) {
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
        // The proxy entry's fetched template reaches the real module through `import()`.
        let follow = rec.kind.is_static()
          || (Some(module_idx) == follow_dynamic_of && rec.kind == ImportKind::DynamicImport);
        if follow && let Some(dep_idx) = rec.resolved_module {
          to_sort.push(dep_idx);
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
