//! The chunk side of a fast native relink whose update appends or retires modules: every
//! appended module becomes its own record, and a retired module's record leaves the output.
//! The cross-chunk imports of a changed record are derived as `compute_cross_chunk_links`
//! derives them for one chunk; where they cannot be, the relink links the whole graph.

use std::sync::Arc;

use arcstr::ArcStr;
use oxc_index::IndexVec;
use oxc_str::CompactStr;
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, ChunkMeta, CrossChunkImportItem, ImportKind, ImportRecordMeta,
  ModuleIdx, Output, OutputChunk, OutputExports, RUNTIME_HELPER_NAMES, SourceMapType,
  SymbolOrMemberExprRef, SymbolRef, UsedSymbolRefsView, WrapKind,
};
use rolldown_error::BuildResult;
use rolldown_std_utils::{PathBufExt as _, absolutize_path_buf};
use rolldown_utils::{
  BitSet,
  dashmap::FxDashMap,
  hash_placeholder::HashPlaceholderGenerator,
  indexmap::{FxIndexMap, FxIndexSet},
};
use rustc_hash::{FxHashMap, FxHashSet};
use sugar_path::SugarPath as _;

use super::{
  FinalEsmInitMetadata, GenerateStage, PendingNativeRelinkState, Sealed,
  chunk_ext::{ChunkCreationReason, ChunkDebugExt},
  compute_cross_chunk_links::FinalEsmInitMetadataAvailability,
  order_wrap_state::{OrderCjsCarrierKey, OrderWrapState},
  preserve_modules_chunk_name, render_chunk_to_assets,
};
use crate::{
  chunk_graph::ChunkGraph,
  types::scan_stage_cache::{CachedChunkNaming, ScanStageCache},
  utils::chunk::generate_pre_rendered_chunk,
};

/// Gives up on the graph change, saying why by the line that gave up; the relink then links the
/// whole graph.
macro_rules! reject {
  () => {{
    tracing::trace!(target: "native_relink", line = line!(), "fast native relink rejected");
    return Ok(Err(concat!("graph_change:", line!())));
  }};
  ($reason:literal) => {{
    tracing::trace!(target: "native_relink", reason = $reason, "fast native relink rejected");
    return Ok(Err($reason));
  }};
}

/// How a fast native relink changes the cached chunk graph.
pub struct NativeChunkChange {
  /// Appended modules and the chunks that hold them, in chunk order.
  pub added: Vec<(ModuleIdx, ChunkIdx)>,
  pub retired: Vec<ModuleIdx>,
  /// Cached modules whose edges changed, with the modules in the output, neither appended nor
  /// retired, that their new records import or their dropped records imported (#626).
  pub edges_changed: Vec<(ModuleIdx, Vec<ModuleIdx>)>,
  /// Every reachable normal module in execution order.
  pub sorted_modules: Vec<ModuleIdx>,
  /// Whether cached modules execute in another relative order than the cached output's.
  pub reordered: bool,
}

/// Whether a chunk still holds its module, rather than a module that a fast relink retired.
pub(crate) fn is_live_chunk(chunk_graph: &ChunkGraph, chunk_idx: ChunkIdx) -> bool {
  chunk_graph.chunk_table[chunk_idx].entry_module_idx().is_some_and(|module| {
    chunk_graph.module_to_chunk.get(module).and_then(|c| *c) == Some(chunk_idx)
  })
}

/// The lowercase file names that deconfliction relates to `filename`: the name itself, and each
/// name that it could have deconflicted to `filename` (#610). `make_unique_name` appends a count
/// from 2 up to the part of a name before the first dot after its file name's first byte, so the
/// names it relates to one another all share one of these.
fn name_family(filename: &str) -> Vec<String> {
  let name = filename.to_ascii_lowercase();
  let file_start = name.rfind('/').map_or(0, |slash| slash + 1);
  let stem_end = name.as_bytes()[file_start..]
    .get(1..)
    .and_then(|rest| rest.iter().position(|&byte| byte == b'.'))
    .map_or(name.len(), |dot| file_start + 1 + dot);
  let (stem, extension) = name.split_at(stem_end);
  let digits = stem.bytes().rev().take_while(u8::is_ascii_digit).count();
  let mut family = Vec::with_capacity(1 + digits);
  for len in 1..=digits {
    let (prefix, count) = stem.split_at(stem.len() - len);
    if prefix.len() > file_start
      && !count.starts_with('0')
      && count.parse::<u64>().is_ok_and(|count| count >= 2)
    {
      family.push(format!("{prefix}{extension}"));
    }
  }
  family.push(name);
  family
}

/// Whether `chunk`'s record holds the name it asks for, so that deconfliction named it after no
/// other record's. A whole-graph generation caches a record's naming with `used_name_keys` only
/// when no two of its names collided, and a fast relink caches it for each record that it
/// appends, whose name no live record holds.
fn holds_own_name(cache: Option<&ScanStageCache>, chunk: &Chunk) -> bool {
  let [module_idx] = chunk.modules.as_slice() else {
    return false;
  };
  cache.and_then(|cache| cache.cached_chunk_namings.get(module_idx)).is_some_and(|naming| {
    naming.used_name_keys.is_some()
      && chunk.preliminary_filename.as_ref().map(|f| f.as_str())
        == Some(naming.preliminary_filename.as_str())
      && chunk.preliminary_sourcemap_filename.as_ref().map(|f| f.as_str())
        == naming.preliminary_sourcemap_filename.as_ref().map(|f| f.as_str())
  })
}

/// Whether deconfliction, which names records in execution order, would name each of `live`'s
/// records as it is named in any order: each holds the name it asks for. The records that a
/// relink appends take names that no live record holds.
fn names_ignore_order(
  cache: Option<&ScanStageCache>,
  chunk_graph: &ChunkGraph,
  live: &[ChunkIdx],
) -> bool {
  live.iter().all(|&chunk_idx| holds_own_name(cache, &chunk_graph.chunk_table[chunk_idx]))
}

impl GenerateStage<'_> {
  /// Applies an update's module graph change to the cached chunk graph: retires the records of
  /// retired modules, appends a record for each appended module, and derives the cross-chunk
  /// imports of every record whose edges changed. Returns the retired records' files, or why
  /// the change is not provably equal to generating the whole graph.
  #[expect(clippy::too_many_lines)]
  pub(crate) async fn apply_native_graph_change(
    &mut self,
    relink: &mut PendingNativeRelinkState,
    change: &NativeChunkChange,
  ) -> BuildResult<Result<Vec<ArcStr>, &'static str>> {
    let modules_len = self.link_output.module_table.modules.len();
    // The names that deconfliction relates to those that the update retires or appends, the
    // appended records' names with the record that takes each, and their namings.
    let mut related: FxHashSet<String> = FxHashSet::default();
    let mut appended_names: Vec<(ChunkIdx, String)> = Vec::new();
    let mut appended_namings: Vec<(ModuleIdx, CachedChunkNaming)> = Vec::new();

    let mut retired_chunks = FxHashSet::default();
    let mut retired_filenames = Vec::new();
    for &module in &change.retired {
      let chunk_graph = &mut relink.chunk_graph;
      let Some(chunk_idx) = chunk_graph.module_to_chunk[module] else {
        continue;
      };
      let chunk = &chunk_graph.chunk_table[chunk_idx];
      let Some(filename) = chunk.preliminary_filename.as_ref().map(|f| ArcStr::from(f.as_str()))
      else {
        reject!();
      };
      related.extend(
        std::iter::once(filename.as_str())
          .chain(chunk.preliminary_sourcemap_filename.as_ref().map(|map| map.as_str()))
          .flat_map(name_family),
      );
      if let Some(map) = &chunk.preliminary_sourcemap_filename {
        retired_filenames.push(ArcStr::from(map.as_str()));
      } else if matches!(self.options.sourcemap, Some(SourceMapType::File | SourceMapType::Hidden))
      {
        retired_filenames.push(arcstr::format!("{filename}.map"));
      }
      retired_filenames.push(filename);
      retired_chunks.insert(chunk_idx);
      chunk_graph.module_to_chunk[module] = None;
      chunk_graph.entry_module_to_entry_chunk.remove(&module);
      relink.used_overrides.remove(&module);
      if let Some(cache) = self.cache.as_deref_mut() {
        cache.cached_native_chunks.remove(&module);
        cache.cached_chunk_namings.remove(&module);
      }
    }

    {
      let chunk_graph = &mut relink.chunk_graph;
      while chunk_graph.module_to_chunk.len() < modules_len {
        chunk_graph.module_to_chunk.push(None);
      }
      let Some(input_base) = chunk_graph.chunk_table.iter().next().map(|c| c.input_base.clone())
      else {
        reject!();
      };
      for &(module_idx, chunk_idx) in &change.added {
        let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
          reject!();
        };
        if chunk_idx.index() != chunk_graph.chunk_table.len() {
          reject!();
        }
        let mut meta = ChunkMeta::default();
        meta.set(
          ChunkMeta::DynamicImported,
          self.link_output.unreachable_modules.dynamically_imported(module),
        );
        let mut chunk = Chunk::new(
          None,
          None,
          BitSet::default(),
          vec![],
          ChunkKind::EntryPoint { meta, bit: module_idx.raw(), module: module_idx },
          input_base.clone(),
          None,
        );
        chunk.add_creation_reason(
          ChunkCreationReason::PreserveModules {
            is_user_defined_entry: false,
            module_stable_id: &module.stable_id,
          },
          self.options,
        );
        chunk.output_exports = OutputExports::Named;
        let added_idx = chunk_graph.add_chunk(chunk);
        chunk_graph.add_module_to_chunk(
          module_idx,
          added_idx,
          self.link_output.metas[module_idx].depended_runtime_helper,
        );
        chunk_graph.entry_module_to_entry_chunk.insert(module_idx, added_idx);
      }

      // Chunks render in their modules' execution order.
      let mut next_exec_order = 0u32;
      for &module_idx in &change.sorted_modules {
        if let Some(chunk_idx) = chunk_graph.module_to_chunk[module_idx] {
          chunk_graph.chunk_table[chunk_idx].exec_order = next_exec_order;
          next_exec_order += 1;
        }
      }
      let sort_key = |chunk_graph: &ChunkGraph, chunk_idx: ChunkIdx| match &chunk_graph.chunk_table
        [chunk_idx]
        .kind
      {
        ChunkKind::EntryPoint { meta, .. } if meta.contains(ChunkMeta::UserDefinedEntry) => {
          (0, chunk_idx.raw())
        }
        _ => (1, chunk_graph.chunk_table[chunk_idx].exec_order),
      };
      let mut sorted = std::mem::take(&mut chunk_graph.sorted_chunk_idx_vec);
      sorted.retain(|chunk_idx| !retired_chunks.contains(chunk_idx));
      if change.reordered {
        // Deconfliction names records in this order, so a reorder keeps every name only if no
        // record took another name than the one it asks for.
        if !names_ignore_order(self.cache.as_deref(), chunk_graph, &sorted) {
          reject!("exec_order_names");
        }
        // The cached chunks' order no longer follows their keys.
        sorted.extend(change.added.iter().map(|&(_, chunk_idx)| chunk_idx));
        sorted.sort_by_key(|chunk_idx| sort_key(chunk_graph, *chunk_idx));
      } else {
        for &(_, chunk_idx) in &change.added {
          let key = sort_key(chunk_graph, chunk_idx);
          let position = sorted.partition_point(|existing| sort_key(chunk_graph, *existing) < key);
          sorted.insert(position, chunk_idx);
        }
      }
      chunk_graph.sorted_chunk_idx_vec = sorted;
    }

    let added_chunks: FxHashSet<ChunkIdx> = change.added.iter().map(|(_, c)| *c).collect();
    // An edited module whose records change may gain runtime helpers (`prepare_native_module`),
    // which its record imports anew (#653).
    let mut gained_helpers = FxHashSet::default();
    for (module_idx, _) in &change.edges_changed {
      let helpers = self.link_output.metas[*module_idx].depended_runtime_helper;
      if let Some(chunk_idx) = relink.chunk_graph.module_to_chunk[*module_idx] {
        let chunk = &mut relink.chunk_graph.chunk_table[chunk_idx];
        if chunk.depended_runtime_helper != helpers {
          chunk.depended_runtime_helper = helpers;
          gained_helpers.insert(chunk_idx);
        }
      }
    }
    let mut new_imports = Vec::with_capacity(change.added.len() + change.edges_changed.len());
    {
      let chunk_graph = &relink.chunk_graph;
      let is_live = |symbol: SymbolRef| {
        if let Some(module) = self.link_output.module_table[symbol.owner].as_normal()
          && module.namespace_object_ref == symbol
        {
          return self.link_output.metas[symbol.owner].namespace_included;
        }
        relink
          .used_overrides
          .get(&symbol.owner)
          .map_or_else(|| relink.used_symbol_refs.contains(&symbol), |used| used.contains(&symbol))
      };
      for &(_, chunk_idx) in &change.added {
        let Some(imports) = self.native_chunk_imports(
          chunk_graph,
          chunk_idx,
          &relink.order_state,
          &relink.final_esm_init_metadata,
          relink.used_symbol_refs.view(),
          &is_live,
        ) else {
          reject!();
        };
        new_imports.push((chunk_idx, imports));
      }
      for (module_idx, edge_targets) in &change.edges_changed {
        let Some(chunk_idx) = chunk_graph.module_to_chunk[*module_idx] else {
          reject!();
        };
        let Some(imports) = self.native_chunk_imports(
          chunk_graph,
          chunk_idx,
          &relink.order_state,
          &relink.final_esm_init_metadata,
          relink.used_symbol_refs.view(),
          &is_live,
        ) else {
          reject!();
        };
        // Records the edges did not change keep the imports that the whole graph's link gave
        // them; this derivation must reproduce those. The chunks of the modules that new
        // records import may be imported anew, and those that dropped records imported may
        // no longer be (#626).
        let edge_chunks: FxHashSet<ChunkIdx> =
          edge_targets.iter().filter_map(|target| chunk_graph.module_to_chunk[*target]).collect();
        let runtime_chunk = chunk_graph.module_to_chunk[self.link_output.runtime.id()];
        let old_imports = &chunk_graph.chunk_table[chunk_idx].imports_from_other_chunks;
        let kept_equal = imports.iter().all(|(importee, items)| {
          added_chunks.contains(importee)
            || edge_chunks.contains(importee) && !old_imports.contains_key(importee)
            || gained_helpers.contains(&chunk_idx) && Some(*importee) == runtime_chunk
            || old_imports.get(importee).is_some_and(|old_items| {
              old_items.len() == items.len()
                && old_items.iter().zip(items).all(|(a, b)| a.import_ref == b.import_ref)
            })
        }) && old_imports.keys().all(|importee| {
          retired_chunks.contains(importee)
            || edge_chunks.contains(importee)
            || imports.contains_key(importee)
        });
        if !kept_equal {
          reject!();
        }
        new_imports.push((chunk_idx, imports));
      }

      // Appended records export their complete declared interface. A CommonJS one exports its
      // `require_*` wrapper, which every record that imports or requires it calls (#653).
      let mut added_exports = Vec::with_capacity(change.added.len());
      for &(module_idx, chunk_idx) in &change.added {
        let mut exports: FxHashMap<SymbolRef, Vec<CompactStr>> = FxHashMap::default();
        let meta = &self.link_output.metas[module_idx];
        if let Some(wrapper) = meta.wrapper_ref
          && matches!(meta.wrap_kind(), WrapKind::Cjs)
          && is_live(wrapper)
        {
          let name = CompactStr::new(wrapper.name(&self.link_output.symbol_db));
          exports.entry(wrapper).or_default().push(name);
        }
        for (name, export) in meta.canonical_exports(false) {
          if is_live(export.symbol_ref) {
            exports.entry(export.symbol_ref).or_default().push(name.clone());
          }
        }
        added_exports.push((chunk_idx, exports));
      }
      // No record's interface may change: everything imported anew is already exported, or is
      // the record's own native namespace, which its importers reach through `import * as`, or
      // the interop namespace through which an appended record forwards a CommonJS module's
      // binding, which its importers read the binding from (#674).
      let exports_of = |chunk_idx: ChunkIdx| {
        added_exports.iter().find(|(added, _)| *added == chunk_idx).map_or_else(
          || &chunk_graph.chunk_table[chunk_idx].exports_to_other_chunks,
          |(_, exports)| exports,
        )
      };
      let mut namespace_exports: Vec<(ChunkIdx, SymbolRef)> = Vec::new();
      let mut forwarded_namespaces: Vec<(ChunkIdx, SymbolRef)> = Vec::new();
      for (_, imports) in &new_imports {
        for (importee, items) in imports {
          if retired_chunks.contains(importee) {
            reject!();
          }
          let exports = exports_of(*importee);
          for item in items {
            if exports.contains_key(&item.import_ref) {
              continue;
            }
            let owner = item.import_ref.owner;
            if added_exports.iter().any(|(added, _)| added == importee)
              && chunk_graph.chunk_table[*importee].entry_module_idx() == Some(owner)
              && self.link_output.metas[owner]
                .import_record_ns_to_cjs_module
                .contains_key(&item.import_ref)
            {
              if !is_live(item.import_ref) {
                reject!();
              }
              if !forwarded_namespaces.contains(&(*importee, item.import_ref)) {
                forwarded_namespaces.push((*importee, item.import_ref));
              }
              continue;
            }
            let canonical = self.link_output.symbol_db.canonical_ref_for(item.import_ref);
            if !crate::utils::native_records::is_native_namespace(
              self.options,
              self.link_output,
              canonical,
            ) || chunk_graph.chunk_table[*importee].entry_module_idx() != Some(canonical.owner)
            {
              reject!();
            }
            if !namespace_exports.contains(&(*importee, item.import_ref)) {
              namespace_exports.push((*importee, item.import_ref));
            }
          }
        }
      }
      // A whole link still lists that namespace among the record's exports, under the name that
      // `deconflict_exported_names` resolves from its symbol's (#651). That is the symbol's own
      // name unless another export of the record takes it or a name suffixed from it.
      let mut named_namespaces = Vec::with_capacity(namespace_exports.len());
      for (chunk_idx, namespace) in namespace_exports {
        let name = namespace.name(&self.link_output.symbol_db);
        let taken = |export: &CompactStr| {
          export.strip_prefix(name).is_some_and(|rest| rest.is_empty() || rest.starts_with('$'))
        };
        if name == "then" || exports_of(chunk_idx).values().flatten().any(taken) {
          reject!("namespace_export_name");
        }
        named_namespaces.push((chunk_idx, namespace, CompactStr::new(name)));
      }
      // An appended record's exports are named as `deconflict_exported_names` names a preserved
      // module's: its declared names, reserved first, then the symbol's own name, deconflicted in
      // `deconflict_order_key` order, for each interop namespace that its importers read (#682).
      let mut forwarded_names = Vec::with_capacity(forwarded_namespaces.len());
      for (chunk_idx, exports) in &added_exports {
        if !forwarded_namespaces.iter().any(|(forwarding, _)| forwarding == chunk_idx) {
          continue;
        }
        if named_namespaces.iter().any(|(exporting, _, _)| exporting == chunk_idx) {
          reject!();
        }
        let mut namespaces = forwarded_namespaces
          .iter()
          .filter(|(forwarding, _)| forwarding == chunk_idx)
          .map(|(_, namespace)| *namespace)
          .collect::<Vec<_>>();
        namespaces.sort_by_cached_key(|namespace| {
          crate::utils::chunk::conflict_resolver::deconflict_order_key(
            *namespace,
            &self.link_output.module_table,
            &self.link_output.symbol_db,
          )
        });
        let mut resolver = crate::utils::chunk::conflict_resolver::ConflictResolver::with_capacity(
          exports.len() + namespaces.len(),
        );
        resolver.reserve(CompactStr::new_const(
          super::compute_cross_chunk_links::THENABLE_HAZARD_EXPORT_NAME,
        ));
        for name in exports.values().flatten() {
          resolver.reserve(name.clone());
        }
        for namespace in namespaces {
          let name = resolver
            .resolve(CompactStr::new(namespace.name(&self.link_output.symbol_db)), |_, _| true);
          forwarded_names.push((*chunk_idx, namespace, name));
        }
      }
      let chunk_graph = &mut relink.chunk_graph;
      for (chunk_idx, exports) in added_exports {
        chunk_graph.chunk_table[chunk_idx].exports_to_other_chunks = exports;
      }
      for (chunk_idx, namespace, name) in named_namespaces {
        chunk_graph.chunk_table[chunk_idx].exports_to_other_chunks.insert(namespace, vec![name]);
      }
      for (chunk_idx, namespace, name) in forwarded_names {
        chunk_graph.chunk_table[chunk_idx].exports_to_other_chunks.insert(namespace, vec![name]);
      }
    }
    let derived: FxHashSet<ChunkIdx> =
      new_imports.iter().map(|(chunk_idx, _)| *chunk_idx).collect();
    for (chunk_idx, imports) in new_imports {
      let chunk = &mut relink.chunk_graph.chunk_table[chunk_idx];
      chunk.cross_chunk_imports = imports.keys().copied().collect();
      chunk.imports_from_other_chunks = imports;
    }
    // The other records keep the imports that the cached link sorted by authored rank, then by
    // execution order. An update that reorders execution keeps them only if linking the updated
    // graph would sort them the same way.
    if change.reordered {
      let chunk_graph = &relink.chunk_graph;
      for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
        if (chunk.cross_chunk_imports.len() < 2 && chunk.imports_from_other_chunks.len() < 2)
          || derived.contains(&chunk_idx)
          || !is_live_chunk(chunk_graph, chunk_idx)
        {
          continue;
        }
        let Some(rank) = self.native_authored_import_rank(chunk_graph, chunk_idx) else {
          reject!();
        };
        let key = |importee: &ChunkIdx| {
          (
            rank.get(importee).copied().unwrap_or(usize::MAX),
            chunk_graph.chunk_table[*importee].exec_order,
          )
        };
        if !chunk.imports_from_other_chunks.keys().map(key).is_sorted()
          || !chunk.cross_chunk_imports.iter().map(key).is_sorted()
        {
          reject!("exec_order_imports");
        }
      }
    }

    // Appended records' names; none may take a name that another record holds or derives from.
    let output_dir = absolutize_path_buf(self.options.cwd.join(&self.options.out_dir));
    for &(module_idx, chunk_idx) in &change.added {
      let module_id = self.link_output.module_table[module_idx].id().as_str();
      let input_base = relink.chunk_graph.chunk_table[chunk_idx].input_base.clone();
      let names = preserve_modules_chunk_name(self.options, module_id, &input_base).await?;
      let pre_rendered_chunk = generate_pre_rendered_chunk(
        &relink.chunk_graph.chunk_table[chunk_idx],
        &names.chunk_name,
        self.link_output,
      );
      let mut hash_placeholder_generator = HashPlaceholderGenerator::default();
      let used_name_counts = FxDashMap::default();
      let preliminary_filename = relink.chunk_graph.chunk_table[chunk_idx]
        .generate_preliminary_filename(
          self.options,
          &pre_rendered_chunk,
          &names.chunk_filename,
          &mut hash_placeholder_generator,
          &used_name_counts,
        )
        .await?;
      let chunk = &mut relink.chunk_graph.chunk_table[chunk_idx];
      chunk.name = Some(names.chunk_name.clone());
      chunk.absolute_preliminary_filename =
        Some(preliminary_filename.absolutize_with(&output_dir).into_owned().expect_into_string());
      chunk.preliminary_filename = Some(preliminary_filename.clone());
      let preliminary_sourcemap_filename = relink.chunk_graph.chunk_table[chunk_idx]
        .generate_preliminary_sourcemap_filename(
          self.options,
          &pre_rendered_chunk,
          &names.chunk_filename,
          &mut hash_placeholder_generator,
          &used_name_counts,
        )
        .await?;
      if preliminary_filename.hash_placeholder().is_some()
        || preliminary_sourcemap_filename.as_ref().is_some_and(|f| f.hash_placeholder().is_some())
      {
        reject!();
      }
      for filename in
        std::iter::once(&preliminary_filename).chain(preliminary_sourcemap_filename.iter())
      {
        // Appended records take distinct names, each the name it asks for.
        let name = filename.as_str().to_ascii_lowercase();
        if appended_names.iter().any(|(owner, taken)| *owner != chunk_idx && *taken == name) {
          reject!("appended_name");
        }
        related.extend(name_family(filename.as_str()));
        appended_names.push((chunk_idx, name));
      }
      let keys: Vec<ArcStr> = appended_names
        .iter()
        .filter(|(owner, _)| *owner == chunk_idx)
        .map(|(_, name)| ArcStr::from(name.as_str()))
        .collect();
      // As a whole-graph generation does, the naming proves that the record holds the names it
      // asks for only if each is its own key and neither took another, as a sourcemap that asks
      // for its chunk's name would.
      let own_names = used_name_counts.len() == keys.len()
        && keys
          .iter()
          .all(|key| used_name_counts.get(key.as_str()).is_some_and(|count| *count == 2))
        && (keys.len() < 2 || keys[0] != keys[1]);
      let chunk = &mut relink.chunk_graph.chunk_table[chunk_idx];
      if let Some(cache) = self.cache.as_deref() {
        appended_namings.push((
          module_idx,
          CachedChunkNaming {
            scan_generation: cache.module_scan_generation(module_idx),
            input_base: input_base.clone(),
            explicit_name: None,
            is_user_defined_entry: false,
            used_name_keys: own_names.then_some(keys),
            representative_chunk_name: names.representative_chunk_name.clone(),
            chunk_name: names.chunk_name.clone(),
            chunk_filename: names.chunk_filename.clone(),
            preliminary_filename: preliminary_filename.clone(),
            absolute_preliminary_filename: chunk
              .absolute_preliminary_filename
              .clone()
              .unwrap_or_default(),
            preliminary_sourcemap_filename: preliminary_sourcemap_filename.clone(),
          },
        ));
      }
      chunk.preliminary_sourcemap_filename = preliminary_sourcemap_filename;
      chunk.pre_rendered_chunk = Some(pre_rendered_chunk);
      relink.index_chunk_id_to_name.insert(chunk_idx, names.representative_chunk_name);
    }

    // A fresh link names every record that the update keeps as it is named, unless an appended
    // record takes its name, or deconfliction named it after a name that the update retires or
    // appends. A record that holds the name it asks for wasn't named after another.
    if !related.is_empty() {
      let chunk_graph = &relink.chunk_graph;
      let appended: FxHashSet<&str> =
        appended_names.iter().map(|(_, name)| name.as_str()).collect();
      for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
        if added_chunks.contains(&chunk_idx) || !is_live_chunk(chunk_graph, chunk_idx) {
          continue;
        }
        let own = holds_own_name(self.cache.as_deref(), chunk);
        if appended.is_empty() && own {
          continue;
        }
        if own {
          if let [module_idx] = chunk.modules.as_slice()
            && let Some(keys) = self
              .cache
              .as_deref()
              .and_then(|c| c.cached_chunk_namings.get(module_idx))
              .and_then(|naming| naming.used_name_keys.as_deref())
          {
            if keys.iter().any(|key| appended.contains(key.as_str())) {
              reject!("renamed_record");
            }
            continue;
          }
        }
        for filename in
          chunk.preliminary_filename.iter().chain(chunk.preliminary_sourcemap_filename.iter())
        {
          if (!appended.is_empty() && {
            let s = filename.as_str();
            if s.bytes().all(|b| !b.is_ascii_uppercase()) {
              appended.contains(s)
            } else {
              appended.contains(s.to_ascii_lowercase().as_str())
            }
          }) || (!own && name_family(filename.as_str()).iter().any(|name| related.contains(name)))
          {
            reject!("renamed_record");
          }
        }
      }
    }

    self.merge_appended_cjs_namespaces(relink, change);

    for &(module_idx, _) in &change.added {
      relink.chunk_decisions.push(None);
      relink.render_export_items_index_vec.push(FxIndexMap::default());
      relink.index_chunk_to_instances.push(FxIndexSet::default());
      // Never equal to a digest a whole-graph generation computes, so it reuses no record.
      let stable_id = self.link_output.module_table[module_idx].stable_id().as_str();
      let owner_digest = [xxhash_rust::xxh3::xxh3_128(stable_id.as_bytes()), u128::MAX];
      let has_native_namespace =
        self.link_output.module_table[module_idx].as_normal().is_some_and(|m| {
          crate::utils::native_records::has_native_namespace(
            self.options,
            m,
            &self.link_output.metas[module_idx],
            &self.link_output.symbol_db,
          )
        });
      // A refreshed module keeps its index; the module-indexed tables update in place.
      if module_idx.index() < relink.module_owner_digest.len() {
        relink.module_owner_digest[module_idx] = owner_digest;
        relink.module_has_native_ns[module_idx] = has_native_namespace;
      } else {
        relink.module_owner_digest.push(owner_digest);
        relink.module_has_native_ns.push(has_native_namespace);
      }
    }
    if relink.module_owner_digest.len() != modules_len
      || relink.module_has_native_ns.len() != modules_len
      || relink.chunk_decisions.len() != relink.chunk_graph.chunk_table.len()
    {
      reject!();
    }

    // The output template drops retired records and gains placeholders for appended ones,
    // replaced as they render.
    relink.sorted_output_template.retain(|(chunk_idx, _)| !retired_chunks.contains(chunk_idx));
    for &(module_idx, chunk_idx) in &change.added {
      let chunk = &relink.chunk_graph.chunk_table[chunk_idx];
      let Some(filename) = chunk.preliminary_filename.as_ref().map(|f| ArcStr::from(f.as_str()))
      else {
        reject!();
      };
      let placeholder = Output::Chunk(Arc::new(OutputChunk {
        name: chunk.name.clone().unwrap_or_default(),
        filename: filename.clone(),
        code: String::new(),
        is_entry: false,
        is_dynamic_entry: false,
        facade_module_id: Some(self.link_output.module_table[module_idx].id().clone()),
        modules: FxHashMap::default().into(),
        exports: vec![],
        module_ids: vec![],
        imports: vec![],
        dynamic_imports: vec![],
        map: None,
        sourcemap_filename: None,
        preliminary_filename: filename.to_string(),
      }));
      let key = (render_chunk_to_assets::get_sorting_file_type_key(&placeholder), filename);
      let position = relink.sorted_output_template.partition_point(|(_, existing)| {
        (render_chunk_to_assets::get_sorting_file_type_key(existing), existing.filename())
          < (key.0, key.1.as_str())
      });
      relink.sorted_output_template.insert(position, (chunk_idx, placeholder));
    }
    relink.chunk_idx_to_template_pos =
      IndexVec::from_vec(vec![None; relink.chunk_graph.chunk_table.len()]);
    for (position, (chunk_idx, _)) in relink.sorted_output_template.iter().enumerate() {
      relink.chunk_idx_to_template_pos[*chunk_idx] = Some(position);
    }
    for filename in &retired_filenames {
      relink.reused_chunk_filenames_pool.remove(filename);
      relink.excluded_from_reused_filenames.retain(|excluded| excluded != filename);
      if let Some(cache) = self.cache.as_deref_mut() {
        cache.last_reused_chunk_filenames.remove(filename);
      }
    }
    if let Some(cache) = self.cache.as_deref_mut() {
      cache.naming_order_signature = None;
      // The appended records hold the names they ask for, which a whole-graph generation would
      // cache, so that a later relink knows that they do.
      cache.cached_chunk_namings.extend(appended_namings);
    }
    Ok(Ok(retired_filenames))
  }

  /// Merges an appended record's imports of one CommonJS module into its first, as
  /// `merge_cjs_namespace` does per chunk, and an edited record's new imports of an appended
  /// one (#653).
  fn merge_appended_cjs_namespaces(
    &mut self,
    relink: &mut PendingNativeRelinkState,
    change: &NativeChunkChange,
  ) {
    let added: FxHashSet<ModuleIdx> =
      change.added.iter().map(|(module_idx, _)| *module_idx).collect();
    let edited = change
      .edges_changed
      .iter()
      .filter_map(|(module_idx, _)| {
        Some((*module_idx, relink.chunk_graph.module_to_chunk[*module_idx]?))
      })
      .collect::<Vec<_>>();
    for (module_idx, chunk_idx) in change.added.iter().copied().chain(edited) {
      let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
        continue;
      };
      let meta = &self.link_output.metas[module_idx];
      let mut groups: FxIndexMap<ModuleIdx, Vec<SymbolRef>> = FxIndexMap::default();
      for rec in &module.import_records {
        if rec.kind != ImportKind::Import || rec.meta.contains(ImportRecordMeta::IsExportStar) {
          continue;
        }
        let Some(target) = rec.resolved_module else {
          continue;
        };
        // An edited record's imports of modules in the output keep their merged namespaces.
        if !added.contains(&module_idx) && !added.contains(&target) {
          continue;
        }
        let is_commonjs = self.link_output.module_table[target]
          .as_normal()
          .is_some_and(|target| target.exports_kind.is_commonjs());
        if !is_commonjs || !self.link_output.safely_merge_cjs_ns_map.contains_key(&target) {
          continue;
        }
        let namespace = rec.namespace_ref;
        let included = self.link_output.stmt_infos[module_idx]
          .declared_stmts_by_symbol(&namespace)
          .iter()
          .all(|stmt| meta.stmt_info_included.has_bit(*stmt));
        if included {
          groups.entry(target).or_default().push(namespace);
        }
      }
      let merged = &mut relink.chunk_graph.finalized_cjs_ns_map_idx_vec[chunk_idx];
      for namespaces in groups.values() {
        let Some((&first, rest)) = namespaces.split_first() else {
          continue;
        };
        merged.insert(first, first);
        for &namespace in rest {
          self.link_output.symbol_db.link(namespace, first);
          merged.insert(namespace, first);
        }
      }
    }
  }

  /// One native record's cross-chunk imports, in authored order, as `compute_cross_chunk_links`
  /// derives them: `collect_depended_symbols`, then `compute_chunk_imports` for the chunk.
  /// `None` for anything outside a preserved native record that is unwrapped or a CommonJS
  /// module's.
  fn native_chunk_imports(
    &self,
    chunk_graph: &ChunkGraph,
    chunk_idx: ChunkIdx,
    order_state: &OrderWrapState,
    final_esm_init_metadata: &Sealed<FinalEsmInitMetadata>,
    used_view: UsedSymbolRefsView<'_>,
    is_live: &dyn Fn(SymbolRef) -> bool,
  ) -> Option<FxIndexMap<ChunkIdx, Vec<CrossChunkImportItem>>> {
    let chunk = &chunk_graph.chunk_table[chunk_idx];
    let [module_idx] = chunk.modules.as_slice() else {
      return None;
    };
    let module_idx = *module_idx;
    let runtime_id = self.link_output.runtime.id();
    let module = self.link_output.module_table[module_idx].as_normal()?;
    let meta = &self.link_output.metas[module_idx];
    let symbols = &self.link_output.symbol_db;
    if module_idx == runtime_id
      || !matches!(chunk.kind, ChunkKind::EntryPoint { module, .. } if module == module_idx)
      || !matches!(meta.wrap_kind(), WrapKind::None | WrapKind::Cjs)
      || chunk_graph.common_chunk_exported_facade_chunk_namespace.contains_key(&chunk_idx)
      || order_state.synthetic_statements_for_chunk(chunk_idx).next().is_some()
      || order_state.consumer_local_namespace_targets(module_idx).is_some()
      || order_state.esm_init_target(module_idx, meta).is_some()
    {
      return None;
    }
    // Dynamic imports and external modules have chunk tables of their own.
    for rec in &module.import_records {
      if rec.kind == ImportKind::DynamicImport
        || rec
          .resolved_module
          .is_some_and(|target| self.link_output.module_table[target].is_external())
      {
        return None;
      }
    }

    let mut depended = FxIndexSet::default();
    for (stmt_info_idx, stmt_info) in self.link_output.stmt_infos[module_idx].iter_enumerated() {
      if !meta.stmt_info_included.has_bit(stmt_info_idx)
        && !order_state.forces_runtime_stmt(&self.link_output.runtime, module_idx, stmt_info)
      {
        continue;
      }
      if stmt_info.import_records.iter().any(|record| {
        order_state
          .has_order_cjs_carrier(OrderCjsCarrierKey { importer: module_idx, record: *record })
      }) {
        return None;
      }
      for reference in &stmt_info.referenced_symbols {
        let referenced = match reference {
          SymbolOrMemberExprRef::Symbol(referenced) => *referenced,
          SymbolOrMemberExprRef::MemberExpr(member_expr) => {
            let Some(referenced) =
              member_expr.represent_symbol_ref(&meta.resolved_member_expr_refs)
            else {
              continue;
            };
            referenced
          }
        };
        self.add_depended_symbol_with_wrapped_esm_init(
          chunk_graph,
          order_state,
          &mut depended,
          symbols.canonical_ref_resolving_namespace(referenced),
        );
      }
    }
    self.add_module_esm_init_depended_symbols(
      chunk_graph,
      used_view,
      order_state,
      FinalEsmInitMetadataAvailability::Sealed(final_esm_init_metadata),
      &mut depended,
      module_idx,
    );
    if matches!(meta.wrap_kind(), WrapKind::Cjs) {
      // A CommonJS record's exports are its wrapper's: an appended one's all come from
      // CommonJS (`seed_added_modules`), so it depends on no other module's (#653).
      depended.insert(meta.wrapper_ref?);
    } else {
      self.register_entry_export_depended_symbols(
        chunk_graph,
        order_state,
        &mut depended,
        module_idx,
        meta,
      );
    }
    self.add_absorbed_entry_init_deps(chunk_graph, order_state, &mut depended, chunk_idx);
    for helper in chunk.depended_runtime_helper {
      let index = helper.bits().trailing_zeros() as usize;
      depended.insert(self.link_output.runtime.resolve_symbol(RUNTIME_HELPER_NAMES[index]));
    }

    let mut imports: FxIndexMap<ChunkIdx, Vec<CrossChunkImportItem>> = FxIndexMap::default();
    let routes = self.native_authored_reexport_routes(chunk_graph, chunk_idx)?;
    for import_ref in depended {
      if !is_live(import_ref) {
        continue;
      }
      if self.link_output.module_table[import_ref.owner].is_external() {
        return None;
      }
      let owner_chunk = if import_ref.owner == runtime_id {
        symbols.get(import_ref).chunk_idx
      } else {
        chunk_graph.module_to_chunk[import_ref.owner]
      }?;
      let route = (owner_chunk != chunk_idx)
        .then(|| self.native_authored_reexport_route(&routes, owner_chunk, import_ref))
        .flatten();
      let (importee_chunk, exported_ref) = route.unwrap_or((owner_chunk, import_ref));
      if importee_chunk != chunk_idx {
        imports
          .entry(importee_chunk)
          .or_default()
          .push(CrossChunkImportItem { import_ref: exported_ref });
      }
    }
    if routes.0 != module_idx {
      return None;
    }
    for &(_, importee_chunk) in &routes.1 {
      imports.entry(importee_chunk).or_default();
    }
    for (record_idx, rec) in module.import_records.iter_enumerated() {
      let target_chunk = rec.resolved_module.and_then(|m| chunk_graph.module_to_chunk[m]);
      if rec.kind == ImportKind::DynamicImport
        || (rec.kind == ImportKind::Import && target_chunk != Some(chunk_idx))
        || crate::utils::native_records::omittable_star_evaluation(
          self.options,
          self.link_output,
          module,
          record_idx,
        )
        || crate::utils::native_records::native_cell_evaluation_dependencies(
          self.options,
          self.link_output,
          &self.ast_table,
          module,
          record_idx,
        )
        .is_some()
      {
        continue;
      }
      if let Some(target_chunk) = target_chunk {
        imports.entry(target_chunk).or_default();
      }
    }
    if meta.has_side_effectful_runtime_dep
      && let Some(runtime_chunk) = chunk_graph.module_to_chunk[runtime_id]
      && runtime_chunk != chunk_idx
    {
      imports.entry(runtime_chunk).or_default();
    }
    let rank = self.native_authored_import_rank(chunk_graph, chunk_idx)?;
    imports.sort_by(|a, _, b, _| {
      let key = |chunk: &ChunkIdx| {
        (rank.get(chunk).copied().unwrap_or(usize::MAX), chunk_graph.chunk_table[*chunk].exec_order)
      };
      key(a).cmp(&key(b))
    });
    Some(imports)
  }
}
