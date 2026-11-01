use std::path::PathBuf;

use arcstr::ArcStr;
use futures::future::try_join_all;
use oxc_index::IndexVec;
use render_chunk_to_assets::set_emitted_chunk_preliminary_filenames;
use rolldown_common::{
  ChunkIdx, ChunkKind, InstantiationKind, ModuleIdx, OutputExports, PackageJson, PathsOutputOption,
  UsedSymbolRefs, UsedSymbolRefsBuilder,
};
use rolldown_devtools::{action, trace_action, trace_action_enabled};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_plugin::SharedPluginDriver;
use rolldown_std_utils::{
  PathBufExt as _, absolutize_path_buf, path_buf_to_slash, relative_path_to_slash,
  representative_file_name_for_preserve_modules, strip_path_prefix_to_slash,
};
use rolldown_utils::{
  concat_string, dashmap::FxDashMap, hash_placeholder::HashPlaceholderGenerator,
  index_vec_ext::IndexVecExt as _, indexmap::FxIndexSet, node_style_absolute,
  rayon::ParallelIterator as _,
};
use rustc_hash::{FxHashMap, FxHashSet};
use sugar_path::SugarPath as _;
use tracing::debug_span;

const COMMON_JS_EXTENSIONS: &[&str] = &["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"];

#[derive(Debug)]
#[derive(Default)]
struct PreGeneratedChunkName {
  /// The representative name used for symbol deconflicting and chunk binding references.
  representative_chunk_name: ArcStr,
  /// The full chunk name including directory structure relative to `preserveModulesRoot`.
  /// This appears in `PreRenderedChunk.name` and hooks like `entryFileNames`.
  chunk_name: ArcStr,
  /// The input for the `[name]` placeholder of filename templates. For a preserved module,
  /// this is its sanitized relative path without the extension.
  chunk_filename: ArcStr,
}

type DevtoolsPackageInfoEntry = (action::PackageInfo, FxIndexSet<String>, FxIndexSet<u32>);

/// Representative chunk names keyed by chunk, plus the registry id of every inline common chunk
/// record (records have a name but no file).
type ChunkNamingResult = (FxHashMap<ChunkIdx, ArcStr>, Vec<(ChunkIdx, ArcStr)>);

/// The names of a preserved module's chunk, from which its file name and `[name]` derive.
async fn preserve_modules_chunk_name(
  options: &rolldown_common::NormalizedBundlerOptions,
  module_id: &str,
  input_base: &str,
) -> anyhow::Result<PreGeneratedChunkName> {
  let sanitize_filename = &options.sanitize_filename;
  let preserve_modules_root = &options.preserve_modules_root;
  let virtual_dirname = &options.virtual_dirname;
  let cwd = &options.cwd;
  let (representative_chunk_name, absolute_chunk_file_name, ext) =
    representative_file_name_for_preserve_modules(module_id.as_path());

  // The relative path is built from the raw id and sanitized once at the end:
  // `preserveModulesRoot` and the input base are raw paths, so a sanitized id may no
  // longer start with them, or may start with another directory that sanitizes to
  // the same name.
  let relative_path = {
    let p = PathBuf::from(absolute_chunk_file_name.as_str());
    // Besides genuinely absolute paths, `node_style_absolute` anchors
    // a rooted-but-volume-less id (`/favicon`, `\favicon`) to the
    // cwd volume root (a drive or UNC share): Node and Rollup treat
    // such ids as absolute, but Rust's
    // `Path::is_absolute()` reports them as non-absolute on Windows,
    // and letting them fall into the `virtual_dirname` join below
    // would discard the prefix and leak the leading slash into
    // `[name]`.
    if let Some(abs) = node_style_absolute(&p, cwd) {
      let stripped_by_root = preserve_modules_root.as_ref().and_then(|preserve_modules_root| {
        // See internal-docs/module-id/implementation.md: output paths may normalize separators even
        // when module ids keep native separators.
        strip_path_prefix_to_slash(&abs, preserve_modules_root.as_path())
      });
      if let Some(relative_path) = stripped_by_root {
        relative_path
      } else {
        relative_path_to_slash(abs, input_base)
      }
    } else {
      path_buf_to_slash(PathBuf::from(virtual_dirname.as_str()).join(p))
    }
  };
  // The `[name]` placeholder gets the path without the extension.
  let chunk_filename = sanitize_filename.call_relative(&relative_path).await?;
  // `relative_path` has no extension. The chunk name gets it back, except for
  // common ones: https://github.com/rollup/rollup/pull/4565/files
  let chunk_name = match ext.as_deref() {
    Some(e) if !e.is_empty() && !COMMON_JS_EXTENSIONS.contains(&e) => {
      sanitize_filename.call_relative(&concat_string!(relative_path, ".", e)).await?
    }
    _ => chunk_filename.clone(),
  };

  let sanitized_representative_chunk_name =
    sanitize_filename.call(&representative_chunk_name).await?;
  Ok(PreGeneratedChunkName {
    representative_chunk_name: sanitized_representative_chunk_name,
    chunk_name,
    chunk_filename,
  })
}

fn is_devtools_source_importer(module_id: &str, cwd: &str, cwd_slash: &str) -> bool {
  fn is_path_inside(path: &str, parent: &str) -> bool {
    let parent = parent.trim_end_matches(['/', '\\']);
    path == parent
      || path
        .strip_prefix(parent)
        .is_some_and(|rest| rest.starts_with('/') || rest.starts_with('\\'))
  }

  let is_inside_cwd = is_path_inside(module_id, cwd) || is_path_inside(module_id, cwd_slash);
  let is_node_module =
    module_id.contains("/node_modules/") || module_id.contains("\\node_modules\\");

  is_inside_cwd && !is_node_module
}

fn ensure_devtools_package_info<'a>(
  package_infos: &'a mut FxHashMap<String, DevtoolsPackageInfoEntry>,
  package_json: &PackageJson,
) -> Option<&'a mut DevtoolsPackageInfoEntry> {
  let package_root = package_json.realpath().parent()?.to_slash_lossy().into_owned();
  let package_json_path = package_json.realpath().to_slash_lossy().into_owned();

  Some(package_infos.entry(package_root.clone()).or_insert_with(|| {
    (
      action::PackageInfo {
        package_id: package_root.clone(),
        name: package_json.name().map(str::to_string),
        version: package_json.version().map(str::to_string),
        package_json_path,
        package_root,
        is_used: false,
        dependency_type: "transitive",
        size: 0,
        modules: Vec::new(),
        chunk_ids: Vec::new(),
      },
      FxIndexSet::default(),
      FxIndexSet::default(),
    )
  }))
}

use crate::{
  BundleOutput, SharedOptions,
  chunk_graph::ChunkGraph,
  stages::{
    generate_stage::compute_cross_chunk_links::FinalEsmInitMetadataAvailability,
    link_stage::LinkStageOutput,
  },
  type_alias::IndexEcmaAst,
  types::{
    generator::GenerateContext,
    scan_stage_cache::{CachedChunkNaming, ScanStageCache},
  },
  utils::chunk::{
    deconflict_chunk_symbols::deconflict_chunk_symbols,
    determine_export_mode::determine_export_mode, generate_pre_rendered_chunk,
    render_chunk_exports::get_chunk_export_names,
  },
  utils::external_import_interop::ChunkAssignments,
};

mod chunk_ext;
mod chunk_optimizer;
mod code_splitting;
mod compute_cross_chunk_links;
mod compute_wrapped_esm_init_metadata;
mod detect_ineffective_dynamic_imports;
mod dynamic_already_loaded;
mod entry_facades;
mod finalize_chunk_plan;
mod finalize_modules;
mod inline_common_chunks;
mod manual_code_splitting;
mod minify_chunks;
mod native_relink_graph;
mod order_analysis;
pub mod order_wrap_state;
mod order_wrapping;
mod post_banner_footer;
mod reduced_chunk_graph;
mod render_chunk_to_assets;
mod resolve_file_urls;
mod runtime_module_sweep;
mod simulated_facade_inclusion;

pub(crate) use code_splitting::common_dir_input;
pub use compute_wrapped_esm_init_metadata::{FinalEsmInitMetadata, Sealed};
pub use inline_common_chunks::{
  CarriedRender, FileInlineNames, InlineCommonChunksState, InlineReader,
};
pub use native_relink_graph::NativeChunkChange;
pub(crate) use native_relink_graph::is_live_chunk;

#[derive(Debug)]
pub struct PendingNativeRelinkState {
  pub chunk_graph: ChunkGraph,
  pub used_symbol_refs: UsedSymbolRefs,
  pub order_state: order_wrap_state::OrderWrapState,
  pub final_esm_init_metadata: Sealed<FinalEsmInitMetadata>,
  pub index_chunk_id_to_name: FxHashMap<ChunkIdx, ArcStr>,
  pub chunk_decisions: IndexVec<ChunkIdx, Option<[u128; 2]>>,
  pub module_owner_digest: IndexVec<ModuleIdx, [u128; 2]>,
  pub runtime_chunk_sig: Option<[u128; 2]>,
  pub module_has_native_ns: IndexVec<ModuleIdx, bool>,
  pub render_export_items_index_vec:
    IndexVec<ChunkIdx, rolldown_utils::indexmap::FxIndexMap<rolldown_common::SymbolRef, Vec<oxc_str::CompactStr>>>,
  pub index_chunk_to_instances: crate::type_alias::IndexChunkToInstances,
  pub protected_modules: FxHashSet<ModuleIdx>,
  /// The modules that chunk decisions depend on, among the protected ones: deferred imports'
  /// targets and native cells' evaluation dependencies. A module that imports one may change
  /// how it evaluates, where the decisions assume it as they were made.
  pub decision_dependencies: FxHashSet<ModuleIdx>,
  pub sorted_output_template: Vec<(ChunkIdx, rolldown_common::Output)>,
  pub chunk_idx_to_template_pos: IndexVec<ChunkIdx, Option<usize>>,
  pub reused_chunk_filenames_pool: FxHashSet<ArcStr>,
  pub excluded_from_reused_filenames: Vec<ArcStr>,
  pub pool_installed: bool,
  /// What fast relinks since linked as used for each module that they relinked, in place of
  /// `used_symbol_refs`.
  pub used_overrides: FxHashMap<ModuleIdx, FxHashSet<rolldown_common::SymbolRef>>,
}

pub struct GenerateStage<'a> {
  link_output: &'a mut LinkStageOutput,
  ast_table: IndexEcmaAst,
  options: &'a SharedOptions,
  plugin_driver: &'a SharedPluginDriver,
  cache: Option<&'a mut ScanStageCache>,
  /// Pre-resolved paths for external modules. When the user provides a JS function for the
  /// `paths` option, it is resolved asynchronously here before entering sync rendering code,
  /// avoiding the need for `invoke_sync` which can cause deadlocks.
  resolved_paths: Option<PathsOutputOption>,
  /// Decisions of `experimentalInlineCommonChunks`; empty unless the option is on.
  inline_state: InlineCommonChunksState,
  /// Carrier -> the records it prints, finalized and rendered for it (`finalize_inline_copies`);
  /// moved out when the files are instantiated.
  inline_renders: FxHashMap<ChunkIdx, Vec<CarriedRender>>,
  pub pending_native_relink: Option<Box<PendingNativeRelinkState>>,
}

impl Drop for GenerateStage<'_> {
  fn drop(&mut self) {
    if !self.ast_table.is_empty()
      && let Some(cache) = self.cache.as_deref_mut()
      && cache.has_snapshot()
    {
      let mut ast_table = std::mem::take(&mut self.ast_table);
      for (idx, original_ast) in std::mem::take(&mut self.link_output.pre_lazy_export_asts) {
        ast_table[idx] = Some(original_ast);
      }
      cache.get_snapshot_mut().index_ecma_ast = ast_table;
    }
  }
}

impl<'a> GenerateStage<'a> {
  pub fn new_with_cache(
    link_output: &'a mut LinkStageOutput,
    ast_table: IndexEcmaAst,
    options: &'a SharedOptions,
    plugin_driver: &'a SharedPluginDriver,
    cache: Option<&'a mut ScanStageCache>,
  ) -> Self {
    Self {
      link_output,
      ast_table,
      options,
      plugin_driver,
      cache,
      resolved_paths: None,
      inline_state: InlineCommonChunksState::default(),
      inline_renders: FxHashMap::default(),
      pending_native_relink: None,
    }
  }

  pub fn take_restored_ast_table(&mut self) -> IndexEcmaAst {
    let mut ast_table = std::mem::take(&mut self.ast_table);
    for (idx, original_ast) in std::mem::take(&mut self.link_output.pre_lazy_export_asts) {
      ast_table[idx] = Some(original_ast);
    }
    ast_table
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn generate(
    &mut self,
    mut used_symbol_refs_builder: UsedSymbolRefsBuilder,
  ) -> BuildResult<BundleOutput> {
    self.plugin_driver.render_start(self.options).await?;
    self.prepare_inline_common_chunks().await?;
    let mut chunk_graph = self.generate_chunks(&mut used_symbol_refs_builder).await?;

    let finalize_chunk_plan::FinalizedChunkPlan {
      mut order_state,
      final_esm_init_metadata,
      mut link_state,
    } = self.finalize_chunk_plan(&mut chunk_graph, &mut used_symbol_refs_builder)?;

    // Order lowering and the unused-runtime sweep have had their last chance to update liveness.
    // Sealing consumes the builder, so nothing downstream can mutate the set.
    let used_symbol_refs = used_symbol_refs_builder.seal();
    self.compute_retained_export_symbols(&used_symbol_refs);
    self.narrow_consumer_local_namespace_targets(&mut order_state);

    // Cross-chunk linking in two halves. `experimentalInlineCommonChunks` decides on the derived
    // final edges and registers the registry's runtime demand; a second derivation then carries
    // that demand (it only adds edges to the runtime chunk) into the one commit. Records stay
    // live chunks throughout, so their own edges are derived like any other chunk's, and the
    // projection afterwards turns the logical edges into the physical file graph.
    // See internal-docs/inline-common-chunks/implementation.md.
    if self.select_inline_common_chunks(&chunk_graph, &link_state, &mut order_state) {
      link_state = self.compute_cross_chunk_link_state(
        &chunk_graph,
        used_symbol_refs.view(),
        &order_state,
        FinalEsmInitMetadataAvailability::Sealed(&final_esm_init_metadata),
      );
    }
    self.commit_cross_chunk_links(&mut chunk_graph, link_state, &used_symbol_refs);
    self.apply_inline_common_chunks_links(&mut chunk_graph);

    self.ensure_lazy_module_initialization_order(&mut chunk_graph);

    self.merge_cjs_namespace(&mut chunk_graph, &order_state);

    self.trace_action_chunks_infos(&chunk_graph);

    let mut warnings = vec![];
    self.compute_chunk_output_exports(
      &mut chunk_graph,
      &mut warnings,
      &used_symbol_refs,
      &order_state,
    )?;
    if !warnings.is_empty() {
      self.link_output.diagnostics.extend(warnings);
    }
    let (index_chunk_id_to_name, inline_record_ids) =
      self.generate_chunk_name_and_preliminary_filenames(&mut chunk_graph).await?;
    for (chunk_idx, id) in inline_record_ids {
      self.inline_state.set_record_id(chunk_idx, id);
    }
    set_emitted_chunk_preliminary_filenames(&self.plugin_driver.file_emitter, &chunk_graph);

    // Pre-resolve paths for external modules to avoid sync JS callbacks during rendering.
    // This eliminates the need for `invoke_sync` which can cause deadlocks (see #7280).
    if let Some(paths) = &self.options.paths {
      let ids = self
        .link_output
        .module_table
        .modules
        .iter()
        .filter_map(|m| m.as_external().map(|e| e.id.as_str()));
      self.resolved_paths = Some(paths.resolve_all(ids).await);
    }

    let resolved_file_urls = self.resolve_file_urls(&chunk_graph).await?;
    let (chunk_decisions, reused_chunks, relink_digest_state) = self.compute_native_chunk_decisions(
      &chunk_graph,
      &resolved_file_urls,
      &used_symbol_refs,
      &order_state,
      &final_esm_init_metadata,
      &index_chunk_id_to_name,
    );

    // A file names its carried records' modules together with its own.
    // See internal-docs/inline-common-chunks/implementation.md ("Deconflicting").
    let inline_naming_inputs = self.inline_naming_inputs(&chunk_graph);
    debug_span!("deconflict_chunk_symbols").in_scope(|| {
      // Borrow the chunk table mutably alongside the assignment tables it does not touch, so
      // deconflicting can attribute recorded external interop to the chunk it belongs to.
      let ChunkGraph { chunk_table, module_to_chunk, post_chunk_optimization_operations, .. } =
        &mut chunk_graph;
      let chunk_assignments =
        ChunkAssignments::new(&*module_to_chunk, &*post_chunk_optimization_operations);
      let inline_names = chunk_table
        .par_iter_mut_enumerated()
        .filter_map(|(chunk_idx, chunk)| {
          if reused_chunks.contains(&chunk_idx)
            || post_chunk_optimization_operations.contains_key(&chunk_idx)
            || self.inline_state.is_record(chunk_idx)
          {
            return None;
          }
          let names = deconflict_chunk_symbols(
            chunk_idx,
            chunk,
            self.link_output,
            &order_state,
            self.options.format,
            &index_chunk_id_to_name,
            chunk_assignments,
            inline_naming_inputs.get(&chunk_idx),
          );
          if chunk.pre_rendered_chunk.is_none()
            && let Some(chunk_name) = chunk.name.clone()
          {
            chunk.pre_rendered_chunk =
              Some(generate_pre_rendered_chunk(chunk, &chunk_name, self.link_output));
          }
          Some((chunk_idx, names?))
        })
        .collect::<Vec<_>>();
      for (chunk_idx, names) in inline_names {
        self.inline_state.set_file_names(chunk_idx, names);
      }
    });

    let rendered_modules = self.finalize_modules(
      &mut chunk_graph,
      &resolved_file_urls,
      &used_symbol_refs,
      &order_state,
      &final_esm_init_metadata,
      &reused_chunks,
    )?;
    self.detect_ineffective_dynamic_imports(&chunk_graph);
    let bundle_output = self
      .render_chunk_to_assets(
        &chunk_graph,
        rendered_modules,
        &used_symbol_refs,
        &order_state,
        &final_esm_init_metadata,
        &chunk_decisions,
        &reused_chunks,
      )
      .await?;

    if let Some((
      runtime_chunk_sig,
      module_has_native_ns,
      module_owner_digest,
      protected_modules,
      decision_dependencies,
    )) = relink_digest_state
      && let Some(cache) = self.cache.as_ref()
      && self.options.preserve_modules
      && crate::utils::native_records::enabled(self.options)
      && cache.omit_reused_chunk_bytes
      && !trace_action_enabled!()
      && self.options.paths.is_none()
      && !self.options.minify.is_enabled()
      && self.options.optimization.inline_const_pass() <= 1
      && self.link_output.diagnostics.is_empty()
      && !order_state.has_import_overlays()
      && order_state.required_runtime_helpers().is_empty()
      && chunk_graph.post_chunk_optimization_operations.is_empty()
      && (chunk_graph.module_to_chunk[self.link_output.runtime.id()].is_none()
        || cache.runtime_rendered_chunk.is_some())
      && chunk_graph.chunk_table.iter_enumerated().all(|(cidx, c)| {
        c.modules.len() == 1
          && chunk_decisions.get(cidx).and_then(|d| *d).is_some()
          && cache.cached_native_chunks.contains_key(&c.modules[0])
      })
    {
      let chunks_len = chunk_graph.chunk_table.len();
      let mut sorted_output_template = Vec::with_capacity(chunks_len);
      let mut reused_chunk_filenames_pool =
        FxHashSet::with_capacity_and_hasher(chunks_len, Default::default());
      for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
        let module_idx = chunk.modules[0];
        let cached_chunk = &cache.cached_native_chunks[&module_idx].chunk;
        reused_chunk_filenames_pool.insert(cached_chunk.filename.clone());
        sorted_output_template
          .push((chunk_idx, rolldown_common::Output::Chunk(std::sync::Arc::clone(cached_chunk))));
      }
      sorted_output_template.sort_unstable_by(|(_, a), (_, b)| {
        let a_type = render_chunk_to_assets::get_sorting_file_type_key(a);
        let b_type = render_chunk_to_assets::get_sorting_file_type_key(b);
        if a_type == b_type {
          return a.filename().cmp(b.filename());
        }
        a_type.cmp(&b_type)
      });
      let mut chunk_idx_to_template_pos = oxc_index::index_vec![None; chunks_len];
      for (pos, &(chunk_idx, _)) in sorted_output_template.iter().enumerate() {
        chunk_idx_to_template_pos[chunk_idx] = Some(pos);
      }
      let render_export_items_index_vec =
        oxc_index::index_vec![rolldown_utils::indexmap::FxIndexMap::default(); chunks_len];
      let index_chunk_to_instances = oxc_index::index_vec![FxIndexSet::default(); chunks_len];
      self.pending_native_relink = Some(Box::new(PendingNativeRelinkState {
        chunk_graph,
        used_symbol_refs,
        order_state,
        final_esm_init_metadata,
        index_chunk_id_to_name,
        chunk_decisions,
        module_owner_digest,
        runtime_chunk_sig,
        module_has_native_ns,
        render_export_items_index_vec,
        index_chunk_to_instances,
        protected_modules,
        decision_dependencies,
        sorted_output_template,
        chunk_idx_to_template_pos,
        reused_chunk_filenames_pool,
        excluded_from_reused_filenames: Vec::new(),
        pool_installed: false,
        used_overrides: FxHashMap::default(),
      }));
    }

    Ok(bundle_output)
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn generate_fast_native_relink(
    &mut self,
    cached_relink: &mut PendingNativeRelinkState,
    dirty_modules_with_used: &[(
      ModuleIdx,
      ChunkIdx,
      FxHashSet<rolldown_common::SymbolRef>,
    )],
    graph_change: Option<&NativeChunkChange>,
  ) -> BuildResult<Result<(BundleOutput, Vec<ArcStr>), &'static str>> {
    let retired_filenames = match graph_change {
      Some(change) => match self.apply_native_graph_change(cached_relink, change).await? {
        Ok(filenames) => filenames,
        Err(reason) => return Ok(Err(reason)),
      },
      None => Vec::new(),
    };
    // Only the output's own records render; a retired record must not be published again.
    if dirty_modules_with_used.iter().any(|&(module_idx, chunk_idx, _)| {
      cached_relink.chunk_graph.module_to_chunk.get(module_idx).copied().flatten()
        != Some(chunk_idx)
        || cached_relink.chunk_idx_to_template_pos.get(chunk_idx).copied().flatten().is_none()
    }) {
      return Ok(Err("dirty_record"));
    }
    self.plugin_driver.render_start(self.options).await?;
    set_emitted_chunk_preliminary_filenames(
      &self.plugin_driver.file_emitter,
      &cached_relink.chunk_graph,
    );

    let runtime_id = self.link_output.runtime.id();
    if let Some(cache) = self.cache.as_ref() {
      for &(module_idx, chunk_idx, ref local_used_symbols) in dirty_modules_with_used {
        // A barrel that the relink appends is protected, with every module that it re-exports
        // or imports, as a whole generation protects one (#629).
        if cache.export_star_modules.contains(&module_idx) {
          cached_relink.protected_modules.insert(module_idx);
          if let Some(module) = self.link_output.module_table.modules.get(module_idx) {
            cached_relink
              .protected_modules
              .extend(module.import_records().iter().filter_map(|rec| rec.resolved_module));
          }
        }
        let chunk = &cached_relink.chunk_graph.chunk_table[chunk_idx];
        let (decision, _, dependencies) = self.compute_single_native_chunk_decision(
          cache,
          &cached_relink.chunk_graph,
          chunk_idx,
          chunk,
          runtime_id,
          cached_relink.runtime_chunk_sig,
          &cached_relink.module_has_native_ns,
          &cached_relink.module_owner_digest,
          &cached_relink.index_chunk_id_to_name,
          &cached_relink.used_symbol_refs,
          Some(local_used_symbols),
        );
        cached_relink.chunk_decisions[chunk_idx] = decision;
        // The relinked records' certificates protect what they depend on from later updates,
        // as a whole generation's do (#621).
        cached_relink.decision_dependencies.extend(dependencies.iter().copied());
        cached_relink.protected_modules.extend(dependencies);
      }
    }

    {
      let ChunkGraph { chunk_table, module_to_chunk, post_chunk_optimization_operations, .. } =
        &mut cached_relink.chunk_graph;
      let chunk_assignments =
        ChunkAssignments::new(&*module_to_chunk, &*post_chunk_optimization_operations);
      for &(_module_idx, chunk_idx, _) in dirty_modules_with_used {
        let chunk = &mut chunk_table[chunk_idx];
        deconflict_chunk_symbols(
          chunk_idx,
          chunk,
          self.link_output,
          &cached_relink.order_state,
          self.options.format,
          &cached_relink.index_chunk_id_to_name,
          chunk_assignments,
          None,
        );
        if chunk.pre_rendered_chunk.is_none()
          && let Some(chunk_name) = chunk.name.clone()
        {
          chunk.pre_rendered_chunk =
            Some(generate_pre_rendered_chunk(chunk, &chunk_name, self.link_output));
        }
      }
    }

    let empty_file_urls = FxHashMap::default();
    let has_enum_inlining = self.link_output.has_enum_inlining;
    let mut dirty_rendered_modules = Vec::with_capacity(dirty_modules_with_used.len());
    let mut diagnostics = Vec::new();
    for &(idx, chunk_idx, _) in dirty_modules_with_used {
      let mut ast = self.ast_table[idx].as_ref().expect("normal module ast").clone_with_another_arena();
      let module = self.link_output.module_table[idx].as_normal().unwrap();
      let ast_scope = &self.link_output.symbol_db[idx].as_ref().unwrap().ast_scopes;
      let chunk = &cached_relink.chunk_graph.chunk_table[chunk_idx];
      let linking_info = &self.link_output.metas[module.idx];
      let ctx = crate::module_finalizers::ScopeHoistingFinalizerContext {
        idx,
        chunk,
        chunk_idx,
        file_idx: chunk_idx,
        chunk_graph: &cached_relink.chunk_graph,
        symbol_db: &self.link_output.symbol_db,
        linking_info,
        module,
        stmt_infos: &self.link_output.stmt_infos[idx],
        index_stmt_infos: &self.link_output.stmt_infos,
        modules: &self.link_output.module_table.modules,
        linking_infos: &self.link_output.metas,
        dynamic_import_usage: self.link_output.dynamic_import_exports_usage_map.get(&idx),
        order_wrap_state: &cached_relink.order_state,
        final_esm_init_metadata: &cached_relink.final_esm_init_metadata,
        used_symbol_refs: &cached_relink.used_symbol_refs,
        runtime: &self.link_output.runtime,
        options: self.options,
        file_emitter: &self.plugin_driver.file_emitter,
        constant_value_map: &self.link_output.global_constant_symbol_map,
        safely_merge_cjs_ns_map: &self.link_output.safely_merge_cjs_ns_map,
        retained_export_symbols: &self.link_output.retained_export_symbols,
        resolved_paths: self.resolved_paths.as_ref(),
        resolved_file_urls: &empty_file_urls,
        has_enum_inlining,
        unreachable_modules: &self.link_output.unreachable_modules,
        inline_state: &self.inline_state,
      };
      let (_, _, module_diagnostics) = ctx.finalize_normal_module(&mut ast, ast_scope);
      let has_module_error = module_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity() == rolldown_error::Severity::Error);
      diagnostics.extend(module_diagnostics);
      if !has_module_error {
        let rendered = self.render_normal_module(module, &ast);
        drop(ast);
        dirty_rendered_modules.push((chunk_idx, vec![Some(rendered)]));
      }
    }
    let has_error =
      diagnostics.iter().any(|diagnostic| diagnostic.severity() == rolldown_error::Severity::Error);
    self.link_output.diagnostics.extend(diagnostics);
    if has_error {
      let errors = self.link_output.diagnostics.extract_errors();
      return Err(errors.into());
    }

    self
      .render_dirty_chunks_to_assets(
        &cached_relink.chunk_graph,
        dirty_rendered_modules,
        &cached_relink.used_symbol_refs,
        &cached_relink.order_state,
        &cached_relink.chunk_decisions,
        &mut cached_relink.render_export_items_index_vec,
        &mut cached_relink.index_chunk_to_instances,
        &mut cached_relink.sorted_output_template,
        &cached_relink.chunk_idx_to_template_pos,
        &mut cached_relink.reused_chunk_filenames_pool,
        &mut cached_relink.excluded_from_reused_filenames,
        &mut cached_relink.pool_installed,
      )
      .await
      .map(|output| Ok((output, retired_filenames)))
  }

  /// Notices:
  /// - Should generate filenames that are stable cross builds and os.
  #[tracing::instrument(level = "debug", skip_all)]
  async fn generate_chunk_name_and_preliminary_filenames(
    &mut self,
    chunk_graph: &mut ChunkGraph,
  ) -> BuildResult<ChunkNamingResult> {
    let modules = &self.link_output.module_table.modules;

    let mut index_chunk_id_to_representative_name = FxHashMap::default();
    let can_cache_naming = self.options.preserve_modules
      && crate::utils::native_records::enabled(self.options)
      && self.cache.as_ref().is_some_and(|c| c.omit_reused_chunk_bytes);
    let used_name_counts = FxDashMap::default();
    let mut cached_by_chunk: FxHashMap<ChunkIdx, ModuleIdx> = FxHashMap::default();
    let mut candidate_order_sig = None;
    let mut order_matches_previous = false;
    if can_cache_naming && let Some(cache) = self.cache.as_ref() {
      use std::hash::Hash;
      use xxhash_rust::xxh3::Xxh3;
      let mut h0 = Xxh3::with_seed(0x243f6a8885a308d3);
      let mut h1 = Xxh3::with_seed(0x13198a2e03707344);
      chunk_graph.sorted_chunk_idx_vec.len().hash(&mut h0);
      chunk_graph.sorted_chunk_idx_vec.len().hash(&mut h1);
      let mut order_sig_valid = true;
      let mut all_cached = true;
      for chunk_id in &chunk_graph.sorted_chunk_idx_vec {
        let chunk = &chunk_graph.chunk_table[*chunk_id];
        if chunk.preliminary_filename.is_some() {
          (0u8, chunk.preliminary_filename.as_ref().map(|f| f.as_str())).hash(&mut h0);
          (0u8, chunk.preliminary_filename.as_ref().map(|f| f.as_str())).hash(&mut h1);
          continue;
        }
        let [module_idx] = chunk.modules.as_slice() else {
          order_sig_valid = false;
          all_cached = false;
          continue;
        };
        let module_idx = *module_idx;
        let is_user_defined = match chunk.kind {
          ChunkKind::EntryPoint { module: entry_module_id, meta, .. } => {
            if entry_module_id != module_idx {
              order_sig_valid = false;
              all_cached = false;
              continue;
            }
            meta.contains(rolldown_common::ChunkMeta::UserDefinedEntry)
          }
          ChunkKind::Common => false,
        };
        let key = (
          1u8,
          module_idx.raw(),
          chunk.input_base.as_str(),
          chunk.name.as_deref(),
          is_user_defined,
        );
        key.hash(&mut h0);
        key.hash(&mut h1);
        if let Some(naming) = cache.cached_chunk_namings.get(&module_idx)
          && naming.input_base == chunk.input_base
          && naming.explicit_name == chunk.name
          && naming.is_user_defined_entry == is_user_defined
        {
          cached_by_chunk.insert(*chunk_id, module_idx);
        } else {
          all_cached = false;
        }
      }
      if order_sig_valid {
        let sig = [h0.digest128(), h1.digest128()];
        candidate_order_sig = Some(sig);
        order_matches_previous = all_cached && cache.naming_order_signature == Some(sig);
      }
    }

    let options = self.options;
    let runtime_id = self.link_output.runtime.id();
    let mut index_pre_generated_names: IndexVec<ChunkIdx, PreGeneratedChunkName> =
      if order_matches_previous {
        IndexVec::default()
      } else {
        let cache_ref = self.cache.as_deref();
        let index_pre_generated_names_futures =
          chunk_graph.chunk_table.iter_enumerated().map(|(chunk_idx, chunk)| {
            let is_record = self.inline_state.is_record(chunk_idx);
            let cached_pre = cached_by_chunk
              .get(&chunk_idx)
              .and_then(|m_idx| cache_ref?.cached_chunk_namings.get(m_idx))
              .map(|c| PreGeneratedChunkName {
                representative_chunk_name: c.representative_chunk_name.clone(),
                chunk_name: c.chunk_name.clone(),
                chunk_filename: c.chunk_filename.clone(),
              });
            let already_named = chunk.preliminary_filename.is_some()
              || chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx);
            let sanitize_filename = options.sanitize_filename.clone();
            let input_base = chunk.input_base.clone();
            let preserve_modules = options.preserve_modules;
            async move {
              if already_named {
                return anyhow::Ok(PreGeneratedChunkName::default());
              }
              if let Some(cached_pre) = cached_pre {
                return anyhow::Ok(cached_pre);
              }
              if let Some(name) = &chunk.name {
                let name = sanitize_filename.call(name).await?;
                return anyhow::Ok(PreGeneratedChunkName {
                  representative_chunk_name: name.clone(),
                  chunk_name: name.clone(),
                  chunk_filename: name,
                });
              }
              match chunk.kind {
                ChunkKind::EntryPoint { module: entry_module_id, meta, .. } => {
                  let module = &modules[entry_module_id];
                  let generated = if preserve_modules {
                    preserve_modules_chunk_name(options, module.id().as_str(), input_base.as_str()).await?
                  } else if meta.contains(rolldown_common::ChunkMeta::UserDefinedEntry) {
                    // try extract meaningful input name from path
                    if let Some(file_stem) =
                      module.id().as_str().as_path().file_stem().and_then(|f| f.to_str())
                    {
                      let name = sanitize_filename.call(file_stem).await?;
                      PreGeneratedChunkName {
                        chunk_name: name.clone(),
                        representative_chunk_name: name.clone(),
                        chunk_filename: name,
                      }
                    } else {
                      let name = arcstr::literal!("input");
                      PreGeneratedChunkName {
                        representative_chunk_name: name.clone(),
                        chunk_name: name.clone(),
                        chunk_filename: name,
                      }
                    }
                  } else {
                    let chunk_name = sanitize_filename.call(&module.id().representative_name()).await?;

                    PreGeneratedChunkName {
                      representative_chunk_name: chunk_name.clone(),
                      chunk_name: chunk_name.clone(),
                      chunk_filename: chunk_name,
                    }
                  };
                  Ok(generated)
                }
                ChunkKind::Common => {
                  // - rollup use the first entered/last executed module as the `[name]` of common chunks.
                  // - esbuild always use 'chunk' as the `[name]`. However we try to make the name more meaningful here.
                  if let Some(module_id) =
                    chunk.modules.iter().rev().find(|each| **each != runtime_id)
                  {
                    let module = &modules[*module_id];
                    let name = module.id().representative_name();
                    // A record is no file, so the `sanitizeFileName` hook does not run for it.
                    let sanitized_filename = if is_record {
                      ArcStr::from(name.as_ref())
                    } else {
                      sanitize_filename.call(&name).await?
                    };
                    Ok(PreGeneratedChunkName {
                      representative_chunk_name: sanitized_filename.clone(),
                      chunk_name: sanitized_filename.clone(),
                      chunk_filename: sanitized_filename,
                    })
                  } else {
                    let name = arcstr::literal!("chunk");
                    Ok(PreGeneratedChunkName {
                      representative_chunk_name: name.clone(),
                      chunk_name: name.clone(),
                      chunk_filename: name,
                    })
                  }
                }
              }
            }
          });
        try_join_all(index_pre_generated_names_futures).await?.into()
      };
    let index_record_module_ids: FxHashMap<ChunkIdx, Vec<rolldown_common::ModuleId>> = self
      .inline_state
      .record_indices()
      .map(|record_idx| {
        let module_ids = chunk_graph.chunk_table[record_idx]
          .modules
          .iter()
          .map(|module_idx| modules[*module_idx].id().as_str().into())
          .collect();
        (record_idx, module_ids)
      })
      .collect();

    let mut hash_placeholder_generator = HashPlaceholderGenerator::default();

    let output_dir = absolutize_path_buf(self.options.cwd.join(&self.options.out_dir));
    let mut newly_cached_namings: Vec<(ModuleIdx, CachedChunkNaming)> = Vec::new();
    let mut reused_naming_count = 0usize;
    let mut record_ids = Vec::new();
    let mut taken_record_ids = FxHashSet::default();

    for chunk_id in &chunk_graph.sorted_chunk_idx_vec {
      let chunk = &mut chunk_graph.chunk_table[*chunk_id];
      // `preliminary_filename` is the single source of truth for "already generated": it is
      // written only here, together with `preliminary_sourcemap_filename`.
      if chunk.preliminary_filename.is_some() {
        reused_naming_count += 1;
        continue;
      }

      if let Some(m_idx) = cached_by_chunk.get(chunk_id)
        && let Some(cached) = self.cache.as_ref().and_then(|c| c.cached_chunk_namings.get(m_idx))
      {
        let can_reuse_directly = if order_matches_previous {
          true
        } else if let Some(keys) = &cached.used_name_keys
          && keys.iter().all(|k| !used_name_counts.contains_key(k.as_str()))
        {
          for k in keys {
            used_name_counts.insert(k.clone(), 2);
          }
          true
        } else {
          false
        };
        if can_reuse_directly {
          index_chunk_id_to_representative_name
            .insert(*chunk_id, cached.representative_chunk_name.clone());
          chunk.name = Some(cached.chunk_name.clone());
          chunk.absolute_preliminary_filename = Some(cached.absolute_preliminary_filename.clone());
          chunk.preliminary_filename = Some(cached.preliminary_filename.clone());
          chunk.preliminary_sourcemap_filename = cached.preliminary_sourcemap_filename.clone();
          reused_naming_count += 1;
          continue;
        }
      }

      let used_before = used_name_counts.len();
      let explicit_name = chunk.name.clone();
      let pre_generated_chunk_name = &mut index_pre_generated_names[*chunk_id];
      if !pre_generated_chunk_name.chunk_filename.is_ascii()
        && !self.options.preserve_modules
        && matches!(
          &self.options.chunk_filenames,
          rolldown_common::ChunkFilenamesOutputOption::String(tpl) if !tpl.contains("[hash")
        )
      {
        use std::fmt::Write as _;
        use xxhash_rust::xxh3::xxh3_64;
        let digest = xxh3_64(pre_generated_chunk_name.chunk_filename.as_bytes());
        let mut ascii_name = String::with_capacity(65);
        for ch in pre_generated_chunk_name.chunk_filename.chars() {
          if ascii_name.len() >= 48 {
            break;
          }
          ascii_name.push(if ch.is_ascii() { ch } else { '_' });
        }
        let _ = write!(ascii_name, "_{digest:016x}");
        pre_generated_chunk_name.chunk_filename = ArcStr::from(ascii_name);
      }
      // Notice we didn't used deconflict name here, chunk names are allowed to be duplicated.
      index_chunk_id_to_representative_name
        .insert(*chunk_id, pre_generated_chunk_name.representative_chunk_name.clone());
      if self.inline_state.is_record(*chunk_id) {
        // A record is no file: no filename template runs and nothing below is assigned. Its id
        // follows the chunk name and the record's module set, see `record_id`.
        let chunk_name = pre_generated_chunk_name.chunk_name.clone();
        let id = inline_common_chunks::record_id(
          &chunk_name,
          chunk.modules.iter().map(|module_idx| modules[*module_idx].stable_id().as_str()),
          &mut taken_record_ids,
        );
        record_ids.push((*chunk_id, id));
        chunk.name = Some(chunk_name);
        continue;
      }
      let mut pre_rendered_chunk =
        generate_pre_rendered_chunk(chunk, &pre_generated_chunk_name.chunk_name, self.link_output);
      // The modules of every carried record are printed into this file, so `chunkFileNames` and
      // the rendered chunk see them among `moduleIds`.
      for record_idx in self.inline_state.carried_by(*chunk_id) {
        pre_rendered_chunk
          .module_ids
          .extend(index_record_module_ids.get(record_idx).into_iter().flatten().cloned());
      }
      let preliminary_filename = chunk
        .generate_preliminary_filename(
          self.options,
          &pre_rendered_chunk,
          &pre_generated_chunk_name.chunk_filename,
          &mut hash_placeholder_generator,
          &used_name_counts,
        )
        .await?;
      // Defer chunk name assignment to make sure at this point only entry chunk have a name
      // if user provided one.
      chunk.name = Some(pre_generated_chunk_name.chunk_name.clone());

      let absolute_preliminary_filename =
        preliminary_filename.absolutize_with(&output_dir).into_owned().expect_into_string();
      chunk.absolute_preliminary_filename = Some(absolute_preliminary_filename.clone());
      chunk.preliminary_filename = Some(preliminary_filename.clone());
      // Derives its `[chunkhash]` from the just-assigned `preliminary_filename`, so it must run
      // after it.
      let preliminary_sourcemap_filename = chunk
        .generate_preliminary_sourcemap_filename(
          self.options,
          &pre_rendered_chunk,
          &pre_generated_chunk_name.chunk_filename,
          &mut hash_placeholder_generator,
          &used_name_counts,
        )
        .await?;
      let cacheable_kind = match (chunk.modules.as_slice(), &chunk.kind) {
        ([module_idx], ChunkKind::EntryPoint { module: entry_module_id, meta, .. })
          if *entry_module_id == *module_idx =>
        {
          Some((*module_idx, meta.contains(rolldown_common::ChunkMeta::UserDefinedEntry)))
        }
        ([module_idx], ChunkKind::Common) => Some((*module_idx, false)),
        _ => None,
      };
      if can_cache_naming
        && let Some((module_idx, is_user_defined_entry)) = cacheable_kind
        && preliminary_filename.hash_placeholder().is_none()
        && preliminary_sourcemap_filename.as_ref().is_none_or(|f| f.hash_placeholder().is_none())
        && let Some(cache) = self.cache.as_ref()
      {
        let key = ArcStr::from(preliminary_filename.as_str().to_ascii_lowercase());
        let sm_key = preliminary_sourcemap_filename
          .as_ref()
          .map(|sm| ArcStr::from(sm.as_str().to_ascii_lowercase()));
        let expected_added = 1 + usize::from(sm_key.as_ref().is_some_and(|sk| sk != &key));
        let used_name_keys = (self.options.file.is_none()
          && chunk.file_name.is_none()
          && used_name_counts.len() == used_before + expected_added
          && used_name_counts.get(key.as_str()).is_some_and(|c| *c == 2)
          && sm_key
            .as_ref()
            .is_none_or(|sk| sk != &key && used_name_counts.get(sk.as_str()).is_some_and(|c| *c == 2)))
        .then(|| {
          let mut keys = vec![key];
          if let Some(sk) = sm_key {
            keys.push(sk);
          }
          keys
        });
        newly_cached_namings.push((
          module_idx,
          CachedChunkNaming {
            scan_generation: cache.module_scan_generation(module_idx),
            input_base: chunk.input_base.clone(),
            explicit_name,
            is_user_defined_entry,
            used_name_keys,
            representative_chunk_name: pre_generated_chunk_name.representative_chunk_name.clone(),
            chunk_name: pre_generated_chunk_name.chunk_name.clone(),
            chunk_filename: pre_generated_chunk_name.chunk_filename.clone(),
            preliminary_filename: preliminary_filename.clone(),
            absolute_preliminary_filename,
            preliminary_sourcemap_filename: preliminary_sourcemap_filename.clone(),
          },
        ));
      }
      chunk.preliminary_sourcemap_filename = preliminary_sourcemap_filename;
      chunk.pre_rendered_chunk = Some(pre_rendered_chunk);
    }
    if let Some(cache) = self.cache.as_deref_mut() {
      let build_collision_free = used_name_counts.iter().all(|e| *e.value() == 2);
      let all_cached_after = order_matches_previous
        || reused_naming_count + newly_cached_namings.len() == chunk_graph.sorted_chunk_idx_vec.len();
      for (module_idx, mut naming) in newly_cached_namings {
        if !build_collision_free {
          naming.used_name_keys = None;
        }
        cache.cached_chunk_namings.insert(module_idx, naming);
      }
      if can_cache_naming {
        cache.naming_order_signature = all_cached_after.then_some(candidate_order_sig).flatten();
      }
    }
    Ok((index_chunk_id_to_representative_name, record_ids))
  }

  #[expect(clippy::type_complexity)]
  fn compute_native_chunk_decisions(
    &self,
    chunk_graph: &ChunkGraph,
    resolved_file_urls: &resolve_file_urls::ResolvedFileUrls,
    used_symbol_refs: &UsedSymbolRefs,
    order_state: &order_wrap_state::OrderWrapState,
    final_esm_init_metadata: &Sealed<FinalEsmInitMetadata>,
    index_chunk_id_to_name: &FxHashMap<ChunkIdx, ArcStr>,
  ) -> (
    IndexVec<ChunkIdx, Option<[u128; 2]>>,
    FxHashSet<ChunkIdx>,
    Option<(
      Option<[u128; 2]>,
      IndexVec<ModuleIdx, bool>,
      IndexVec<ModuleIdx, [u128; 2]>,
      FxHashSet<ModuleIdx>,
      FxHashSet<ModuleIdx>,
    )>,
  ) {
    use rolldown_utils::index_vec_ext::IndexVecRefExt as _;
    use std::hash::Hash;
    use xxhash_rust::xxh3::Xxh3;

    let empty = (
      IndexVec::from_vec(vec![None; chunk_graph.chunk_table.len()]),
      FxHashSet::default(),
      None,
    );
    let Some(cache) = self.cache.as_ref() else {
      return empty;
    };
    if !cache.omit_reused_chunk_bytes
      || !crate::utils::native_records::enabled(self.options)
      || !self.options.experimental.is_incremental_build_enabled()
      || self.options.banner.is_some()
      || self.options.footer.is_some()
      || self.options.intro.is_some()
      || self.options.outro.is_some()
      || self.options.post_banner.is_some()
      || self.options.post_footer.is_some()
      || self.options.experimental.is_attach_debug_info_full()
      || self.options.sourcemap_debug_ids
      || !resolved_file_urls.is_empty()
    {
      return empty;
    }

    struct DualHasher(Xxh3, Xxh3);
    impl DualHasher {
      fn new() -> Self {
        Self(Xxh3::with_seed(0x243f6a8885a308d3), Xxh3::with_seed(0x13198a2e03707344))
      }
      fn write_val<T: Hash + ?Sized>(&mut self, val: &T) {
        val.hash(&mut self.0);
        val.hash(&mut self.1);
      }
      fn write_unordered(&mut self, tag: u8, len: usize, acc: (u128, u128)) {
        self.write_val(&(tag, len, acc.0, acc.1));
      }
      fn finish(self) -> [u128; 2] {
        [self.0.digest128(), self.1.digest128()]
      }
    }

    let hash_const_export_meta =
      |h: &mut DualHasher, meta: Option<&rolldown_common::ConstExportMeta>| match meta {
        None => h.write_val(&0u8),
        Some(meta) => {
          h.write_val(&(1u8, meta.commonjs_export, meta.safe_to_inline));
          match &meta.value {
            rolldown_common::ConstantValue::Number(n) => h.write_val(&(1u8, n.to_bits())),
            rolldown_common::ConstantValue::BigInt(s) => h.write_val(&(2u8, s)),
            rolldown_common::ConstantValue::String(s) => h.write_val(&(3u8, s.as_str())),
            rolldown_common::ConstantValue::Boolean(b) => h.write_val(&(4u8, *b)),
            rolldown_common::ConstantValue::Undefined => h.write_val(&5u8),
            rolldown_common::ConstantValue::Null => h.write_val(&6u8),
          }
        }
      };

    let sym_id_key = |s: rolldown_common::SymbolRef| -> (u32, u32, &str) {
      if self.link_output.symbol_db.is_facade_symbol(s) {
        (s.owner.raw(), u32::MAX, s.name(&self.link_output.symbol_db))
      } else {
        (s.owner.raw(), s.symbol.index() as u32, s.name(&self.link_output.symbol_db))
      }
    };

    let runtime_id = self.link_output.runtime.id();
    let runtime_chunk_idx = chunk_graph.module_to_chunk[runtime_id];
    let runtime_chunk_sig = runtime_chunk_idx.and_then(|rt_idx| {
      let rt_chunk = chunk_graph.chunk_table.get(rt_idx)?;
      let mut h = DualHasher::new();
      h.write_val(&rt_chunk.preliminary_filename.as_ref().map(|f| f.as_str()));
      let mut exp_acc = (0u128, 0u128);
      for (sym_ref, names) in &rt_chunk.exports_to_other_chunks {
        let mut eh = DualHasher::new();
        eh.write_val(&(sym_id_key(*sym_ref), names));
        let [d0, d1] = eh.finish();
        exp_acc = (exp_acc.0.wrapping_add(d0), exp_acc.1.wrapping_add(d1));
      }
      h.write_unordered(1, rt_chunk.exports_to_other_chunks.len(), exp_acc);
      Some(h.finish())
    });

    let module_has_native_ns: IndexVec<ModuleIdx, bool> = self
      .link_output
      .module_table
      .modules
      .iter_enumerated()
      .map(|(idx, m)| {
        m.as_normal().is_some_and(|normal| {
          crate::utils::native_records::has_native_namespace(
            self.options,
            normal,
            &self.link_output.metas[idx],
            &self.link_output.symbol_db,
          )
        })
      })
      .collect();

    let module_owner_digest: IndexVec<ModuleIdx, [u128; 2]> = self
      .link_output
      .module_table
      .modules
      .par_iter_enumerated()
      .map(|(idx, m)| {
        let mut mh = DualHasher::new();
        mh.write_val(&m.stable_id().as_str());
        if let Some(normal) = m.as_normal() {
          let ometa = &self.link_output.metas[idx];
          let chunk_file = chunk_graph.module_to_chunk[idx]
            .and_then(|cidx| chunk_graph.chunk_table.get(cidx))
            .and_then(|c| c.preliminary_filename.as_ref().map(|f| f.as_str()));
          let entry_chunk_file = chunk_graph
            .entry_module_to_entry_chunk
            .get(&idx)
            .and_then(|cidx| chunk_graph.chunk_table.get(*cidx))
            .and_then(|c| c.preliminary_filename.as_ref().map(|f| f.as_str()));
          let init_target = order_state.esm_init_target(idx, ometa).map(|t| {
            (sym_id_key(t.wrapper_ref), t.tla_tainted, t.origin as u8)
          });
          let init_noop = final_esm_init_metadata.init_is_noop(idx);
          let hash_exported_sym = |mh: &mut DualHasher, sref: rolldown_common::SymbolRef| {
            let cref = self.link_output.symbol_db.canonical_ref_for(sref);
            let sym = self.link_output.symbol_db.get(cref);
            let owner_chunk_file = sym
              .chunk_idx
              .and_then(|cidx| chunk_graph.chunk_table.get(cidx))
              .and_then(|c| c.preliminary_filename.as_ref().map(|f| f.as_str()));
            let ns_alias = sym.namespace_alias.as_ref().map(|na| {
              let ns_canon = self.link_output.symbol_db.canonical_ref_for(na.namespace_ref);
              (na.property_name.as_str(), sym_id_key(ns_canon))
            });
            mh.write_val(&(
              sym_id_key(sref),
              sym_id_key(cref),
              used_symbol_refs.contains(&sref),
              used_symbol_refs.contains(&cref),
              self.link_output.retained_export_symbols.contains(&sref),
              self.link_output.retained_export_symbols.contains(&cref),
            ));
            mh.write_val(&(
              self.link_output.symbol_db.is_facade_symbol(sref),
              self.link_output.symbol_db.is_facade_symbol(cref),
              sref.is_not_reassigned(&self.link_output.symbol_db),
              cref.is_not_reassigned(&self.link_output.symbol_db),
              owner_chunk_file,
              ns_alias,
            ));
            hash_const_export_meta(mh, self.link_output.global_constant_symbol_map.get(&sref));
            hash_const_export_meta(mh, self.link_output.global_constant_symbol_map.get(&cref));
          };
          mh.write_val(&(
            normal.exports_kind as u8,
            ometa.wrap_kind() as u8,
            ometa.is_included,
            ometa.is_tla_or_contains_tla_dependency,
            ometa.has_dynamic_exports,
            module_has_native_ns[idx],
            self.link_output.has_enum_inlining,
            chunk_file,
            entry_chunk_file,
            init_target,
            init_noop,
          ));
          if !normal.ecma_view.enum_member_value_map.is_empty() {
            let mut enum_acc = (0u128, 0u128);
            let mut enum_count = 0usize;
            for (enum_sym, members) in &normal.ecma_view.enum_member_value_map {
              for (prop, meta) in members {
                let mut eh = DualHasher::new();
                eh.write_val(&(enum_sym.as_str(), prop.as_str()));
                hash_const_export_meta(&mut eh, Some(meta));
                let [d0, d1] = eh.finish();
                enum_acc = (enum_acc.0.wrapping_add(d0), enum_acc.1.wrapping_add(d1));
                enum_count += 1;
              }
            }
            mh.write_unordered(10, enum_count, enum_acc);
          }
          hash_exported_sym(&mut mh, normal.namespace_object_ref);
          hash_exported_sym(&mut mh, normal.default_export_ref);
          if let Some(wref) = ometa.wrapper_ref {
            hash_exported_sym(&mut mh, wref);
          }
          if idx == runtime_id
            && let Some(db) = self.link_output.symbol_db[idx].as_ref()
          {
            for sym_id in db.classic_data.indices() {
              let sref: rolldown_common::SymbolRef = (idx, sym_id).into();
              if !self.link_output.symbol_db.is_facade_symbol(sref) {
                hash_exported_sym(&mut mh, sref);
              }
            }
          }
          let mut rexp_acc = (0u128, 0u128);
          for (name, export) in &ometa.resolved_exports {
            let mut eh = DualHasher::new();
            let non_ambig = ometa.sorted_and_non_ambiguous_resolved_exports.get(name).copied();
            eh.write_val(&(name.as_str(), export.came_from_commonjs, non_ambig));
            hash_exported_sym(&mut eh, export.symbol_ref);
            let [d0, d1] = eh.finish();
            rexp_acc = (rexp_acc.0.wrapping_add(d0), rexp_acc.1.wrapping_add(d1));
          }
          mh.write_unordered(9, ometa.resolved_exports.len(), rexp_acc);
          mh.write_val(
            &self
              .link_output
              .safely_merge_cjs_ns_map
              .get(&idx)
              .map(|info| info.needs_interop),
          );
        }
        mh.finish()
      })
      .collect::<Vec<_>>()
      .into();

    let decisions_and_reuse: Vec<(Option<[u128; 2]>, bool, Vec<ModuleIdx>)> = chunk_graph
      .chunk_table
      .par_iter_enumerated()
      .map(|(chunk_idx, chunk)| {
        self.compute_single_native_chunk_decision(
          cache,
          chunk_graph,
          chunk_idx,
          chunk,
          runtime_id,
          runtime_chunk_sig,
          &module_has_native_ns,
          &module_owner_digest,
          index_chunk_id_to_name,
          used_symbol_refs,
          None,
        )
      })
      .collect();

    let mut decisions = IndexVec::with_capacity(decisions_and_reuse.len());
    let mut reused = FxHashSet::default();
    let mut protected_modules = cache.native_cell_modules.clone();
    protected_modules.extend(cache.export_star_modules.iter().copied());
    for &star_idx in &cache.export_star_modules {
      if let Some(m) = self.link_output.module_table.modules.get(star_idx) {
        for rec in m.import_records() {
          if let Some(target_idx) = rec.resolved_module {
            protected_modules.insert(target_idx);
          }
        }
      }
    }
    let mut decision_dependencies = FxHashSet::default();
    for (idx, (decision, is_reused, deps)) in decisions_and_reuse.into_iter().enumerate() {
      let chunk_idx = ChunkIdx::from_usize(idx);
      decisions.push(decision);
      if is_reused {
        reused.insert(chunk_idx);
      }
      decision_dependencies.extend(deps.iter().copied());
      protected_modules.extend(deps);
    }
    (
      decisions,
      reused,
      Some((
        runtime_chunk_sig,
        module_has_native_ns,
        module_owner_digest,
        protected_modules,
        decision_dependencies,
      )),
    )
  }

  #[expect(clippy::too_many_arguments, clippy::too_many_lines)]
  pub(crate) fn compute_single_native_chunk_decision(
    &self,
    cache: &ScanStageCache,
    chunk_graph: &ChunkGraph,
    chunk_idx: ChunkIdx,
    chunk: &rolldown_common::Chunk,
    runtime_id: ModuleIdx,
    runtime_chunk_sig: Option<[u128; 2]>,
    module_has_native_ns: &IndexVec<ModuleIdx, bool>,
    module_owner_digest: &IndexVec<ModuleIdx, [u128; 2]>,
    index_chunk_id_to_name: &FxHashMap<ChunkIdx, ArcStr>,
    used_symbol_refs: &UsedSymbolRefs,
    local_used_override: Option<&FxHashSet<rolldown_common::SymbolRef>>,
  ) -> (Option<[u128; 2]>, bool, Vec<ModuleIdx>) {
    use std::hash::Hash;
    use xxhash_rust::xxh3::Xxh3;

    struct DualHasher(Xxh3, Xxh3);
    impl DualHasher {
      fn new() -> Self {
        Self(Xxh3::with_seed(0x243f6a8885a308d3), Xxh3::with_seed(0x13198a2e03707344))
      }
      fn write_val<T: Hash + ?Sized>(&mut self, val: &T) {
        val.hash(&mut self.0);
        val.hash(&mut self.1);
      }
      fn write_unordered(&mut self, tag: u8, len: usize, acc: (u128, u128)) {
        self.write_val(&(tag, len, acc.0, acc.1));
      }
      fn finish(self) -> [u128; 2] {
        [self.0.digest128(), self.1.digest128()]
      }
    }

    let hash_const_export_meta =
      |h: &mut DualHasher, meta: Option<&rolldown_common::ConstExportMeta>| match meta {
        None => h.write_val(&0u8),
        Some(meta) => {
          h.write_val(&(1u8, meta.commonjs_export, meta.safe_to_inline));
          match &meta.value {
            rolldown_common::ConstantValue::Number(n) => h.write_val(&(1u8, n.to_bits())),
            rolldown_common::ConstantValue::BigInt(s) => h.write_val(&(2u8, s)),
            rolldown_common::ConstantValue::String(s) => h.write_val(&(3u8, s.as_str())),
            rolldown_common::ConstantValue::Boolean(b) => h.write_val(&(4u8, *b)),
            rolldown_common::ConstantValue::Undefined => h.write_val(&5u8),
            rolldown_common::ConstantValue::Null => h.write_val(&6u8),
          }
        }
      };

    let sym_id_key = |s: rolldown_common::SymbolRef| -> (u32, u32, &str) {
      if self.link_output.symbol_db.is_facade_symbol(s) {
        (s.owner.raw(), u32::MAX, s.name(&self.link_output.symbol_db))
      } else {
        (s.owner.raw(), s.symbol.index() as u32, s.name(&self.link_output.symbol_db))
      }
    };

    if chunk.modules.len() != 1
      || !chunk.insert_map.is_empty()
      || !chunk.remove_map.is_empty()
      || !chunk.module_groups.is_empty()
      || chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx)
    {
      return (None, false, Vec::new());
    }
    let Some(preliminary_filename) = chunk.preliminary_filename.as_ref() else {
      return (None, false, Vec::new());
    };
    if preliminary_filename.hash_placeholder().is_some()
      || chunk
        .preliminary_sourcemap_filename
        .as_ref()
        .is_some_and(|f| f.hash_placeholder().is_some())
    {
      return (None, false, Vec::new());
    }
    let module_idx = chunk.modules[0];
    let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
      return (None, false, Vec::new());
    };
    if !module.ecma_view.mutations.is_empty()
      || module.ecma_view.json_module_none_self_reference_included_symbol.is_some()
    {
      return (None, false, Vec::new());
    }

    let scan_gen = cache.module_scan_generation(module_idx);
    if scan_gen == 0 {
      return (None, false, Vec::new());
    }

    let is_sym_used = |s: rolldown_common::SymbolRef| -> bool {
      if let Some(local_used) = local_used_override
        && s.owner == module_idx
        && s != module.namespace_object_ref
      {
        local_used.contains(&s)
      } else {
        used_symbol_refs.contains(&s)
      }
    };

    let mut protected_deps = Vec::new();
    let symbol_db = &self.link_output.symbol_db;
    let metas = &self.link_output.metas;
    let meta = &metas[module_idx];
    let mut h = DualHasher::new();
    h.write_val(&(
      scan_gen,
      module_idx.raw(),
      module.ecma_view.dummy_record_set.len(),
      preliminary_filename.as_str(),
      chunk.preliminary_sourcemap_filename.as_ref().map(|f| f.as_str()),
      chunk.name.as_deref(),
      chunk.input_base.as_str(),
      runtime_chunk_sig,
    ));
    match &chunk.kind {
      ChunkKind::EntryPoint { meta: cmeta, module: emod, .. } => {
        h.write_val(&(1u8, cmeta.bits(), emod.raw()));
      }
      ChunkKind::Common => {
        h.write_val(&0u8);
      }
    }
    h.write_val(&chunk.entry_level_external_module_idx.len());
    for &ext_idx in &chunk.entry_level_external_module_idx {
      let ext_path = self.link_output.module_table[ext_idx]
        .as_external()
        .map(|e| (e.get_import_path(chunk, self.resolved_paths.as_ref()), e.get_file_name(self.resolved_paths.as_ref())));
      h.write_val(&(ext_idx.raw(), ext_path));
    }
    h.write_val(&chunk.direct_imports_from_external_modules.len());
    for (ext_idx, named_imports) in &chunk.direct_imports_from_external_modules {
      let ext_path = self.link_output.module_table[*ext_idx]
        .as_external()
        .map(|e| (e.get_import_path(chunk, self.resolved_paths.as_ref()), e.get_file_name(self.resolved_paths.as_ref())));
      h.write_val(&(ext_idx.raw(), ext_path, named_imports.len()));
    }
    h.write_val(&chunk.dynamic_imports_from_external_modules.len());
    for &ext_idx in &chunk.dynamic_imports_from_external_modules {
      let ext_path = self.link_output.module_table[ext_idx]
        .as_external()
        .map(|e| (e.get_import_path(chunk, self.resolved_paths.as_ref()), e.get_file_name(self.resolved_paths.as_ref())));
      h.write_val(&(ext_idx.raw(), ext_path));
    }
    h.write_val(&chunk.import_symbol_from_external_modules.len());
    for &ext_idx in &chunk.import_symbol_from_external_modules {
      let ext_path = self.link_output.module_table[ext_idx]
        .as_external()
        .map(|e| (e.get_import_path(chunk, self.resolved_paths.as_ref()), e.get_file_name(self.resolved_paths.as_ref())));
      h.write_val(&(ext_idx.raw(), ext_path));
    }

    if self.options.is_dev_mode_enabled() {
      let rows = crate::utils::native_records::dev_graph_rows(
        chunk_idx,
        chunk,
        &self.link_output.module_table,
        chunk_graph,
        &self.link_output.metas,
        crate::utils::native_records::enabled(self.options),
      );
      h.write_val(&rows.len());
      for &row_idx in rows.iter().skip(1) {
        let row_mod = &self.link_output.module_table[row_idx];
        let partially_accepting = row_mod
          .as_normal()
          .is_some_and(rolldown_common::NormalModule::is_hmr_partially_accepting_module);
        h.write_val(&(row_idx.raw(), row_mod.stable_id().as_str(), partially_accepting));
        let bindings = row_mod
          .as_normal()
          .map(crate::hmr::module_graph_delta::static_import_bindings);
        for (rec_idx, rec) in row_mod.import_records().iter_enumerated() {
          let target_stable =
            rec.resolved_module.map(|t| self.link_output.module_table[t].stable_id().as_str());
          let target_partially_accepting = rec
            .resolved_module
            .and_then(|t| self.link_output.module_table[t].as_normal())
            .is_some_and(rolldown_common::NormalModule::is_hmr_partially_accepting_module);
          h.write_val(&(
            rec.kind as u8,
            rec.meta.bits(),
            target_stable,
            target_partially_accepting,
            target_partially_accepting
              .then(|| bindings.as_ref().and_then(|b| b.get(&rec_idx))),
          ));
        }
      }
    }

    h.write_val(&chunk.imports_from_other_chunks.len());
    for (exporter_chunk_idx, items) in &chunk.imports_from_other_chunks {
      let exporter_chunk = &chunk_graph.chunk_table[*exporter_chunk_idx];
      h.write_val(&(
        exporter_chunk.preliminary_filename.as_ref().map(|f| f.as_str()),
        index_chunk_id_to_name.get(exporter_chunk_idx).map(|n| n.as_str()),
        exporter_chunk.entry_module_idx().map(|i| i.raw()),
        items.len(),
      ));
      for item in items {
        let canonical_ref = symbol_db.canonical_ref_for(item.import_ref);
        let is_ns = canonical_ref.owner != runtime_id
          && module_has_native_ns[canonical_ref.owner]
          && self.link_output.module_table[canonical_ref.owner]
            .as_normal()
            .is_some_and(|m| m.namespace_object_ref == canonical_ref);
        let export_aliases = exporter_chunk.exports_to_other_chunks.get(&item.import_ref);
        h.write_val(&(
          sym_id_key(item.import_ref),
          sym_id_key(canonical_ref),
          is_ns,
          export_aliases,
        ));
      }
    }

    h.write_val(&chunk.cross_chunk_imports.len());
    for &importee_chunk_idx in &chunk.cross_chunk_imports {
      let importee_chunk = &chunk_graph.chunk_table[importee_chunk_idx];
      h.write_val(&importee_chunk.preliminary_filename.as_ref().map(|f| f.as_str()));
    }

    h.write_val(&chunk.cross_chunk_dynamic_imports.len());
    for &importee_chunk_idx in &chunk.cross_chunk_dynamic_imports {
      let importee_chunk = &chunk_graph.chunk_table[importee_chunk_idx];
      h.write_val(&importee_chunk.preliminary_filename.as_ref().map(|f| f.as_str()));
    }

    let mut exp_acc = (0u128, 0u128);
    for (export_ref, aliases) in &chunk.exports_to_other_chunks {
      let mut eh = DualHasher::new();
      let canonical_ref = symbol_db.canonical_ref_for(*export_ref);
      eh.write_val(&(
        sym_id_key(*export_ref),
        sym_id_key(canonical_ref),
        aliases,
      ));
      let [d0, d1] = eh.finish();
      exp_acc = (exp_acc.0.wrapping_add(d0), exp_acc.1.wrapping_add(d1));
    }
    h.write_unordered(5, chunk.exports_to_other_chunks.len(), exp_acc);

    h.write_val(&(
      meta.is_included,
      meta.wrap_kind() as u8,
      meta.wrapper_ref.map(&sym_id_key),
      meta.wrapper_stmt_info.map(|s| s.raw()),
      meta.concatenated_wrapped_module_kind as u8,
      meta.is_tla_or_contains_tla_dependency,
      meta.has_dynamic_exports,
      meta.module_namespace_included_reason.bits(),
      meta.namespace_included,
      meta.depended_runtime_helper.bits(),
      meta.native_default_expression,
      self.link_output.stmt_infos[module_idx].len(),
    ));
    // A JSON module's importers that the graph reaches decide whether its namespace holds its
    // properties.
    h.write_val(&crate::utils::import_options::json_namespace_has_properties(
      module,
      &self.link_output.module_table.modules,
      |idx| self.link_output.unreachable_modules.reaches(idx),
    ));
    // A CommonJS record calls its wrapper as it evaluates only while a static import or dynamic
    // `import()` of a module that the graph reaches imports it (`render_wrapped_entry_chunk`,
    // #821).
    h.write_val(
      &(matches!(meta.wrap_kind(), rolldown_common::WrapKind::Cjs)
        && self
          .link_output
          .unreachable_modules
          .imported_by_static_or_dynamic_import(&self.link_output.module_table.modules, module_idx)),
    );
    for stmt_idx in 0..self.link_output.stmt_infos[module_idx].len() {
      let sidx = rolldown_common::StmtInfoIdx::from_raw_unchecked(stmt_idx as u32);
      h.write_val(&meta.stmt_info_included.has_bit(sidx));
    }

    let mut temp_acc = (0u128, 0u128);
    for (binding, flag) in &meta.temporal_symbols {
      let mut eh = DualHasher::new();
      let flag_used = is_sym_used(*flag);
      let is_const = meta.temporal_constants.contains(binding);
      eh.write_val(&(
        sym_id_key(*binding),
        sym_id_key(*flag),
        flag_used,
        is_const,
      ));
      let [d0, d1] = eh.finish();
      temp_acc = (temp_acc.0.wrapping_add(d0), temp_acc.1.wrapping_add(d1));
    }
    h.write_unordered(7, meta.temporal_symbols.len(), temp_acc);

    let mut def_acc = (0u128, 0u128);
    for (rec_idx, target_sym) in &meta.deferred_imports {
      let mut eh = DualHasher::new();
      let target_idx = target_sym.owner;
      protected_deps.push(target_idx);
      let target_path = chunk_graph.module_to_chunk[target_idx].map(|tc_idx| {
        crate::utils::native_records::deferred_json_import_path(
          chunk,
          &chunk_graph.chunk_table[tc_idx],
        )
      });
      eh.write_val(&(
        rec_idx.raw(),
        sym_id_key(*target_sym),
        target_path,
        cache.module_scan_generation(target_idx),
      ));
      let [d0, d1] = eh.finish();
      def_acc = (def_acc.0.wrapping_add(d0), def_acc.1.wrapping_add(d1));
    }
    h.write_unordered(8, meta.deferred_imports.len(), def_acc);

    h.write_val(&module_owner_digest[module_idx]);

    h.write_val(&meta.referenced_symbols_by_entry_point_chunk.len());
    for (sym_ref, came_from_cjs) in &meta.referenced_symbols_by_entry_point_chunk {
      h.write_val(&(sym_id_key(*sym_ref), *came_from_cjs));
    }

    let mut ns_cjs_acc = (0u128, 0u128);
    for (k, v) in &meta.import_record_ns_to_cjs_module {
      let mut eh = DualHasher::new();
      eh.write_val(&(sym_id_key(*k), v.raw()));
      let [d0, d1] = eh.finish();
      ns_cjs_acc = (ns_cjs_acc.0.wrapping_add(d0), ns_cjs_acc.1.wrapping_add(d1));
    }
    h.write_unordered(11, meta.import_record_ns_to_cjs_module.len(), ns_cjs_acc);

    let mut mem_acc = (0u128, 0u128);
    for (node_id, res) in &meta.resolved_member_expr_refs {
      let mut eh = DualHasher::new();
      eh.write_val(&(
        node_id.index(),
        res.resolved.map(|r| (sym_id_key(r), module_owner_digest[r.owner])),
        res.target_commonjs_exported_symbol.map(|(s, b)| (sym_id_key(s), b)),
        res.prop_and_related_span_list.len(),
      ));
      if let Some((cjs_sym, _)) = res.target_commonjs_exported_symbol {
        hash_const_export_meta(
          &mut eh,
          self.link_output.global_constant_symbol_map.get(&cjs_sym),
        );
      }
      for prop in &res.prop_and_related_span_list {
        eh.write_val(&(prop.name.as_str(), prop.span.start, prop.span.end));
      }
      let [d0, d1] = eh.finish();
      mem_acc = (mem_acc.0.wrapping_add(d0), mem_acc.1.wrapping_add(d1));
    }
    h.write_unordered(12, meta.resolved_member_expr_refs.len(), mem_acc);

    let mut ns_in_acc = (0u128, 0u128);
    for (node_id, folded) in &meta.folded_namespace_in_checks {
      let mut eh = DualHasher::new();
      eh.write_val(&(node_id.index(), *folded));
      let [d0, d1] = eh.finish();
      ns_in_acc = (ns_in_acc.0.wrapping_add(d0), ns_in_acc.1.wrapping_add(d1));
    }
    h.write_unordered(16, meta.folded_namespace_in_checks.len(), ns_in_acc);

    let hash_sym_full = |h: &mut DualHasher, sym_ref: rolldown_common::SymbolRef| {
      let canonical_ref = symbol_db.canonical_ref_for(sym_ref);
      let sym = symbol_db.get(canonical_ref);
      let ns_alias = sym.namespace_alias.as_ref().map(|na| {
        let ns_canon = symbol_db.canonical_ref_for(na.namespace_ref);
        (
          na.property_name.as_str(),
          sym_id_key(ns_canon),
          module_owner_digest[ns_canon.owner],
        )
      });
      h.write_val(&(
        sym_id_key(sym_ref),
        sym_id_key(canonical_ref),
        is_sym_used(sym_ref),
        is_sym_used(canonical_ref),
        self.link_output.retained_export_symbols.contains(&sym_ref),
        self.link_output.retained_export_symbols.contains(&canonical_ref),
        symbol_db.is_facade_symbol(sym_ref),
        ns_alias,
        module_owner_digest[canonical_ref.owner],
      ));
      hash_const_export_meta(h, self.link_output.global_constant_symbol_map.get(&sym_ref));
      hash_const_export_meta(
        h,
        self.link_output.global_constant_symbol_map.get(&canonical_ref),
      );
    };
    let local_sym_count =
      symbol_db[module_idx].as_ref().map_or(0, |db| db.classic_data.len());
    let mut seen_local_syms = vec![false; local_sym_count];
    let mut seen_other_syms = FxHashSet::default();
    let mut hash_sym = |h: &mut DualHasher, sym_ref: rolldown_common::SymbolRef| {
      if sym_ref.owner == module_idx && !symbol_db.is_facade_symbol(sym_ref) {
        let idx = sym_ref.symbol.index();
        if idx < seen_local_syms.len() {
          if std::mem::replace(&mut seen_local_syms[idx], true) {
            return;
          }
          let canonical_ref = symbol_db.canonical_ref_for(sym_ref);
          let sym = symbol_db.get(canonical_ref);
          if canonical_ref == sym_ref && sym.namespace_alias.is_none() {
            h.write_val(&(
              (sym_ref.owner.raw(), idx as u32, sym_ref.name(symbol_db)),
              is_sym_used(sym_ref),
              self.link_output.retained_export_symbols.contains(&sym_ref),
              false,
            ));
            hash_const_export_meta(
              h,
              self.link_output.global_constant_symbol_map.get(&sym_ref),
            );
            return;
          }
          hash_sym_full(h, sym_ref);
          return;
        }
      }
      let key = sym_id_key(sym_ref);
      if seen_other_syms.insert(key) {
        let canonical_ref = symbol_db.canonical_ref_for(sym_ref);
        let sym = symbol_db.get(canonical_ref);
        if canonical_ref == sym_ref && sym_ref.owner == module_idx && sym.namespace_alias.is_none()
        {
          h.write_val(&(
            key,
            is_sym_used(sym_ref),
            self.link_output.retained_export_symbols.contains(&sym_ref),
            symbol_db.is_facade_symbol(sym_ref),
          ));
          hash_const_export_meta(h, self.link_output.global_constant_symbol_map.get(&sym_ref));
        } else {
          hash_sym_full(h, sym_ref);
        }
      }
    };

    hash_sym(&mut h, module.namespace_object_ref);
    hash_sym(&mut h, module.default_export_ref);
    if let Some(hmr_hot_ref) = module.ecma_view.hmr_hot_ref {
      hash_sym(&mut h, hmr_hot_ref);
    }
    let mut nimp_acc = (0u128, 0u128);
    for (&sym_ref, named_import) in &module.named_imports {
      let mut eh = DualHasher::new();
      hash_sym_full(&mut eh, sym_ref);
      eh.write_val(&named_import.record_idx.raw());
      let [d0, d1] = eh.finish();
      nimp_acc = (nimp_acc.0.wrapping_add(d0), nimp_acc.1.wrapping_add(d1));
    }
    h.write_unordered(13, module.named_imports.len(), nimp_acc);

    for stmt_info in &self.link_output.stmt_infos[module_idx].infos {
      for sym_ref in &stmt_info.declared_symbols {
        hash_sym(&mut h, sym_ref.inner());
      }
      for sym_or_mem in &stmt_info.referenced_symbols {
        match sym_or_mem {
          rolldown_common::SymbolOrMemberExprRef::Symbol(sym_ref) => {
            hash_sym(&mut h, *sym_ref);
          }
          rolldown_common::SymbolOrMemberExprRef::MemberExpr(mem_ref) => {
            hash_sym(&mut h, mem_ref.object_ref);
            if let Some(res) = mem_ref.resolution(&meta.resolved_member_expr_refs)
              && let Some(resolved) = res.resolved
            {
              hash_sym(&mut h, resolved);
            }
          }
        }
      }
    }

    h.write_val(&module.import_records.len());
    for (rec_idx, rec) in module.import_records.iter_enumerated() {
      hash_sym(&mut h, rec.namespace_ref);
      let merged_cjs_ns = chunk_graph.finalized_cjs_ns_map_idx_vec[chunk_idx]
        .get(&rec.namespace_ref)
        .copied()
        .map(&sym_id_key);
      h.write_val(&(
        rec.kind as u8,
        rec.meta.bits(),
        rec.resolved_module.map(|m| m.raw()),
        merged_cjs_ns,
      ));
      if let Some(importee_idx) = rec.resolved_module {
        let ext_path = self.link_output.module_table[importee_idx]
          .as_external()
          .map(|e| (e.get_import_path(chunk, self.resolved_paths.as_ref()), e.get_file_name(self.resolved_paths.as_ref())));
        let importee_meta = &metas[importee_idx];
        let omittable_star = cache.export_star_modules.contains(&module_idx)
          && crate::utils::native_records::omittable_star_evaluation(
            self.options,
            self.link_output,
            module,
            rec_idx,
          );
        let cell_deps = if cache.native_cell_modules.is_empty() {
          None
        } else {
          crate::utils::native_records::native_cell_evaluation_dependencies(
            self.options,
            self.link_output,
            &self.ast_table,
            module,
            rec_idx,
          )
        };
        h.write_val(&(module_owner_digest[importee_idx], omittable_star, ext_path));
        if let Some(wref) = importee_meta.wrapper_ref {
          hash_sym(&mut h, wref);
        }
        if let Some(deps) = cell_deps {
          h.write_val(&deps.len());
          for dep_idx in deps {
            h.write_val(&(dep_idx.raw(), cache.module_scan_generation(dep_idx)));
            protected_deps.push(dep_idx);
          }
        }
      }
    }

    let decision = h.finish();
    let reused = (module_idx != runtime_id || cache.runtime_rendered_chunk.is_some())
      && cache
        .cached_native_chunks
        .get(&module_idx)
        .is_some_and(|cached| cached.decision_hash == decision);
    (Some(decision), reused, protected_deps)
  }

  fn compute_chunk_output_exports(
    &self,
    chunk_graph: &mut ChunkGraph,
    warnings: &mut Vec<BuildDiagnostic>,
    used_symbol_refs: &UsedSymbolRefs,
    order_state: &order_wrap_state::OrderWrapState,
  ) -> BuildResult<()> {
    // Collect all the chunk data we need first
    let mut chunk_export_data = Vec::new();
    for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
      if let Some(entry_module) = chunk.user_defined_entry_module(&self.link_output.module_table) {
        let export_names = get_chunk_export_names(chunk, self.link_output);
        chunk_export_data.push((chunk_idx, entry_module, export_names));
      }
    }

    // Now compute the export modes for each chunk
    for (chunk_idx, entry_module, export_names) in chunk_export_data {
      let export_mode = determine_export_mode(
        warnings,
        &GenerateContext {
          chunk: &chunk_graph.chunk_table[chunk_idx],
          options: self.options,
          link_output: self.link_output,
          used_symbol_refs,
          order_wrap_state: order_state,
          chunk_graph,
          plugin_driver: self.plugin_driver,
          module_id_to_codegen_ret: Vec::new(),
          cached_module_sources: None,
          skip_sourcemap_join: false,
          multi_module_chunk: None,
          render_export_items_index_vec: &IndexVec::default(),
          chunk_idx,
          resolved_paths: self.resolved_paths.as_ref(),
          ast_table: None,
          final_esm_init_metadata: None,
          inline_state: &self.inline_state,
          carried_renders: vec![],
        },
        entry_module,
        &export_names,
      )?;
      chunk_graph.chunk_table[chunk_idx].output_exports = export_mode;
    }

    // Set common chunks to Named
    for chunk in chunk_graph.chunk_table.iter_mut() {
      if chunk.user_defined_entry_module(&self.link_output.module_table).is_none() {
        chunk.output_exports = OutputExports::Named;
      }
    }

    Ok(())
  }

  fn trace_action_chunks_infos(&self, chunk_graph: &ChunkGraph) {
    if trace_action_enabled!() {
      let mut chunk_infos = Vec::new();
      for (idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
        let mut imports = chunk
          .imports_from_other_chunks
          .iter()
          .map(|(importee_idx, _imports)| action::ChunkImport {
            chunk_id: importee_idx.raw(),
            kind: "import-statement",
          })
          .collect::<Vec<_>>();

        imports.extend(chunk.cross_chunk_dynamic_imports.iter().map(|importee_idx| {
          action::ChunkImport { chunk_id: importee_idx.raw(), kind: "dynamic-import" }
        }));

        chunk_infos.push(action::Chunk {
          is_user_defined_entry: chunk.is_user_defined_entry(),
          is_async_entry: chunk.is_async_entry(),
          entry_module: chunk
            .entry_module_idx()
            .map(|idx| self.link_output.module_table[idx].id().to_string()),
          modules: chunk
            .modules
            .iter()
            .map(|idx| self.link_output.module_table[*idx].id().to_string())
            .collect(),
          reason: chunk.chunk_reason_type.as_static_str(),
          advanced_chunk_group_id: chunk.chunk_reason_type.group_index(),
          chunk_id: idx.raw(),
          name: chunk.name.as_ref().map(ArcStr::to_string),
          imports,
        });
      }
      trace_action!(action::ChunkGraphReady { action: "ChunkGraphReady", chunks: chunk_infos });
    }
  }

  /// Per-module rendered sizes (post tree-shaking, pre whole-chunk minification), summed across
  /// every chunk a module was bundled into — the byte weights the metrics sink's dominator-tree
  /// retained-size analysis attributes over the module graph.
  fn trace_action_module_rendered(
    &self,
    instantiated_chunks: &crate::type_alias::IndexInstantiatedChunks,
  ) {
    if trace_action_enabled!() {
      let mut bytes_by_module: FxHashMap<&str, u64> = FxHashMap::default();
      for chunk in instantiated_chunks {
        let InstantiationKind::Ecma(ecma_meta) = &chunk.kind else {
          continue;
        };
        for (module_id, rendered_module) in ecma_meta
          .rendered_chunk
          .modules
          .keys
          .iter()
          .zip(ecma_meta.rendered_chunk.modules.values.iter())
        {
          *bytes_by_module.entry(module_id.as_str()).or_default() +=
            rendered_module.rendered_byte_length() as u64;
        }
      }
      let mut modules: Vec<action::ModuleRendered> = bytes_by_module
        .into_iter()
        .map(|(id, bytes)| action::ModuleRendered {
          id: id.to_string(),
          bytes: u32::try_from(bytes).unwrap_or(u32::MAX),
        })
        .collect();
      modules.sort_unstable_by(|a, b| a.id.cmp(&b.id));
      trace_action!(action::ModuleRenderedReady { action: "ModuleRenderedReady", modules });
    }
  }

  fn trace_action_package_graph_ready(
    &self,
    chunk_graph: &ChunkGraph,
    instantiated_chunks: &crate::type_alias::IndexInstantiatedChunks,
  ) {
    if trace_action_enabled!() {
      let mut package_infos: FxHashMap<String, DevtoolsPackageInfoEntry> = FxHashMap::default();
      let mut module_id_to_package_id: FxHashMap<String, String> = FxHashMap::default();
      let cwd = self.options.cwd.to_string_lossy();
      let cwd_slash = self.options.cwd.to_slash_lossy();

      for module in self.link_output.module_table.modules.iter().filter_map(|m| m.as_normal()) {
        let Some(package_json) = module.originative_resolved_id.package_json.as_ref() else {
          continue;
        };

        let Some((package_info, _, _)) =
          ensure_devtools_package_info(&mut package_infos, package_json)
        else {
          continue;
        };

        if module.importers_idx.iter().any(|importer_idx| {
          let importer_id = self.link_output.module_table[*importer_idx].id().as_str();
          is_devtools_source_importer(importer_id, cwd.as_ref(), cwd_slash.as_ref())
        }) {
          package_info.dependency_type = "direct";
        }
      }

      for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
        for module_idx in &chunk.modules {
          let Some(module) = self.link_output.module_table[*module_idx].as_normal() else {
            continue;
          };
          let Some(package_json) = module.originative_resolved_id.package_json.as_ref() else {
            continue;
          };
          let Some((package_info, modules, chunk_ids)) =
            ensure_devtools_package_info(&mut package_infos, package_json)
          else {
            continue;
          };

          package_info.is_used = true;

          let module_id = module.id.to_string();
          module_id_to_package_id.insert(module_id.clone(), package_info.package_id.clone());
          if modules.insert(module_id.clone()) {
            package_info.modules.push(module_id);
          }
          if chunk_ids.insert(chunk_idx.raw()) {
            package_info.chunk_ids.push(chunk_idx.raw());
          }
        }
      }

      for chunk in instantiated_chunks {
        let InstantiationKind::Ecma(ecma_meta) = &chunk.kind else {
          continue;
        };

        for (module_id, rendered_module) in ecma_meta
          .rendered_chunk
          .modules
          .keys
          .iter()
          .zip(ecma_meta.rendered_chunk.modules.values.iter())
        {
          let Some(package_id) = module_id_to_package_id.get(module_id.as_str()) else {
            continue;
          };
          let Some((package_info, _, _)) = package_infos.get_mut(package_id) else {
            continue;
          };

          let rendered_byte_length =
            u32::try_from(rendered_module.rendered_byte_length()).unwrap_or(u32::MAX);
          package_info.size = package_info.size.saturating_add(rendered_byte_length);
        }
      }

      let mut packages = package_infos.into_values().map(|(info, _, _)| info).collect::<Vec<_>>();
      packages.sort_unstable_by(|a, b| {
        a.name
          .cmp(&b.name)
          .then_with(|| a.version.cmp(&b.version))
          .then_with(|| a.package_root.cmp(&b.package_root))
          .then_with(|| a.package_id.cmp(&b.package_id))
      });

      trace_action!(action::PackageGraphReady { action: "PackageGraphReady", packages });
    }
  }
}

fn rendered_module_set(chunk_graph: &ChunkGraph) -> FxHashSet<ModuleIdx> {
  chunk_graph
    .chunk_table
    .iter_enumerated()
    .filter(|(chunk_idx, _)| chunk_graph.chunk_is_live(*chunk_idx))
    .flat_map(|(_, chunk)| chunk.modules.iter().copied())
    .collect()
}
