//! The early check of a property of a CommonJS module that an emitted entry in a code splitting
//! group re-exports. See internal-docs/code-splitting/implementation.md ("Shared CommonJS property
//! reads").

use std::cell::{OnceCell, RefCell};

use oxc_index::IndexVec;
use oxc_str::CompactStr;
use rolldown_common::{
  ChunkIdx, ChunkKind, ExportsKind, ImportKind, Module, ModuleIdx, NamespaceAlias,
  SymbolOrMemberExprRef, SymbolRef, WrapKind,
};
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::{FxHashMap, FxHashSet};

use super::GenerateStage;
use crate::{
  chunk_graph::ChunkGraph,
  utils::chunk::{
    commonjs_export_order::CommonJsExportOrder,
    namespace_alias_origin::{NamespaceAliasOrigin, namespace_alias_origin},
  },
};

/// The result of the check for one CommonJS module: `Err` has the rule that rejects the module.
pub(super) type CommonJsVerdict = Result<(), &'static str>;

/// The state of the check in one run of the chunk optimizer. The check runs before the optimizer
/// applies its decisions, so it uses the chunks that the decisions give:
/// - A common chunk that the optimizer merges into an entry chunk (`CommonChunkMerge`) is that
///   entry chunk.
/// - The reads of an emitted entry in a code splitting group are in the group chunk, or in the
///   facade of the entry. The check accepts a module only if both chunks can share the read. Thus
///   the result for a module does not depend on the facade decisions of the entries.
///
/// The facts about the module graph are computed at the first check, and the result for each
/// module is kept.
pub(super) struct CommonJsPropertyGuard {
  /// The entry chunk into which the optimizer merges each common chunk.
  merged_into: FxHashMap<ChunkIdx, ChunkIdx>,
  /// The group chunk of each emitted entry in a code splitting group.
  group_of_entry: FxHashMap<ModuleIdx, ChunkIdx>,
  /// The dynamic entries whose facades the optimizer removes, with the chunks that hold them.
  eliminated_entries: FxHashMap<ModuleIdx, ChunkIdx>,
  facts: OnceCell<CommonJsFacts>,
  exposed: OnceCell<FxHashSet<ChunkIdx>>,
  import_graph: OnceCell<EarlyImportGraph>,
  verdicts: RefCell<FxHashMap<ModuleIdx, CommonJsVerdict>>,
}

/// A node of `EarlyImportGraph`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum EarlyImportNode {
  Chunk(ChunkIdx),
  /// A module that is in no chunk.
  Module(ModuleIdx),
}

/// The static imports of the chunks of the guard (`GenerateStage::early_reaches`).
struct EarlyImportGraph {
  /// The nodes that each chunk imports.
  chunk_edges: IndexVec<ChunkIdx, FxHashSet<EarlyImportNode>>,
  /// The nodes that each module in no chunk imports.
  module_edges: FxHashMap<ModuleIdx, FxHashSet<EarlyImportNode>>,
}

impl CommonJsPropertyGuard {
  pub(super) fn new(
    merges: impl Iterator<Item = (ChunkIdx, ChunkIdx)>,
    groups: &FxHashMap<ChunkIdx, Vec<ChunkIdx>>,
    eliminated_entries: impl Iterator<Item = (ModuleIdx, ChunkIdx)>,
    chunk_graph: &ChunkGraph,
  ) -> Self {
    let group_of_entry = groups
      .iter()
      .flat_map(|(group, entry_chunks)| {
        entry_chunks.iter().filter_map(move |entry_chunk| {
          Some((chunk_graph.chunk_table.get(*entry_chunk)?.entry_module_idx()?, *group))
        })
      })
      .collect();
    Self {
      merged_into: merges.collect(),
      group_of_entry,
      eliminated_entries: eliminated_entries.collect(),
      facts: OnceCell::new(),
      exposed: OnceCell::new(),
      import_graph: OnceCell::new(),
      verdicts: RefCell::default(),
    }
  }

  /// The chunk that holds `module_idx` after the merges of the optimizer.
  fn chunk_of(&self, chunk_graph: &ChunkGraph, module_idx: ModuleIdx) -> Option<ChunkIdx> {
    let chunk_idx = chunk_graph.module_to_chunk[module_idx]?;
    Some(self.merged_into.get(&chunk_idx).copied().unwrap_or(chunk_idx))
  }
}

/// The facts about the module graph that the check uses.
#[derive(Default)]
struct CommonJsFacts {
  /// The exports of entry modules that read a property of each CommonJS module: the entry module
  /// and the canonical symbol of the export.
  readers: FxHashMap<ModuleIdx, Vec<(ModuleIdx, SymbolRef)>>,
  /// The properties of each CommonJS module that a resolved export of a module reads.
  properties: FxHashMap<ModuleIdx, FxIndexSet<CompactStr>>,
  /// The modules whose resolved exports read a property of each CommonJS module. A module that
  /// exports the namespace of a CommonJS module (`export * as lib from './lib.cjs'`) re-exports all
  /// properties of the module.
  reexporters: FxHashMap<ModuleIdx, FxIndexSet<ModuleIdx>>,
  /// The CommonJS modules whose namespace an entry exports, with the entry.
  exported_namespaces: FxHashMap<ModuleIdx, ModuleIdx>,
  /// The CommonJS modules that code reads outside a re-export, with the module that reads.
  code_reads: FxHashMap<ModuleIdx, ModuleIdx>,
  /// Code reads a property that a barrel gets from two or more CommonJS modules.
  has_unknown_code_read: bool,
  /// The modules whose namespace object code can read, with the way: an included statement
  /// references the namespace object, a `require()` loads the module, or an entry exports the
  /// namespace object.
  namespace_escapes: FxHashMap<ModuleIdx, &'static str>,
  /// The modules that an included `import()` loads.
  dynamic_imports: FxHashSet<ModuleIdx>,
}

impl GenerateStage<'_> {
  /// Whether a facade can keep an export of a property of the CommonJS module `module_idx`. A kept
  /// facade reads the property when it loads, and `share_commonjs_property_reads` gives one read
  /// to the exports of the properties of the module in all chunks. The module passes only if that
  /// pass can read all properties of the module in one accessor, in the order of Node:
  /// - Code reads no property of the module outside a re-export. The accessor reads all
  ///   properties when the first chunk calls it, so a later read in code gets a different value.
  ///   A bare `import './module.cjs'` reads no property.
  /// - No entry exports the namespace of the module, for example with
  ///   `export * as lib from './module.cjs'`. A getter of the namespace reads a property each time
  ///   that code reads it.
  /// - Code cannot read the namespace object of a module that re-exports a property of the module.
  ///   A module that exports the namespace of the module re-exports all its properties. A getter
  ///   of that object reads the property each time that code reads it. Code can read the object if
  ///   an included statement references it, if a `require()` loads the module, or if an entry
  ///   exports the object.
  /// - An `import()` of a module that re-exports a property gives the exports of a chunk
  ///   (`import_loads_chunk_exports`). An `import()` of another module reads its namespace object.
  /// - Each read can call the accessor, or can read a public export of an entry chunk that calls
  ///   it (`GenerateStage::entry_binding`). To call the accessor, a chunk other than the chunk of
  ///   the module must import it, so that chunk must be able to get a new export.
  /// - No read uses the namespace of a barrel.
  /// - If the modules read two or more properties, the source position of each property is known
  ///   (`CommonJsExportOrder`).
  ///
  /// Otherwise the entry gets the decision without the export: the property stops the facade, as
  /// any other property of a namespace does. In a debug build, the last pass asserts the
  /// prediction (`share_commonjs_property_reads`).
  pub(super) fn commonjs_property_verdict(
    &self,
    chunk_graph: &ChunkGraph,
    guard: &CommonJsPropertyGuard,
    module_idx: ModuleIdx,
  ) -> CommonJsVerdict {
    if let Some(verdict) = guard.verdicts.borrow().get(&module_idx) {
      return *verdict;
    }
    let verdict = self.compute_commonjs_property_verdict(chunk_graph, guard, module_idx);
    tracing::debug!(
      module = self.link_output.module_table[module_idx].stable_id().as_str(),
      verdict = if verdict.is_ok() { "allow" } else { "reject" },
      rule = verdict.err().unwrap_or(""),
      "the early check of a CommonJS module"
    );
    guard.verdicts.borrow_mut().insert(module_idx, verdict);
    verdict
  }

  /// The check rejects a property that a barrel gets through `export *` from two or more CommonJS
  /// modules (`NamespaceAliasOrigin::CommonJs(None)`). The output cannot tell which module gives
  /// the property, so the check cannot find the accessor of the read.
  pub(super) fn commonjs_barrel_property_verdict(
    &self,
    entry_idx: ModuleIdx,
    ns_alias: &NamespaceAlias,
  ) -> CommonJsVerdict {
    let symbols = &self.link_output.symbol_db;
    let rule = "a barrel gets the property from two or more CommonJS modules";
    tracing::debug!(
      module = symbols.canonical_ref_for(ns_alias.namespace_ref).name(symbols),
      entry = self.link_output.module_table[entry_idx].stable_id().as_str(),
      verdict = "reject",
      rule,
      "the early check of a CommonJS module"
    );
    Err(rule)
  }

  fn compute_commonjs_property_verdict(
    &self,
    chunk_graph: &ChunkGraph,
    guard: &CommonJsPropertyGuard,
    module_idx: ModuleIdx,
  ) -> CommonJsVerdict {
    let facts = guard.facts.get_or_init(|| self.commonjs_facts());
    if facts.has_unknown_code_read {
      return Err("code reads a property that a barrel gets from two or more CommonJS modules");
    }
    if let Some(reader) = facts.code_reads.get(&module_idx) {
      tracing::debug!(
        module = self.link_output.module_table[module_idx].stable_id().as_str(),
        reader = self.link_output.module_table[*reader].stable_id().as_str(),
        "a module reads a CommonJS module outside a re-export"
      );
      return Err("a module reads the CommonJS module outside a re-export");
    }
    if let Some(entry) = facts.exported_namespaces.get(&module_idx) {
      tracing::debug!(
        module = self.link_output.module_table[module_idx].stable_id().as_str(),
        entry = self.link_output.module_table[*entry].stable_id().as_str(),
        "an entry exports the namespace of a CommonJS module"
      );
      return Err("an entry exports the namespace of the CommonJS module");
    }
    let symbols = &self.link_output.symbol_db;
    let module_table = &self.link_output.module_table;
    // The namespace object of a module that re-exports a property has a getter that reads the
    // property when code reads it. The shared read does not cover such a getter, except the getter
    // of an entry whose facade the check keeps.
    for &reexporter in facts.reexporters.get(&module_idx).into_iter().flatten() {
      if let Some(way) = facts.namespace_escapes.get(&reexporter) {
        tracing::debug!(
          module = module_table[module_idx].stable_id().as_str(),
          reexporter = module_table[reexporter].stable_id().as_str(),
          way,
          "code can read the namespace object of a module that re-exports a CommonJS property"
        );
        return Err("code can read the namespace object of a module that re-exports a property");
      }
      if facts.dynamic_imports.contains(&reexporter)
        && !self.import_loads_chunk_exports(chunk_graph, guard, reexporter, module_idx)
      {
        tracing::debug!(
          module = module_table[module_idx].stable_id().as_str(),
          reexporter = module_table[reexporter].stable_id().as_str(),
          "an import() reads the namespace object of a module that re-exports a CommonJS property"
        );
        return Err(
          "an import() reads the namespace object of a module that re-exports a property",
        );
      }
    }
    let Some(owner) = guard.chunk_of(chunk_graph, module_idx) else {
      return Err("the CommonJS module is in no chunk");
    };
    for &(entry_idx, binding) in facts.readers.get(&module_idx).into_iter().flatten() {
      let ns_alias = symbols
        .get(binding)
        .namespace_alias
        .as_ref()
        .expect("a reader reads a property of a namespace");
      let namespace_ref = ns_alias.namespace_ref;
      if module_table[namespace_ref.owner]
        .as_normal()
        .is_none_or(|owner| owner.namespace_object_ref == namespace_ref)
      {
        return Err("a read uses the namespace of a barrel");
      }
      let Some(namespace_chunk) = guard.chunk_of(chunk_graph, namespace_ref.owner) else {
        return Err("the namespace of a read is in no chunk");
      };
      let read =
        ReadOfModule { module_idx, owner, namespace_chunk, property: &ns_alias.property_name };
      let can_read = if let Some(&group) = guard.group_of_entry.get(&entry_idx) {
        // The group chunk has the reads of an entry without a facade, and the namespace object of
        // the entry. The facade of the entry imports the group chunk.
        self.can_read_in_chunk(chunk_graph, guard, group, &read)
          && self.can_read_in_facade(chunk_graph, guard, group, &read)
      } else if guard.eliminated_entries.contains_key(&entry_idx) {
        // The exports of the entry are a namespace object, which the check does not cover.
        true
      } else if let Some(&entry_chunk) = chunk_graph.entry_module_to_entry_chunk.get(&entry_idx) {
        match guard.chunk_of(chunk_graph, entry_idx) {
          Some(module_chunk) if module_chunk != entry_chunk => {
            self.can_read_in_facade(chunk_graph, guard, module_chunk, &read)
          }
          _ => self.can_read_in_chunk(chunk_graph, guard, entry_chunk, &read),
        }
      } else {
        true
      };
      if !can_read {
        tracing::debug!(
          module = module_table[module_idx].stable_id().as_str(),
          entry = module_table[entry_idx].stable_id().as_str(),
          "a read of a CommonJS property cannot reach the accessor"
        );
        return Err("a read of a property cannot reach the accessor");
      }
    }
    if let Some(properties) = facts.properties.get(&module_idx)
      && properties.len() >= 2
    {
      let Some(ast) = self.ast_table[module_idx].as_ref() else {
        return Err("the CommonJS module has no AST");
      };
      let export_order = CommonJsExportOrder::new(ast.program());
      if properties.iter().any(|property| export_order.position(property).is_none()) {
        return Err("a property has no known source position");
      }
    }
    Ok(())
  }

  /// Whether the chunk `reader` can call the accessor of `read`, or can read a public export of an
  /// entry chunk that calls it. The chunk imports the namespace of the read from
  /// `ReadOfModule::namespace_chunk`. That chunk imports the wrapper of the module from
  /// `ReadOfModule::owner`.
  fn can_read_in_chunk(
    &self,
    chunk_graph: &ChunkGraph,
    guard: &CommonJsPropertyGuard,
    reader: ChunkIdx,
    read: &ReadOfModule,
  ) -> bool {
    let ReadOfModule { owner, namespace_chunk, .. } = *read;
    let passable = |chunk_idx| self.early_passable(chunk_graph, guard, chunk_idx);
    reader == owner
      || (passable(owner)
        && (reader == namespace_chunk || namespace_chunk == owner || passable(namespace_chunk)))
      || self.early_entry_binding(chunk_graph, guard, Some(reader), read)
  }

  /// Whether a facade that imports the chunk `module_chunk` can call the accessor of `read`, or can
  /// read a public export of an entry chunk that calls it. No chunk imports a facade, so the
  /// public export has no cycle.
  fn can_read_in_facade(
    &self,
    chunk_graph: &ChunkGraph,
    guard: &CommonJsPropertyGuard,
    module_chunk: ChunkIdx,
    read: &ReadOfModule,
  ) -> bool {
    let ReadOfModule { owner, namespace_chunk, .. } = *read;
    let passable = |chunk_idx| self.early_passable(chunk_graph, guard, chunk_idx);
    (passable(owner)
      && (namespace_chunk == owner
        || passable(namespace_chunk)
        || (passable(module_chunk) && (module_chunk == owner || module_chunk == namespace_chunk))))
      || self.early_entry_binding(chunk_graph, guard, None, read)
  }

  /// Whether a read in the chunk `reader` can read the public export of the entry chunk that
  /// declares its namespace (`GenerateStage::entry_binding`). `None` is a facade, which no chunk
  /// imports.
  fn early_entry_binding(
    &self,
    chunk_graph: &ChunkGraph,
    guard: &CommonJsPropertyGuard,
    reader: Option<ChunkIdx>,
    read: &ReadOfModule,
  ) -> bool {
    let ReadOfModule { owner, namespace_chunk, .. } = *read;
    let ChunkKind::EntryPoint { module: entry_idx, .. } =
      chunk_graph.chunk_table[namespace_chunk].kind
    else {
      return false;
    };
    let facts = guard.facts.get().expect("the check computes the facts first");
    let meta = &self.link_output.metas[entry_idx];
    let runs_before_exports = !self.options.is_strict_execution_order_enabled()
      && match meta.wrap_kind() {
        WrapKind::None => true,
        WrapKind::Esm => meta.wrapper_ref.is_some(),
        WrapKind::Cjs => false,
      };
    runs_before_exports
      && (namespace_chunk == owner || self.early_passable(chunk_graph, guard, owner))
      && self.entry_exports_property(chunk_graph, guard, facts, read, entry_idx)
      && reader
        .is_none_or(|reader| !self.early_reaches(chunk_graph, guard, namespace_chunk, reader))
  }

  /// Whether the entry `entry_idx` of the chunk `ReadOfModule::namespace_chunk` exports the
  /// property of `read` from a binding that the chunk declares (`GenerateStage::is_entry_binding`).
  fn entry_exports_property(
    &self,
    chunk_graph: &ChunkGraph,
    guard: &CommonJsPropertyGuard,
    facts: &CommonJsFacts,
    read: &ReadOfModule,
    entry_idx: ModuleIdx,
  ) -> bool {
    let symbols = &self.link_output.symbol_db;
    facts.readers.get(&read.module_idx).into_iter().flatten().any(|&(reader, binding)| {
      reader == entry_idx
        && guard.chunk_of(chunk_graph, binding.owner) == Some(read.namespace_chunk)
        && symbols
          .get(binding)
          .namespace_alias
          .as_ref()
          .is_some_and(|ns_alias| ns_alias.property_name == read.property)
    })
  }

  /// Whether an `import()` of `module_idx` gives the exports of a chunk, which the shared read
  /// covers. This is true for an entry that keeps its own entry chunk, and for an emitted entry in a
  /// code splitting group whose facade the check keeps for an ES binding of another chunk
  /// (`kept_for_other_chunk`). An `import()` of such an entry reads the namespace object of the
  /// entry, and the shared read covers its getters (`ReadUse::NamespaceGetter`). For other modules,
  /// the `import()` reads the namespace object of the module.
  fn import_loads_chunk_exports(
    &self,
    chunk_graph: &ChunkGraph,
    guard: &CommonJsPropertyGuard,
    module_idx: ModuleIdx,
    commonjs_idx: ModuleIdx,
  ) -> bool {
    if let Some(&group) = guard.group_of_entry.get(&module_idx) {
      return self.kept_for_other_chunk(chunk_graph, module_idx, group, commonjs_idx);
    }
    !guard.eliminated_entries.contains_key(&module_idx)
      && chunk_graph.entry_module_to_entry_chunk.contains_key(&module_idx)
  }

  /// Whether the cross-chunk check keeps the facade of the emitted entry `module_idx` in the group
  /// chunk `group` if the check accepts the CommonJS module `commonjs_idx`. The facade stays only if
  /// each property of a CommonJS module that the entry exports comes from `commonjs_idx`. A property
  /// of another CommonJS module can depend on the result for that module.
  fn kept_for_other_chunk(
    &self,
    chunk_graph: &ChunkGraph,
    module_idx: ModuleIdx,
    group: ChunkIdx,
    commonjs_idx: ModuleIdx,
  ) -> bool {
    let meta = &self.link_output.metas[module_idx];
    let is_commonjs = self.link_output.module_table[module_idx]
      .as_normal()
      .is_some_and(|module| module.exports_kind.is_commonjs());
    if is_commonjs || meta.has_dynamic_exports {
      return false;
    }
    let symbols = &self.link_output.symbol_db;
    let mut has_binding_from_other_chunk = false;
    for name in meta.sorted_and_non_ambiguous_resolved_exports.keys() {
      let binding = symbols.canonical_ref_for(meta.resolved_exports[name].symbol_ref);
      if let Some(ns_alias) = symbols.get(binding).namespace_alias.as_ref() {
        if namespace_alias_origin(self.link_output, ns_alias)
          == NamespaceAliasOrigin::CommonJs(Some(commonjs_idx))
        {
          continue;
        }
        return false;
      }
      if self.link_output.metas[binding.owner].import_record_ns_to_cjs_module.contains_key(&binding)
      {
        return false;
      }
      has_binding_from_other_chunk |=
        chunk_graph.module_to_chunk[binding.owner].is_some_and(|owner_chunk| owner_chunk != group);
    }
    has_binding_from_other_chunk
  }

  /// Whether the chunk can get a new export: its signature allows a new export, and no `import()`
  /// in the bundle can load it as a namespace. The check treats each chunk that holds an entry with
  /// dynamic importers as such a chunk (`chunks_exposed_to_dynamic_import`).
  fn early_passable(
    &self,
    chunk_graph: &ChunkGraph,
    guard: &CommonJsPropertyGuard,
    chunk_idx: ChunkIdx,
  ) -> bool {
    let exposed = guard.exposed.get_or_init(|| {
      let has_dynamic_importers = |module_idx: ModuleIdx| {
        self.link_output.module_table[module_idx]
          .as_normal()
          .is_some_and(|module| !module.dynamic_importers.is_empty())
      };
      let mut exposed = FxHashSet::default();
      for &entry_idx in self.link_output.entries.keys() {
        if !has_dynamic_importers(entry_idx) {
          continue;
        }
        exposed.extend(chunk_graph.entry_module_to_entry_chunk.get(&entry_idx).copied());
        exposed.extend(guard.chunk_of(chunk_graph, entry_idx));
        exposed.extend(
          guard
            .eliminated_entries
            .get(&entry_idx)
            .map(|chunk_idx| guard.merged_into.get(chunk_idx).copied().unwrap_or(*chunk_idx)),
        );
      }
      exposed
    });
    self.signature_allows_new_exports(&chunk_graph.chunk_table[chunk_idx])
      && !exposed.contains(&chunk_idx)
  }

  /// Whether the chunk `from` imports the chunk `to` through static imports, directly or through
  /// other chunks. The edges are the static import records of the modules, and an edge from each
  /// chunk to the chunk of the runtime. A module that is in no chunk, for example a barrel without
  /// side effects, is a node of the walk. Thus an import through such a module gets to the chunks
  /// that its records import.
  ///
  /// For each static import of the final chunk graph between two chunks that exist at the check
  /// and are not facades, this graph has a path. The exception is an import of runtime helpers
  /// only: `try_merge_runtime_chunk` can merge the runtime into a common chunk after the check.
  fn early_reaches(
    &self,
    chunk_graph: &ChunkGraph,
    guard: &CommonJsPropertyGuard,
    from: ChunkIdx,
    to: ChunkIdx,
  ) -> bool {
    let graph = guard.import_graph.get_or_init(|| {
      let module_table = &self.link_output.module_table;
      let node_of = |module_idx| {
        guard
          .chunk_of(chunk_graph, module_idx)
          .map_or(EarlyImportNode::Module(module_idx), EarlyImportNode::Chunk)
      };
      let mut chunk_edges: IndexVec<ChunkIdx, FxHashSet<EarlyImportNode>> =
        IndexVec::from_vec(vec![FxHashSet::default(); chunk_graph.chunk_table.len()]);
      let mut module_edges: FxHashMap<ModuleIdx, FxHashSet<EarlyImportNode>> = FxHashMap::default();
      for module in module_table.modules.iter().filter_map(Module::as_normal) {
        let node = node_of(module.idx);
        if matches!(node, EarlyImportNode::Chunk(_))
          && !self.link_output.metas[module.idx].is_included
        {
          continue;
        }
        for record in &module.import_records {
          if record.kind == ImportKind::DynamicImport {
            continue;
          }
          let Some(importee) =
            record.resolved_module.filter(|importee| module_table[*importee].as_normal().is_some())
          else {
            continue;
          };
          let importee_node = node_of(importee);
          if importee_node == node {
            continue;
          }
          match node {
            EarlyImportNode::Chunk(chunk_idx) => chunk_edges[chunk_idx].insert(importee_node),
            EarlyImportNode::Module(module_idx) => {
              module_edges.entry(module_idx).or_default().insert(importee_node)
            }
          };
        }
      }
      if let Some(runtime_chunk) = guard.chunk_of(chunk_graph, self.link_output.runtime.id()) {
        for (chunk_idx, importees) in chunk_edges.iter_mut_enumerated() {
          if chunk_idx != runtime_chunk {
            importees.insert(EarlyImportNode::Chunk(runtime_chunk));
          }
        }
      }
      EarlyImportGraph { chunk_edges, module_edges }
    });
    let target = EarlyImportNode::Chunk(to);
    let mut visited = FxHashSet::from_iter([EarlyImportNode::Chunk(from)]);
    let mut stack = vec![EarlyImportNode::Chunk(from)];
    while let Some(node) = stack.pop() {
      let importees = match node {
        EarlyImportNode::Chunk(chunk_idx) => Some(&graph.chunk_edges[chunk_idx]),
        EarlyImportNode::Module(module_idx) => graph.module_edges.get(&module_idx),
      };
      for &importee in importees.into_iter().flatten() {
        if importee == target {
          return true;
        }
        if visited.insert(importee) {
          stack.push(importee);
        }
      }
    }
    false
  }

  /// One walk of the modules for the check.
  fn commonjs_facts(&self) -> CommonJsFacts {
    let link_output = &*self.link_output;
    let symbols = &link_output.symbol_db;
    let is_commonjs = |module_idx: ModuleIdx| {
      link_output.module_table[module_idx]
        .as_normal()
        .is_some_and(|module| matches!(module.exports_kind, ExportsKind::CommonJs))
    };
    // The namespace of each import record of a CommonJS module.
    let mut record_namespaces: FxHashMap<SymbolRef, ModuleIdx> = FxHashMap::default();
    for module in link_output.module_table.modules.iter().filter_map(Module::as_normal) {
      for record in &module.import_records {
        if let Some(importee) = record.resolved_module
          && is_commonjs(importee)
        {
          record_namespaces.insert(record.namespace_ref, importee);
        }
      }
    }
    // The CommonJS module that gives the value of a symbol: `Some(None)` if a barrel gets the value
    // from two or more CommonJS modules.
    let commonjs_source = |symbol_ref: SymbolRef| -> Option<Option<ModuleIdx>> {
      let canonical = symbols.canonical_ref_for(symbol_ref);
      if let Some(&module_idx) = record_namespaces.get(&canonical) {
        return Some(Some(module_idx));
      }
      let ns_alias = symbols.get(canonical).namespace_alias.as_ref()?;
      if let Some(&module_idx) = record_namespaces.get(&ns_alias.namespace_ref) {
        return Some(Some(module_idx));
      }
      match namespace_alias_origin(link_output, ns_alias) {
        NamespaceAliasOrigin::CommonJs(module_idx) => Some(module_idx),
        NamespaceAliasOrigin::External(_) | NamespaceAliasOrigin::Unknown => None,
      }
    };

    // The module of each namespace object.
    let namespace_objects: FxHashMap<SymbolRef, ModuleIdx> = link_output
      .module_table
      .modules
      .iter()
      .filter_map(Module::as_normal)
      .map(|module| (module.namespace_object_ref, module.idx))
      .collect();
    let namespace_object_of = |symbol_ref: SymbolRef| {
      namespace_objects.get(&symbols.canonical_ref_for(symbol_ref)).copied()
    };

    let mut facts = CommonJsFacts::default();
    let mut included_stmts = 0usize;
    for module in link_output.module_table.modules.iter().filter_map(Module::as_normal) {
      let meta = &link_output.metas[module.idx];
      let is_entry = link_output.entries.contains_key(&module.idx);
      for name in meta.sorted_and_non_ambiguous_resolved_exports.keys() {
        let binding = symbols.canonical_ref_for(meta.resolved_exports[name].symbol_ref);
        if is_entry && let Some(namespace_module) = namespace_object_of(binding) {
          facts.namespace_escapes.entry(namespace_module).or_insert("an entry exports it");
        }
        // The module exports the namespace of a CommonJS module, so it re-exports all properties
        // of that module.
        if let Some(&source) = record_namespaces.get(&binding) {
          facts.reexporters.entry(source).or_default().insert(module.idx);
          if is_entry {
            facts.exported_namespaces.entry(source).or_insert(module.idx);
          }
        }
        let Some(ns_alias) = symbols.get(binding).namespace_alias.as_ref() else {
          continue;
        };
        if let Some(Some(source)) = commonjs_source(binding) {
          if ns_alias.property_name != "default" {
            facts.properties.entry(source).or_default().insert(ns_alias.property_name.clone());
          }
          facts.reexporters.entry(source).or_default().insert(module.idx);
          if is_entry {
            facts.readers.entry(source).or_default().push((module.idx, binding));
          }
        }
      }

      if !meta.is_included {
        continue;
      }
      // A read in code: an included statement that references a property or a namespace of a
      // CommonJS module, or a `require()` or an `import()` of such a module. The statement of the
      // namespace object of the module is not a read in code.
      for (stmt_idx, stmt_info) in
        link_output.stmt_infos[module.idx].iter_enumerated_without_namespace_stmt()
      {
        if !meta.stmt_info_included.has_bit(stmt_idx) {
          continue;
        }
        included_stmts += 1;
        for &record_idx in &stmt_info.import_records {
          let record = &module.import_records[record_idx];
          let Some(importee) = record.resolved_module else {
            continue;
          };
          match record.kind {
            ImportKind::Import => {}
            _ if is_commonjs(importee) => {
              facts.code_reads.entry(importee).or_insert(module.idx);
            }
            ImportKind::DynamicImport => {
              facts.dynamic_imports.insert(importee);
            }
            ImportKind::Require => {
              facts.namespace_escapes.entry(importee).or_insert("a require() loads it");
            }
            _ => {}
          }
        }
        for reference in &stmt_info.referenced_symbols {
          if let SymbolOrMemberExprRef::Symbol(symbol_ref) = reference
            && let Some(namespace_module) = namespace_object_of(*symbol_ref)
          {
            facts
              .namespace_escapes
              .entry(namespace_module)
              .or_insert("an included statement references it");
          }
          let resolved = match reference {
            SymbolOrMemberExprRef::Symbol(_) => None,
            SymbolOrMemberExprRef::MemberExpr(member_expr) => {
              member_expr.represent_symbol_ref(&meta.resolved_member_expr_refs)
            }
          };
          for symbol_ref in std::iter::once(*reference.symbol_ref()).chain(resolved) {
            match commonjs_source(symbol_ref) {
              Some(Some(source)) => {
                facts.code_reads.entry(source).or_insert(module.idx);
              }
              Some(None) => facts.has_unknown_code_read = true,
              None => {}
            }
          }
        }
      }
    }
    tracing::debug!(
      included_stmts,
      code_reads = facts.code_reads.len(),
      readers = facts.readers.len(),
      namespace_escapes = facts.namespace_escapes.len(),
      dynamic_imports = facts.dynamic_imports.len(),
      "the facts of the early check of CommonJS modules"
    );
    facts
  }
}

/// A read of a property of a CommonJS module.
#[derive(Clone, Copy)]
struct ReadOfModule<'a> {
  /// The CommonJS module.
  module_idx: ModuleIdx,
  /// The chunk of the CommonJS module.
  owner: ChunkIdx,
  /// The chunk that declares the namespace of the read.
  namespace_chunk: ChunkIdx,
  /// The property that the read reads.
  property: &'a CompactStr,
}
