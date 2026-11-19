//! Shared reads of a CommonJS property that two or more chunks export. See
//! internal-docs/code-splitting/implementation.md ("Shared CommonJS property reads").

use std::collections::VecDeque;

use oxc_index::IndexVec;
use oxc_str::CompactStr;
use rolldown_common::{
  ChunkIdx, ChunkKind, InlineConstMode, ModuleIdx, OutputFormat, SymbolRef, TaggedSymbolRef,
  UsedSymbolRefsView, WrapKind,
};
use rolldown_utils::{concat_string, ecmascript::legitimize_identifier_name, indexmap::FxIndexMap};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{
  GenerateStage,
  compute_cross_chunk_links::CrossChunkLinkState,
  order_wrap_state::{OrderSyntheticStmt, OrderWrapState},
};
use crate::{
  chunk_graph::ChunkGraph,
  module_finalizers::{
    namespace_alias_constant, namespace_object_has_getter, narrowed_namespace_export_names,
  },
  utils::chunk::namespace_alias_origin::{NamespaceAliasOrigin, namespace_alias_origin},
};

/// The functions that give the exports of a CommonJS property one read. The chunks that export
/// the property call a function in place of the property read.
#[derive(Debug, Default)]
pub struct SharedCommonJsReads {
  /// The function that an export calls, by the chunk and the canonical symbol of the export.
  calls: FxHashMap<(ChunkIdx, SymbolRef), SymbolRef>,
  /// The function that a getter of a namespace object calls, by the chunk of the namespace object
  /// and the canonical symbol of the export (`ReadUse::NamespaceGetter`).
  namespace_calls: FxHashMap<(ChunkIdx, SymbolRef), SymbolRef>,
  /// The public export of an entry chunk that an export reads in place of the property, by the
  /// chunk and the canonical symbol of the export. The value is the binding of the public export.
  /// The entry chunk reads the property, and the chunk imports the binding from it.
  bindings: FxHashMap<(ChunkIdx, SymbolRef), SymbolRef>,
  /// The binding that a getter of a namespace object returns in place of the property read, by
  /// the chunk of the namespace object and the canonical symbol of the export.
  namespace_bindings: FxHashMap<(ChunkIdx, SymbolRef), SymbolRef>,
  /// The bindings of `bindings` and `namespace_bindings` that each chunk imports.
  binding_imports: FxHashMap<ChunkIdx, Vec<SymbolRef>>,
  /// The functions that a chunk declares, in the order of the decision.
  declarations: FxHashMap<ChunkIdx, Vec<SharedReadDeclaration>>,
}

/// A function that a chunk declares for a shared read.
#[derive(Debug)]
pub enum SharedReadDeclaration {
  /// The function that reads the property. The chunk writes this code:
  ///
  /// ```js
  /// var value, is_read;
  /// function accessor(ns) {
  ///   if (!is_read) {
  ///     try {
  ///       value = ns.property;
  ///     } catch (e) {
  ///       if (ns == null) throw e;
  ///     }
  ///     is_read = true;
  ///   }
  ///   return value;
  /// }
  /// ```
  ///
  /// A getter that throws gives `undefined`, and the getter runs one time. A namespace that is
  /// `null` or `undefined` is not a read of the property, so the error stops the caller, and a later
  /// call reads the property. A chunk cycle can call the accessor before the chunk of the namespace
  /// runs.
  Accessor { accessor: SymbolRef, value: SymbolRef, is_read: SymbolRef, property: CompactStr },
  /// A function that calls `next` in the chunk that this chunk imports. A chunk that does not
  /// import the chunk of the accessor calls the accessor through forwards. The chunk writes
  /// `function forward(ns) { return next(ns); }`.
  Forward { forward: SymbolRef, next: SymbolRef },
}

impl SharedCommonJsReads {
  pub fn is_empty(&self) -> bool {
    self.calls.is_empty() && self.namespace_calls.is_empty() && self.binding_imports.is_empty()
  }

  /// The function that the export `export_ref` of the chunk `chunk_idx` calls with its namespace,
  /// in place of the property read. `export_ref` is a canonical symbol.
  pub fn call(&self, chunk_idx: ChunkIdx, export_ref: SymbolRef) -> Option<SymbolRef> {
    self.calls.get(&(chunk_idx, export_ref)).copied()
  }

  /// The function that the getter of the export `export_ref` in a namespace object of the chunk
  /// `chunk_idx` calls with its namespace, in place of the property read. `export_ref` is a
  /// canonical symbol.
  pub fn namespace_call(&self, chunk_idx: ChunkIdx, export_ref: SymbolRef) -> Option<SymbolRef> {
    self.namespace_calls.get(&(chunk_idx, export_ref)).copied()
  }

  /// The binding of the public export of an entry chunk that the export `export_ref` of the chunk
  /// `chunk_idx` gives in place of the property read. `export_ref` is a canonical symbol. The
  /// chunk imports the binding (`binding_imports`).
  pub fn binding(&self, chunk_idx: ChunkIdx, export_ref: SymbolRef) -> Option<SymbolRef> {
    self.bindings.get(&(chunk_idx, export_ref)).copied()
  }

  /// The binding that the getter of the export `export_ref` in a namespace object of the chunk
  /// `chunk_idx` returns in place of the property read. `export_ref` is a canonical symbol.
  pub fn namespace_binding(&self, chunk_idx: ChunkIdx, export_ref: SymbolRef) -> Option<SymbolRef> {
    self.namespace_bindings.get(&(chunk_idx, export_ref)).copied()
  }

  /// The bindings of public exports of entry chunks that the chunk `chunk_idx` imports. The link
  /// pass imports each binding, not the namespace of its namespace alias.
  pub fn binding_imports(&self, chunk_idx: ChunkIdx) -> &[SymbolRef] {
    self.binding_imports.get(&chunk_idx).map_or(&[], Vec::as_slice)
  }

  pub fn declarations(&self, chunk_idx: ChunkIdx) -> &[SharedReadDeclaration] {
    self.declarations.get(&chunk_idx).map_or(&[], Vec::as_slice)
  }

  /// The exports that call a function, by chunk and canonical symbol.
  pub fn exports(&self) -> impl Iterator<Item = (ChunkIdx, SymbolRef)> + '_ {
    self.calls.keys().copied()
  }

  /// The bindings that each chunk imports (`binding_imports`).
  pub fn all_binding_imports(&self) -> impl Iterator<Item = (ChunkIdx, SymbolRef)> + '_ {
    self
      .binding_imports
      .iter()
      .flat_map(|(chunk_idx, bindings)| bindings.iter().map(move |binding| (*chunk_idx, *binding)))
  }
}

/// The value that a shared read saves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ReadSource {
  /// The CommonJS module that gives the property (`NamespaceAliasOrigin::CommonJs(Some(_))`). A
  /// namespace of the module and a barrel that gets the property from it read the same property
  /// of `module.exports`.
  Module(ModuleIdx),
  /// The namespace that the exports read, for a property that a barrel gets through `export *`
  /// from two or more CommonJS modules (`NamespaceAliasOrigin::CommonJs(None)`). The output cannot
  /// tell which module gives the property, so only the reads of this namespace share.
  Namespace(SymbolRef),
}

/// The code that reads a CommonJS property in a chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadUse {
  /// An export of the chunk (`render_chunk_exports`).
  Export,
  /// A getter of the namespace object of the entry module of a marked facade
  /// (`generate_declaration_of_module_namespace_object`). An `import()` of the entry in the bundle
  /// gives this namespace object, so the facade and the namespace are two ways to load the entry.
  /// The getter calls the function when code reads the export, not when the namespace object is
  /// created.
  NamespaceGetter,
}

/// The reads of each CommonJS property: the chunk, the canonical symbol of the export, and the code
/// that reads the property.
type PropertyReads = FxIndexMap<(ReadSource, CompactStr), Vec<(ChunkIdx, SymbolRef, ReadUse)>>;

impl GenerateStage<'_> {
  /// Give one read to the exports of a CommonJS property in two or more chunks, if a facade that
  /// `Chunk::is_cross_chunk_export_facade` marks is one of these chunks. A getter of the namespace
  /// object of the entry of such a facade also reads the property (`ReadUse::NamespaceGetter`).
  /// `link_state` is the final cross-chunk derivation. The function declares the accessor in the
  /// chunk of the CommonJS module, and a forward in each chunk between that chunk and a chunk that
  /// reads the property. It uses only the edges of `link_state`, so the chunk graph gets no new
  /// edge. The synthetic statements of `order_state` register the new symbols, and the caller
  /// derives the links again.
  ///
  /// The accessor and the forwards are new exports. A chunk gets one only if the new export does
  /// not change its signature, and if no `import()` in the bundle can see the new key
  /// (`chunks_exposed_to_dynamic_import`). The paths use only the edges that the module-level
  /// prediction covers (`CrossChunkLinkState::predicted_importee_table`).
  ///
  /// A read that cannot reach the accessor through such chunks reads a public export of the entry
  /// chunk that declares the namespace of the read (`namespace_chunk`). This occurs only if that
  /// entry chunk can call the accessor, exports the property under a name of its entry
  /// (`is_entry_binding`), and is an importee of the chunk of the read
  /// (`SharedCommonJsReads::binding`). Otherwise the read stays in its chunk. An ESM export reads
  /// the property one time, when its chunk runs. Thus ESM output gets an accessor only if two or
  /// more chunks can call it. A CJS export is a getter, so in CJS output the getter of such a
  /// public export also calls the accessor.
  #[expect(clippy::too_many_lines)]
  pub(super) fn share_commonjs_property_reads(
    &mut self,
    chunk_graph: &ChunkGraph,
    link_state: &CrossChunkLinkState,
    used_symbol_refs: UsedSymbolRefsView<'_>,
    order_state: &mut OrderWrapState,
  ) -> SharedCommonJsReads {
    let mut shared = SharedCommonJsReads::default();
    if !matches!(self.options.format, OutputFormat::Esm | OutputFormat::Cjs)
      || !chunk_graph.chunk_table.iter().any(|chunk| chunk.is_cross_chunk_export_facade)
    {
      return shared;
    }
    // The chunks that the output writes as files.
    let files: IndexVec<ChunkIdx, bool> = chunk_graph
      .chunk_table
      .indices()
      .map(|chunk_idx| {
        !chunk_graph.post_chunk_optimization_operations.contains_key(&chunk_idx)
          && !self.inline_state.is_record(chunk_idx)
      })
      .collect();
    let is_file = |chunk_idx: ChunkIdx| files[chunk_idx];

    let reads = self.collect_commonjs_property_reads(
      chunk_graph,
      link_state,
      used_symbol_refs,
      order_state,
      &files,
    );

    // The accessor and each forward are new exports of their chunks. A chunk can get one only if
    // its signature allows new exports and no `import()` in the bundle can see the new key.
    let exposed = self.chunks_exposed_to_dynamic_import(chunk_graph);
    let passable: IndexVec<ChunkIdx, bool> = chunk_graph
      .chunk_table
      .iter_enumerated()
      .map(|(chunk_idx, chunk)| {
        is_file(chunk_idx)
          && self.signature_allows_new_exports(chunk)
          && !exposed.contains(&chunk_idx)
      })
      .collect();
    let can_pass_on = |chunk_idx: ChunkIdx| passable[chunk_idx];

    // The edges of `link_state` that the module-level prediction covers. A new import on such an
    // edge does not change the result of the already-loaded analysis (`predicted_importee_table`).
    let importees = link_state.predicted_importee_table(self.link_output.runtime.id());
    // The chunks that import each chunk through these edges.
    let mut importers: IndexVec<ChunkIdx, Vec<ChunkIdx>> =
      IndexVec::from_vec(vec![vec![]; chunk_graph.chunk_table.len()]);
    for (importer, importees) in importees.iter_enumerated() {
      if !is_file(importer) {
        continue;
      }
      for &importee in importees {
        if importee != importer && is_file(importee) {
          importers[importee].push(importer);
        }
      }
    }
    for list in &mut importers {
      list.sort_unstable();
    }
    let imports =
      |importer: ChunkIdx, importee: ChunkIdx| importers[importee].binary_search(&importer).is_ok();
    let is_facade =
      |chunk_idx: ChunkIdx| chunk_graph.chunk_table[chunk_idx].is_cross_chunk_export_facade;

    for ((source, property), uses) in reads {
      let mut chunks = uses.iter().map(|(chunk_idx, _, _)| *chunk_idx).collect::<Vec<_>>();
      chunks.sort_unstable();
      chunks.dedup();
      if chunks.len() < 2 || !chunks.iter().any(|chunk_idx| is_facade(*chunk_idx)) {
        continue;
      }
      let owner = match source {
        ReadSource::Module(module_idx) => chunk_graph.module_to_chunk[module_idx],
        ReadSource::Namespace(namespace_ref) => {
          link_state.symbol_chunk(namespace_ref, &self.link_output.symbol_db)
        }
      };
      let Some(owner) = owner.filter(|chunk_idx| is_file(*chunk_idx)) else {
        continue;
      };

      // Each chunk that can call the accessor gets the next chunk on its path to the owner. A
      // chunk passes the call on through a forward, so only a chunk that can get a new export is
      // on a path. The search passes through common chunks before entry chunks, so an entry chunk
      // gets a forward only if no path through common chunks reaches the chunk that needs it. The
      // owner exports the accessor. If it cannot get a new export, only the owner can call the
      // accessor.
      let mut next_hop: FxHashMap<ChunkIdx, ChunkIdx> = FxHashMap::default();
      if can_pass_on(owner) {
        let mut visited = FxHashSet::from_iter([owner]);
        let mut queue = VecDeque::from([owner]);
        let mut entry_queue = VecDeque::new();
        while let Some(chunk_idx) = queue.pop_front().or_else(|| entry_queue.pop_front()) {
          for &importer in &importers[chunk_idx] {
            if !visited.insert(importer) {
              continue;
            }
            next_hop.insert(importer, chunk_idx);
            if !can_pass_on(importer) {
              continue;
            }
            if matches!(chunk_graph.chunk_table[importer].kind, ChunkKind::Common) {
              queue.push_back(importer);
            } else {
              entry_queue.push_back(importer);
            }
          }
        }
      } else {
        tracing::debug!(
          chunk = owner.raw(),
          property = property.as_str(),
          "the chunk of a CommonJS property cannot export an accessor"
        );
      }
      let (reachable, unreachable): (Vec<ChunkIdx>, Vec<ChunkIdx>) = chunks
        .iter()
        .partition(|chunk_idx| **chunk_idx == owner || next_hop.contains_key(*chunk_idx));

      // A read that cannot reach the accessor reads the property through a public export of the
      // entry chunk that holds the namespace of the read, if that entry chunk reads the property
      // and can call the accessor. The chunk of the read imports the namespace from that entry
      // chunk, so the import of the binding keeps the same edges. The export has a name of the
      // entry, so the import adds no key (`deconflict_exported_names`). Otherwise the read stays
      // in its chunk.
      let mut binding_chunks = vec![];
      // The chunk, the export and the code of each such read, and the binding that it reads.
      let mut binding_reads = vec![];
      for &(chunk_idx, export_ref, read_use) in &uses {
        if !unreachable.contains(&chunk_idx) {
          continue;
        }
        let host = self
          .namespace_chunk(link_state, export_ref)
          .filter(|host| reachable.contains(host) && imports(chunk_idx, *host));
        let binding = host.and_then(|host| {
          uses.iter().find_map(|&(use_chunk, host_export, host_use)| {
            (use_chunk == host
              && host_use == ReadUse::Export
              && self.is_entry_binding(chunk_graph, link_state, order_state, host, host_export))
            .then_some(host_export)
          })
        });
        let Some(binding) = binding else {
          tracing::debug!(
            chunk = chunk_idx.raw(),
            property = property.as_str(),
            "a read of a CommonJS property cannot reach the accessor through existing edges"
          );
          continue;
        };
        if !binding_chunks.contains(&chunk_idx) {
          binding_chunks.push(chunk_idx);
        }
        binding_reads.push((chunk_idx, export_ref, read_use, binding));
      }
      // The reads share one read if two or more chunks have them and a facade is one of these
      // chunks. Otherwise each read stays in its chunk.
      if reachable.len() + binding_chunks.len() < 2
        || !reachable.iter().chain(&binding_chunks).any(|chunk_idx| is_facade(*chunk_idx))
      {
        continue;
      }
      for &(chunk_idx, export_ref, read_use, binding) in &binding_reads {
        let bindings = match read_use {
          ReadUse::Export => &mut shared.bindings,
          ReadUse::NamespaceGetter => &mut shared.namespace_bindings,
        };
        bindings.insert((chunk_idx, export_ref), binding);
        let imported = shared.binding_imports.entry(chunk_idx).or_default();
        if !imported.contains(&binding) {
          imported.push(binding);
        }
      }
      // In ESM output, an export reads the property one time, when its chunk runs. Thus the reads
      // need no accessor if only one chunk reads the property. In CJS output, an export is a getter
      // that reads the property at each access. Thus the getter of an entry chunk calls the
      // accessor if another chunk reads its export.
      let needs_accessor = reachable.len() >= 2
        || (matches!(self.options.format, OutputFormat::Cjs) && !binding_chunks.is_empty());
      if !needs_accessor {
        continue;
      }

      let (owner_module, base_name) = match source {
        ReadSource::Module(module_idx) => {
          (module_idx, self.link_output.module_table[module_idx].repr_name().to_string())
        }
        ReadSource::Namespace(namespace_ref) => {
          (namespace_ref.owner, namespace_ref.name(&self.link_output.symbol_db).to_string())
        }
      };
      let base_name = concat_string!(base_name, "_", legitimize_identifier_name(&property));
      let symbols = &mut self.link_output.symbol_db;
      let accessor =
        symbols.create_facade_root_symbol_ref(owner_module, &concat_string!("read_", base_name));
      let value = symbols.create_facade_root_symbol_ref(owner_module, &base_name);
      let is_read =
        symbols.create_facade_root_symbol_ref(owner_module, &concat_string!("has_", base_name));
      add_shared_read_statement(order_state, owner, vec![value, is_read, accessor], vec![]);
      shared.declarations.entry(owner).or_default().push(SharedReadDeclaration::Accessor {
        accessor,
        value,
        is_read,
        property,
      });

      // The function that each chunk on a path declares: the accessor in the owner, and a forward
      // in each other chunk.
      let mut callee_in = FxHashMap::from_iter([(owner, accessor)]);
      for &chunk_idx in &reachable {
        let callee = if chunk_idx == owner {
          accessor
        } else {
          self.shared_read_callee(
            next_hop[&chunk_idx],
            &next_hop,
            &mut callee_in,
            owner_module,
            &concat_string!("read_", base_name),
            order_state,
            &mut shared,
          )
        };
        add_shared_read_statement(order_state, chunk_idx, vec![], vec![callee]);
      }
      for (chunk_idx, export_ref, read_use) in uses {
        if unreachable.contains(&chunk_idx) {
          continue;
        }
        let callee = if chunk_idx == owner { accessor } else { callee_in[&next_hop[&chunk_idx]] };
        let calls = match read_use {
          ReadUse::Export => &mut shared.calls,
          ReadUse::NamespaceGetter => &mut shared.namespace_calls,
        };
        calls.insert((chunk_idx, export_ref), callee);
      }
    }
    shared
  }

  /// The chunks whose own namespace an `import()` in the bundle gives, if code can see a new key
  /// in that namespace (`dynamic_entry_namespace_is_observable`). An `import()` of an entry loads
  /// `ChunkGraph::dynamic_import_chunk`. It gives the namespace of that chunk, except when the
  /// importers read an extracted namespace of the entry from the chunk. This is true for a pure
  /// dynamic entry, and also for a user-defined or emitted entry that code in the bundle imports
  /// with `import()`. The setting `allow-extension` permits new exports of the entry file, but
  /// `Object.keys(await import('./app.js'))` can still see a new key.
  ///
  /// The rule is stricter than the rule of runtime placement. A reason in
  /// `dynamic_entry_extraction_skip_reason` is not an exception. Such a namespace can already have
  /// extra keys, but a shared read must not add one more key.
  fn chunks_exposed_to_dynamic_import(&self, chunk_graph: &ChunkGraph) -> FxHashSet<ChunkIdx> {
    chunk_graph
      .entry_module_to_entry_chunk
      .keys()
      .chain(chunk_graph.entry_module_to_dynamic_import_chunk.keys())
      .copied()
      .filter(|&module_idx| {
        self.link_output.module_table[module_idx]
          .as_normal()
          .is_some_and(|module| !module.dynamic_importers.is_empty())
      })
      .filter_map(|module_idx| {
        let chunk_idx = chunk_graph.dynamic_import_chunk(module_idx)?;
        self
          .dynamic_entry_namespace_is_observable(chunk_graph, chunk_idx, module_idx)
          .then_some(chunk_idx)
      })
      .collect()
  }

  /// The chunk that declares the namespace that the export `export_ref` reads its property from.
  /// The chunk of the export imports the namespace from that chunk.
  fn namespace_chunk(
    &self,
    link_state: &CrossChunkLinkState,
    export_ref: SymbolRef,
  ) -> Option<ChunkIdx> {
    let symbols = &self.link_output.symbol_db;
    let ns_alias = symbols.get(export_ref).namespace_alias.as_ref()?;
    link_state.symbol_chunk(symbols.canonical_ref_for(ns_alias.namespace_ref), symbols)
  }

  /// Whether another chunk can read a CommonJS property through the export `export_ref` of the
  /// chunk `chunk_idx`. These conditions are necessary:
  /// - The chunk is an entry chunk, and `export_ref` has a name of the entry
  ///   (`index_chunk_exported_symbols`). An import of the binding then gets this name.
  /// - The chunk declares the binding, so an import of the binding comes from this chunk.
  /// - The entry module is not wrapped, so the binding has its value when the chunk has run.
  fn is_entry_binding(
    &self,
    chunk_graph: &ChunkGraph,
    link_state: &CrossChunkLinkState,
    order_state: &OrderWrapState,
    chunk_idx: ChunkIdx,
    export_ref: SymbolRef,
  ) -> bool {
    let ChunkKind::EntryPoint { module, .. } = chunk_graph.chunk_table[chunk_idx].kind else {
      return false;
    };
    matches!(self.link_output.metas[module].wrap_kind(), WrapKind::None)
      && !order_state.has_order_wrapper(module)
      && link_state.symbol_chunk(export_ref, &self.link_output.symbol_db) == Some(chunk_idx)
      && link_state.index_chunk_exported_symbols[chunk_idx]
        .get(&export_ref)
        .is_some_and(|names| !names.is_empty())
  }

  /// The function that `chunk_idx` declares on the path to the accessor. The function creates the
  /// forward of each chunk on the path one time.
  #[expect(clippy::too_many_arguments)]
  fn shared_read_callee(
    &mut self,
    chunk_idx: ChunkIdx,
    next_hop: &FxHashMap<ChunkIdx, ChunkIdx>,
    callee_in: &mut FxHashMap<ChunkIdx, SymbolRef>,
    owner_module: ModuleIdx,
    name: &str,
    order_state: &mut OrderWrapState,
    shared: &mut SharedCommonJsReads,
  ) -> SymbolRef {
    if let Some(callee) = callee_in.get(&chunk_idx) {
      return *callee;
    }
    let next = self.shared_read_callee(
      next_hop[&chunk_idx],
      next_hop,
      callee_in,
      owner_module,
      name,
      order_state,
      shared,
    );
    let forward = self.link_output.symbol_db.create_facade_root_symbol_ref(owner_module, name);
    add_shared_read_statement(order_state, chunk_idx, vec![forward], vec![next]);
    shared
      .declarations
      .entry(chunk_idx)
      .or_default()
      .push(SharedReadDeclaration::Forward { forward, next });
    callee_in.insert(chunk_idx, forward);
    forward
  }

  /// The reads of each CommonJS property, by the value that they read. The list has these reads:
  /// - The exports that the commit of `link_state` gives to the chunks
  ///   (`deconflict_exported_names`), in chunk order. A chunk renders such an export as a read of
  ///   the property from a namespace (`render_chunk_exports`).
  /// - The getters of the namespace object of the entry module of each marked facade, if the bundle
  ///   keeps the namespace object (`ReadUse::NamespaceGetter`). The list has the exports that get a
  ///   getter in the finalizer (`namespace_object_has_getter`), except a getter that returns an
  ///   inlined constant (`namespace_alias_constant`).
  ///
  /// The list does not have `default`: different namespaces of a module can give different values
  /// for it (`__toESM`).
  fn collect_commonjs_property_reads(
    &self,
    chunk_graph: &ChunkGraph,
    link_state: &CrossChunkLinkState,
    used_symbol_refs: UsedSymbolRefsView<'_>,
    order_state: &OrderWrapState,
    files: &IndexVec<ChunkIdx, bool>,
  ) -> PropertyReads {
    let symbols = &self.link_output.symbol_db;
    let mut reads = PropertyReads::default();
    // `export_ref` is a canonical symbol.
    let mut add = |chunk_idx: ChunkIdx, export_ref: SymbolRef, read_use: ReadUse| {
      let Some(ns_alias) = symbols.get(export_ref).namespace_alias.as_ref() else {
        return;
      };
      if ns_alias.property_name == "default" {
        return;
      }
      let source = match namespace_alias_origin(self.link_output, ns_alias) {
        NamespaceAliasOrigin::CommonJs(Some(module_idx)) => ReadSource::Module(module_idx),
        NamespaceAliasOrigin::CommonJs(None) => {
          ReadSource::Namespace(symbols.canonical_ref_for(ns_alias.namespace_ref))
        }
        NamespaceAliasOrigin::External(_) | NamespaceAliasOrigin::Unknown => return,
      };
      reads
        .entry((source, ns_alias.property_name.clone()))
        .or_default()
        .push((chunk_idx, export_ref, read_use));
    };
    for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
      if !files[chunk_idx]
        || chunk
          .entry_module(&self.link_output.module_table)
          .is_some_and(|module| !module.exports_kind.is_esm())
      {
        continue;
      }
      let mut exports = link_state.index_chunk_exported_symbols[chunk_idx]
        .iter()
        .filter(|(symbol_ref, names)| {
          self.cross_chunk_symbol_is_live(
            used_symbol_refs,
            &link_state.order_live_symbols,
            **symbol_ref,
          ) && (names.is_empty() || self.link_output.retained_export_symbols.contains(symbol_ref))
        })
        .map(|(symbol_ref, _)| symbols.canonical_ref_for(*symbol_ref))
        .collect::<Vec<_>>();
      exports.sort_unstable();
      exports.dedup();
      for export_ref in exports {
        add(chunk_idx, export_ref, ReadUse::Export);
      }
    }
    // A getter that returns an inlined constant does not read the property
    // (`namespace_getter_returned_expr`). Only the `All` mode of `inlineConst` puts the constant of
    // a CommonJS property in a getter of a namespace object.
    let getters_inline_constants = self
      .options
      .optimization
      .inline_const
      .is_some_and(|config| matches!(config.mode, InlineConstMode::All));
    for (facade_idx, facade) in chunk_graph.chunk_table.iter_enumerated() {
      if !facade.is_cross_chunk_export_facade || !files[facade_idx] {
        continue;
      }
      let Some(entry_module_idx) = facade.entry_module_idx() else {
        continue;
      };
      let meta = &self.link_output.metas[entry_module_idx];
      let (Some(module), Some(chunk_idx)) = (
        self.link_output.module_table[entry_module_idx].as_normal(),
        chunk_graph.module_to_chunk[entry_module_idx].filter(|chunk_idx| files[*chunk_idx]),
      ) else {
        continue;
      };
      if !meta.namespace_included {
        continue;
      }
      let export_names = narrowed_namespace_export_names(
        meta,
        module.namespace_object_ref,
        order_state,
        chunk_graph,
        self.link_output.dynamic_import_exports_usage_map.get(&entry_module_idx),
      );
      for (export, resolved_export) in meta.canonical_exports(false) {
        if !namespace_object_has_getter(
          export_names,
          export,
          resolved_export.symbol_ref,
          &self.link_output.retained_export_symbols,
          &self.link_output.global_constant_symbol_map,
          symbols,
        ) {
          continue;
        }
        let export_ref = symbols.canonical_ref_for(resolved_export.symbol_ref);
        let returns_constant = getters_inline_constants
          && symbols.get(export_ref).namespace_alias.as_ref().is_some_and(|ns_alias| {
            namespace_alias_constant(
              module,
              resolved_export.symbol_ref,
              ns_alias,
              symbols,
              &self.link_output.module_table.modules,
              &self.link_output.metas,
              &self.link_output.global_constant_symbol_map,
            )
            .is_some()
          });
        if !returns_constant {
          add(chunk_idx, export_ref, ReadUse::NamespaceGetter);
        }
      }
    }
    reads
  }
}

impl GenerateStage<'_> {
  /// Each export that calls a shared read is an export of its chunk after the commit. Otherwise the
  /// decision used another export list than the renderer (`get_export_items`).
  #[cfg(debug_assertions)]
  pub(super) fn debug_assert_shared_reads_are_exported(&self, chunk_graph: &ChunkGraph) {
    let symbols = &self.link_output.symbol_db;
    for (chunk_idx, export_ref) in self.shared_commonjs_reads.exports() {
      assert!(
        chunk_graph.chunk_table[chunk_idx]
          .exports_to_other_chunks
          .keys()
          .any(|symbol_ref| symbols.canonical_ref_for(*symbol_ref) == export_ref),
        "chunk {chunk_idx:?} has a shared CommonJS property read for `{}`, but does not export it",
        export_ref.name(symbols),
      );
    }
    // A chunk that reads the public export of an entry chunk imports the binding from the entry
    // chunk, and the entry chunk exports the binding only under names of its entry. Thus the read
    // adds no key to the entry chunk.
    for (chunk_idx, binding) in self.shared_commonjs_reads.all_binding_imports() {
      let host = symbols.get(binding).chunk_idx.expect("the binding is in a chunk");
      let host_chunk = &chunk_graph.chunk_table[host];
      let entry = host_chunk.entry_module_idx().expect("the binding is in an entry chunk");
      assert!(
        host_chunk.exports_to_other_chunks[&binding]
          .iter()
          .all(|name| self.link_output.metas[entry].resolved_exports.contains_key(name)),
        "chunk {host:?} exports the binding `{}` under a name that its entry does not export",
        binding.name(symbols),
      );
      assert!(
        chunk_graph.chunk_table[chunk_idx]
          .imports_from_other_chunks
          .get(&host)
          .is_some_and(|items| items.iter().any(|item| item.import_ref == binding)),
        "chunk {chunk_idx:?} reads the binding `{}`, but does not import it from chunk {host:?}",
        binding.name(symbols),
      );
    }
  }
}

/// Registers the symbols that `chunk_idx` declares and calls for a shared read. The link pass
/// gives each declared symbol to the chunk and imports each called symbol from the chunk that
/// declares it.
fn add_shared_read_statement(
  order_state: &mut OrderWrapState,
  chunk_idx: ChunkIdx,
  declared_symbols: Vec<SymbolRef>,
  referenced_symbols: Vec<SymbolRef>,
) {
  let stmt_idx = order_state.add_synthetic_statement(OrderSyntheticStmt {
    declared_symbols: declared_symbols.into_iter().map(TaggedSymbolRef::normal).collect(),
    referenced_symbols,
    runtime_helpers: rolldown_common::RuntimeHelper::default(),
    chunk: None,
  });
  order_state.assign_synthetic_statement_chunk(stmt_idx, chunk_idx);
}
