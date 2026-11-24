//! Shared reads of a CommonJS property that two or more chunks export. See
//! internal-docs/code-splitting/implementation.md ("Shared CommonJS property reads").

use std::{cell::OnceCell, collections::VecDeque};

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
  type_alias::IndexEcmaAst,
  utils::chunk::{
    commonjs_export_order::CommonJsExportOrder,
    namespace_alias_origin::{NamespaceAliasOrigin, namespace_alias_origin},
  },
};

/// The functions that give the exports of a CommonJS property one read. The chunks that export
/// the property call a function in place of the property read.
#[derive(Debug, Default)]
pub struct SharedCommonJsReads {
  /// The call that an export makes, by the chunk and the canonical symbol of the export.
  calls: FxHashMap<(ChunkIdx, SymbolRef), SharedRead>,
  /// The call that a getter of a namespace object makes, by the chunk of the namespace object and
  /// the canonical symbol of the export (`ReadUse::NamespaceGetter`).
  namespace_calls: FxHashMap<(ChunkIdx, SymbolRef), SharedRead>,
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

/// The call that replaces a read of a CommonJS property.
#[derive(Debug, Clone)]
pub struct SharedRead {
  /// The accessor, or a forward that calls it.
  pub callee: SymbolRef,
  /// The property that the read takes from the result of the call, if the accessor reads the
  /// properties of a module (`SharedReadDeclaration::ModuleAccessor`). Otherwise the result is the
  /// value of the property.
  pub property: Option<CompactStr>,
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
  /// The function that reads the properties of a CommonJS module, in the order in which Node reads
  /// them (`GenerateStage::plan_module_reads`). The chunk writes this code:
  ///
  /// ```js
  /// var module_values;
  /// function accessor(ns) {
  ///   if (!module_values) {
  ///     var values = { __proto__: null };
  ///     try {
  ///       values.x = ns.x;
  ///     } catch (e) {
  ///       if (ns == null) throw e;
  ///     }
  ///     try {
  ///       values.y = ns.y;
  ///     } catch (e) {}
  ///     module_values = values;
  ///   }
  ///   return module_values;
  /// }
  /// ```
  ///
  /// A read takes its property from the result: `accessor(ns).x`. Each getter runs one time. A
  /// getter that throws gives `undefined`, and the other properties are still read, as in Node. A
  /// namespace that is `null` or `undefined` stops the first read, and a later call reads all
  /// properties. The object of the values has no prototype, so a property `__proto__` is an own
  /// property of the object.
  ModuleAccessor { accessor: SymbolRef, values: SymbolRef, properties: Vec<CompactStr> },
  /// A function that calls `next` in the chunk that this chunk imports. A chunk that does not
  /// import the chunk of the accessor calls the accessor through forwards. The chunk writes
  /// `function forward(ns) { return next(ns); }`.
  Forward { forward: SymbolRef, next: SymbolRef },
}

impl SharedCommonJsReads {
  pub fn is_empty(&self) -> bool {
    self.calls.is_empty() && self.namespace_calls.is_empty() && self.binding_imports.is_empty()
  }

  /// The call that the export `export_ref` of the chunk `chunk_idx` makes with its namespace, in
  /// place of the property read. `export_ref` is a canonical symbol.
  pub fn call(&self, chunk_idx: ChunkIdx, export_ref: SymbolRef) -> Option<&SharedRead> {
    self.calls.get(&(chunk_idx, export_ref))
  }

  /// The call that the getter of the export `export_ref` in a namespace object of the chunk
  /// `chunk_idx` makes with its namespace, in place of the property read. `export_ref` is a
  /// canonical symbol.
  pub fn namespace_call(&self, chunk_idx: ChunkIdx, export_ref: SymbolRef) -> Option<&SharedRead> {
    self.namespace_calls.get(&(chunk_idx, export_ref))
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
  #[cfg(debug_assertions)]
  pub fn exports(&self) -> impl Iterator<Item = (ChunkIdx, SymbolRef)> + '_ {
    self.calls.keys().copied()
  }

  /// The bindings that each chunk imports (`binding_imports`).
  #[cfg(debug_assertions)]
  pub fn all_binding_imports(&self) -> impl Iterator<Item = (ChunkIdx, SymbolRef)> + '_ {
    self
      .binding_imports
      .iter()
      .flat_map(|(chunk_idx, bindings)| bindings.iter().map(move |binding| (*chunk_idx, *binding)))
  }

  /// The read `export_ref` of the chunk `chunk_idx` gives the public export `binding` of an entry
  /// chunk in place of the property read, and the chunk imports `binding`.
  fn add_binding(
    &mut self,
    chunk_idx: ChunkIdx,
    export_ref: SymbolRef,
    read_use: ReadUse,
    binding: SymbolRef,
  ) {
    let bindings = match read_use {
      ReadUse::Export => &mut self.bindings,
      ReadUse::NamespaceGetter => &mut self.namespace_bindings,
    };
    bindings.insert((chunk_idx, export_ref), binding);
    let imported = self.binding_imports.entry(chunk_idx).or_default();
    if !imported.contains(&binding) {
      imported.push(binding);
    }
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

/// A read of a CommonJS property: the chunk, the canonical symbol of the export, and the code that
/// reads the property.
type ReadOfProperty = (ChunkIdx, SymbolRef, ReadUse);

/// The reads of each CommonJS property.
type PropertyReads = FxIndexMap<(ReadSource, CompactStr), Vec<ReadOfProperty>>;

/// A source of CommonJS properties that a marked facade reads: the number of its properties that
/// the chunks read, and all reads of the property if the source has one property.
struct FacadeReadSource {
  source: ReadSource,
  properties: usize,
  single_uses: Vec<ReadOfProperty>,
}

/// The sources of CommonJS properties that a marked facade reads, in the order of `reads`.
fn facade_read_sources(chunk_graph: &ChunkGraph, reads: &PropertyReads) -> Vec<FacadeReadSource> {
  let is_facade =
    |chunk_idx: ChunkIdx| chunk_graph.chunk_table[chunk_idx].is_cross_chunk_export_facade;
  let mut sources: FxIndexMap<ReadSource, (bool, Vec<&Vec<ReadOfProperty>>)> =
    FxIndexMap::default();
  for ((source, _), uses) in reads {
    let (read_by_facade, properties) = sources.entry(*source).or_default();
    *read_by_facade |= uses.iter().any(|(chunk_idx, _, _)| is_facade(*chunk_idx));
    properties.push(uses);
  }
  sources
    .into_iter()
    .filter(|(_, (read_by_facade, _))| *read_by_facade)
    .map(|(source, (_, properties))| FacadeReadSource {
      source,
      properties: properties.len(),
      single_uses: if properties.len() == 1 { properties[0].clone() } else { vec![] },
    })
    .collect()
}

/// The chunks that a shared read can use, and the edges between them
/// (`GenerateStage::read_graph`).
struct ReadGraph {
  /// The chunks that the output writes as files.
  files: IndexVec<ChunkIdx, bool>,
  /// The chunks that can get a new export: an accessor or a forward.
  passable: IndexVec<ChunkIdx, bool>,
  /// The chunks that import each chunk, in ascending order.
  importers: IndexVec<ChunkIdx, Vec<ChunkIdx>>,
  /// All static importees of each chunk (`CrossChunkLinkState::static_importee_table`). The first
  /// call of `ReadGraph::reaches` makes the table.
  static_importees: OnceCell<IndexVec<ChunkIdx, FxHashSet<ChunkIdx>>>,
}

impl ReadGraph {
  fn is_file(&self, chunk_idx: ChunkIdx) -> bool {
    self.files[chunk_idx]
  }

  fn can_pass_on(&self, chunk_idx: ChunkIdx) -> bool {
    self.passable[chunk_idx]
  }

  fn imports(&self, importer: ChunkIdx, importee: ChunkIdx) -> bool {
    self.importers[importee].binary_search(&importer).is_ok()
  }

  /// Whether the chunk `from` imports the chunk `to` through static imports, directly or through
  /// other chunks. The search uses all static edges, also an edge that imports only runtime
  /// helpers.
  fn reaches(
    &self,
    chunk_graph: &ChunkGraph,
    link_state: &CrossChunkLinkState,
    from: ChunkIdx,
    to: ChunkIdx,
  ) -> bool {
    let importees =
      self.static_importees.get_or_init(|| link_state.static_importee_table(chunk_graph));
    let mut visited = FxHashSet::from_iter([from]);
    let mut stack = vec![from];
    while let Some(chunk_idx) = stack.pop() {
      for &importee in &importees[chunk_idx] {
        if importee == to {
          return true;
        }
        if visited.insert(importee) {
          stack.push(importee);
        }
      }
    }
    false
  }

  /// The next chunk on the path to `owner`, for each chunk that can call the accessor in `owner`.
  /// A chunk passes the call on through a forward, so only a chunk that can get a new export is
  /// on a path. The search passes through common chunks before entry chunks, so an entry chunk
  /// gets a forward only if no path through common chunks reaches the chunk that needs it. The
  /// owner exports the accessor. If it cannot get a new export, only the owner can call the
  /// accessor, and the map is empty.
  fn next_hops(&self, chunk_graph: &ChunkGraph, owner: ChunkIdx) -> FxHashMap<ChunkIdx, ChunkIdx> {
    let mut next_hop = FxHashMap::default();
    if !self.can_pass_on(owner) {
      return next_hop;
    }
    let mut visited = FxHashSet::from_iter([owner]);
    let mut queue = VecDeque::from([owner]);
    let mut entry_queue = VecDeque::new();
    while let Some(chunk_idx) = queue.pop_front().or_else(|| entry_queue.pop_front()) {
      for &importer in &self.importers[chunk_idx] {
        if !visited.insert(importer) {
          continue;
        }
        next_hop.insert(importer, chunk_idx);
        if !self.can_pass_on(importer) {
          continue;
        }
        if matches!(chunk_graph.chunk_table[importer].kind, ChunkKind::Common) {
          queue.push_back(importer);
        } else {
          entry_queue.push_back(importer);
        }
      }
    }
    next_hop
  }
}

/// The accessor of the CommonJS properties of one module (`GenerateStage::plan_module_reads`).
struct ModuleReadPlan {
  /// The chunk of the module. It declares the accessor.
  owner: ChunkIdx,
  /// All properties of the module that the chunks read, in the order in which Node reads them.
  properties: Vec<CompactStr>,
  /// The next chunk on the path to `owner`, for each chunk that can call the accessor
  /// (`ReadGraph::next_hops`).
  next_hop: FxHashMap<ChunkIdx, ChunkIdx>,
  /// The reads that cannot call the accessor, by chunk and export, and the public export of an
  /// entry chunk that each of these reads reads instead (`GenerateStage::entry_binding`).
  bindings: FxHashMap<(ChunkIdx, SymbolRef), SymbolRef>,
}

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
  /// prediction covers (`CrossChunkLinkState::predicted_importees`).
  ///
  /// A read that cannot reach the accessor through such chunks reads a public export of the entry
  /// chunk that declares the namespace of the read (`namespace_chunk`). This occurs only if that
  /// entry chunk can call the accessor, exports the property under a name of its entry
  /// (`is_entry_binding`), is an importee of the chunk of the read, and does not import the chunk
  /// of the read through static imports (`SharedCommonJsReads::binding`). Otherwise the read stays
  /// in its chunk. An ESM export reads
  /// the property one time, when its chunk runs. Thus ESM output gets an accessor only if two or
  /// more chunks can call it. A CJS export is a getter, so in CJS output the getter of such a
  /// public export also calls the accessor.
  ///
  /// If a facade reads a property of a CommonJS module, one accessor can read all properties of
  /// the module that the chunks read, in the order in which Node reads them (`plan_module_reads`).
  /// Then each read of these properties calls this accessor, or reads a public export of an entry
  /// chunk that calls it. The steps above do not apply to these reads.
  pub(super) fn share_commonjs_property_reads(
    &mut self,
    chunk_graph: &ChunkGraph,
    link_state: &CrossChunkLinkState,
    used_symbol_refs: UsedSymbolRefsView<'_>,
    order_state: &mut OrderWrapState,
    ast_table: &IndexEcmaAst,
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

    let reads = self.collect_commonjs_property_reads(
      chunk_graph,
      link_state,
      used_symbol_refs,
      order_state,
      &files,
    );

    if reads.is_empty() {
      return shared;
    }

    let graph = self.read_graph(chunk_graph, link_state, files);
    let is_facade =
      |chunk_idx: ChunkIdx| chunk_graph.chunk_table[chunk_idx].is_cross_chunk_export_facade;

    let module_plans =
      self.plan_module_reads(chunk_graph, link_state, order_state, ast_table, &reads, &graph);
    for (module_idx, plan) in &module_plans {
      self.share_module_reads(*module_idx, plan, &reads, order_state, &mut shared);
    }
    let facade_sources = facade_read_sources(chunk_graph, &reads);

    for ((source, property), uses) in reads {
      if let ReadSource::Module(module_idx) = source
        && module_plans.contains_key(&module_idx)
      {
        continue;
      }
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
      let Some(owner) = owner.filter(|chunk_idx| graph.is_file(*chunk_idx)) else {
        continue;
      };

      if !graph.can_pass_on(owner) {
        tracing::debug!(
          chunk = owner.raw(),
          property = property.as_str(),
          "the chunk of a CommonJS property cannot export an accessor"
        );
      }
      let next_hop = graph.next_hops(chunk_graph, owner);
      let can_call = |chunk_idx: ChunkIdx| chunk_idx == owner || next_hop.contains_key(&chunk_idx);
      let (reachable, unreachable): (Vec<ChunkIdx>, Vec<ChunkIdx>) =
        chunks.iter().partition(|chunk_idx| can_call(**chunk_idx));

      // A read that cannot reach the accessor reads the property through a public export of an
      // entry chunk (`entry_binding`). Otherwise the read stays in its chunk.
      let mut binding_chunks = vec![];
      // The chunk, the export and the code of each such read, and the binding that it reads.
      let mut binding_reads = vec![];
      for &(chunk_idx, export_ref, read_use) in &uses {
        if !unreachable.contains(&chunk_idx) {
          continue;
        }
        let binding = self.entry_binding(
          chunk_graph,
          link_state,
          order_state,
          &graph,
          &uses,
          (chunk_idx, export_ref),
          can_call,
        );
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
        shared.add_binding(chunk_idx, export_ref, read_use, binding);
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
        calls.insert((chunk_idx, export_ref), SharedRead { callee, property: None });
      }
    }
    self.check_facade_read_outcomes(chunk_graph, &facade_sources, &module_plans, &shared);
    shared
  }

  /// Checks that each source that a marked facade reads gets one read of its properties. The early
  /// check (`commonjs_property_verdict`) accepts a module only if this pass can do that, so a
  /// different result is a defect of the early check.
  fn check_facade_read_outcomes(
    &self,
    chunk_graph: &ChunkGraph,
    sources: &[FacadeReadSource],
    module_plans: &FxIndexMap<ModuleIdx, ModuleReadPlan>,
    shared: &SharedCommonJsReads,
  ) {
    let symbols = &self.link_output.symbol_db;
    let module_table = &self.link_output.module_table;
    for FacadeReadSource { source, properties, single_uses } in sources {
      let (name, outcome, reason) = match *source {
        ReadSource::Namespace(namespace_ref) => (
          namespace_ref.name(symbols).to_string(),
          "not ok",
          "a facade reads a property that a barrel gets from two or more CommonJS modules",
        ),
        ReadSource::Module(module_idx) => {
          let name = module_table[module_idx].stable_id().to_string();
          if module_plans.contains_key(&module_idx) {
            (name, "ok", "one accessor reads all properties")
          } else if *properties == 1 {
            // A marked facade saves one read for all its exports of a property from one namespace
            // (`save_commonjs_property`). Each other read is one read for each symbol. The shared
            // accessor is one more read for all its callers.
            let own_reads = single_uses
              .iter()
              .filter(|&&(chunk_idx, export_ref, read_use)| match read_use {
                ReadUse::Export => {
                  shared.call(chunk_idx, export_ref).is_none()
                    && shared.binding(chunk_idx, export_ref).is_none()
                }
                ReadUse::NamespaceGetter => {
                  shared.namespace_call(chunk_idx, export_ref).is_none()
                    && shared.namespace_binding(chunk_idx, export_ref).is_none()
                }
              })
              .map(|&(chunk_idx, export_ref, read_use)| {
                let is_saved = read_use == ReadUse::Export
                  && chunk_graph.chunk_table[chunk_idx].is_cross_chunk_export_facade;
                let key = symbols
                  .get(export_ref)
                  .namespace_alias
                  .as_ref()
                  .filter(|_| is_saved)
                  .map_or(export_ref, |alias| symbols.canonical_ref_for(alias.namespace_ref));
                (chunk_idx, key, is_saved, read_use == ReadUse::Export)
              })
              .collect::<FxHashSet<_>>()
              .len();
            let calls_accessor =
              single_uses.iter().any(|&(chunk_idx, export_ref, read_use)| match read_use {
                ReadUse::Export => shared.call(chunk_idx, export_ref).is_some(),
                ReadUse::NamespaceGetter => shared.namespace_call(chunk_idx, export_ref).is_some(),
              });
            if own_reads + usize::from(calls_accessor) <= 1 {
              (name, "ok", "the reads of the one property are one read")
            } else if calls_accessor && own_reads == 1 {
              (name, "not ok", "the accessor and another read read the one property")
            } else {
              (name, "not ok", "the reads of the one property are not one read")
            }
          } else {
            (name, "not ok", "no accessor reads all properties of the module")
          }
        }
      };
      tracing::debug!(
        module = name.as_str(),
        outcome,
        reason,
        "the late outcome of a CommonJS module that a facade reads"
      );
      debug_assert!(
        outcome == "ok",
        "the early check accepts a CommonJS source that a facade reads, but {reason}: {name}"
      );
    }
  }

  /// The chunks that a shared read can use, and the edges between them.
  fn read_graph(
    &self,
    chunk_graph: &ChunkGraph,
    link_state: &CrossChunkLinkState,
    files: IndexVec<ChunkIdx, bool>,
  ) -> ReadGraph {
    // The accessor and each forward are new exports of their chunks. A chunk can get one only if
    // its signature allows new exports and no `import()` in the bundle can see the new key.
    let exposed = self.chunks_exposed_to_dynamic_import(chunk_graph);
    let passable = chunk_graph
      .chunk_table
      .iter_enumerated()
      .map(|(chunk_idx, chunk)| {
        files[chunk_idx]
          && self.signature_allows_new_exports(chunk)
          && !exposed.contains(&chunk_idx)
      })
      .collect();

    // The chunks that import each chunk through the edges of `link_state` that the module-level
    // prediction covers. A new import on such an edge does not change the result of the
    // already-loaded analysis (`predicted_importees`). The loop visits the importers in chunk
    // order, so each list is in ascending order for `binary_search`.
    let mut importers: IndexVec<ChunkIdx, Vec<ChunkIdx>> =
      IndexVec::from_vec(vec![vec![]; files.len()]);
    for importer in files.indices().filter(|&chunk_idx| files[chunk_idx]) {
      for importee in link_state.predicted_importees(importer, self.link_output.runtime.id()) {
        if importee != importer && files[importee] {
          importers[importee].push(importer);
        }
      }
    }
    ReadGraph { files, passable, importers, static_importees: OnceCell::new() }
  }

  /// The CommonJS modules whose properties one accessor reads, with these properties in the order
  /// in which Node reads them.
  ///
  /// When an ES module imports a CommonJS module, Node reads all export names of the module one
  /// time, in source order (`CommonJsExportOrder`). A getter can change the value of another
  /// property. If each chunk reads its properties when it loads, a facade that loads first can
  /// change the value of a property that another chunk exports. One accessor for the module reads
  /// the properties in the order of Node. A module gets one accessor in these conditions:
  /// - The chunks read two or more properties of the module, and a facade reads one of them.
  /// - Each read uses the namespace of an import record of the module. The namespace of a barrel
  ///   can give a different value under the same name.
  /// - The source position of each property is known (`CommonJsExportOrder::position`).
  /// - Each read of a property can call the accessor, or can read a public export of an entry
  ///   chunk that calls it (`entry_binding`). Otherwise the getter of a property that keeps its
  ///   own read can run before or after the accessor, and change the values that the accessor
  ///   reads. If the chunk of the module cannot export the accessor, only the reads in this chunk
  ///   can call it (`ReadGraph::next_hops`), and the accessor stays private.
  ///
  /// Otherwise the reads of each property share a read only with the reads of the same property.
  fn plan_module_reads(
    &self,
    chunk_graph: &ChunkGraph,
    link_state: &CrossChunkLinkState,
    order_state: &OrderWrapState,
    ast_table: &IndexEcmaAst,
    reads: &PropertyReads,
    graph: &ReadGraph,
  ) -> FxIndexMap<ModuleIdx, ModuleReadPlan> {
    let symbols = &self.link_output.symbol_db;
    let module_table = &self.link_output.module_table;
    let is_facade =
      |chunk_idx: ChunkIdx| chunk_graph.chunk_table[chunk_idx].is_cross_chunk_export_facade;
    let mut properties_of: FxIndexMap<ModuleIdx, Vec<(&CompactStr, &[ReadOfProperty])>> =
      FxIndexMap::default();
    for ((source, property), uses) in reads {
      if let ReadSource::Module(module_idx) = source {
        properties_of.entry(*module_idx).or_default().push((property, uses.as_slice()));
      }
    }

    let mut plans = FxIndexMap::default();
    for (module_idx, properties) in properties_of {
      let read_by_facade = |uses: &[ReadOfProperty]| uses.iter().any(|(c, _, _)| is_facade(*c));
      if properties.len() < 2 || !properties.iter().any(|(_, uses)| read_by_facade(uses)) {
        continue;
      }
      let names = properties.iter().map(|(property, _)| property.as_str()).collect::<Vec<_>>();
      // `order` is the properties in the order in which the accessor reads them.
      let report = |decision: &str, reason: &str, order: &[&str]| {
        tracing::debug!(
          module = module_table[module_idx].stable_id().as_str(),
          properties = ?names,
          decision,
          reason,
          order = ?order,
          "the reads of the properties of a CommonJS module"
        );
      };

      let reads_barrel =
        properties.iter().flat_map(|(_, uses)| uses.iter()).any(|(_, export, _)| {
          symbols.get(*export).namespace_alias.as_ref().is_some_and(|ns_alias| {
            let namespace_ref = ns_alias.namespace_ref;
            module_table[namespace_ref.owner]
              .as_normal()
              .is_none_or(|owner| owner.namespace_object_ref == namespace_ref)
          })
        });
      if reads_barrel {
        report("unknown order", "a read uses the namespace of a barrel", &[]);
        continue;
      }
      let Some(owner) = chunk_graph.module_to_chunk[module_idx].filter(|c| graph.is_file(*c))
      else {
        report("per property", "the module is not in a chunk that the output writes", &[]);
        continue;
      };

      // A read that cannot call the accessor reads a public export of an entry chunk that calls
      // it (`entry_binding`). If no such export exists, the read reads its property itself. If
      // the accessor read only the other properties, the getter of that property would run
      // outside the order of the accessor. Then an entry that loads alone can get a value that
      // differs both from Node and from the reads of each property. Thus all properties keep
      // their own reads. If the owner cannot export the accessor, `next_hop` is empty. Then only
      // the reads in the owner call the accessor, and each other read needs a public export of
      // the owner.
      let next_hop = graph.next_hops(chunk_graph, owner);
      let can_call = |chunk_idx: ChunkIdx| chunk_idx == owner || next_hop.contains_key(&chunk_idx);
      let mut bindings = FxHashMap::default();
      let all_reach = properties.iter().all(|(_, uses)| {
        uses.iter().all(|&(chunk_idx, export_ref, _)| {
          if can_call(chunk_idx) {
            return true;
          }
          let binding = self.entry_binding(
            chunk_graph,
            link_state,
            order_state,
            graph,
            uses,
            (chunk_idx, export_ref),
            can_call,
          );
          binding.inspect(|binding| {
            bindings.insert((chunk_idx, export_ref), *binding);
          });
          binding.is_some()
        })
      });
      if !all_reach {
        report("per property", "a read of a property cannot reach the accessor", &[]);
        continue;
      }

      // The walk of the AST is the last check, so it runs only for a module that passes the other
      // checks.
      let Some(ast) = ast_table[module_idx].as_ref() else {
        report("unknown order", "the module has no AST", &[]);
        continue;
      };
      let export_order = CommonJsExportOrder::new(ast.program());
      let Some(positions) = properties
        .iter()
        .map(|(property, _)| export_order.position(property))
        .collect::<Option<Vec<_>>>()
      else {
        report("unknown order", "a property has no known source position", &[]);
        continue;
      };
      let mut ordered = properties.iter().zip(positions).collect::<Vec<_>>();
      ordered.sort_unstable_by_key(|(_, position)| *position);
      let order = ordered.iter().map(|((property, _), _)| property.as_str()).collect::<Vec<_>>();
      report("module", "all properties share the accessor", &order);
      let properties = ordered.into_iter().map(|((property, _), _)| (*property).clone()).collect();
      plans.insert(module_idx, ModuleReadPlan { owner, properties, next_hop, bindings });
    }
    plans
  }

  /// Declares the accessor of `plan` in the chunk of the module, and a forward in each chunk on a
  /// path to it. Each read of a property of `plan` calls the accessor and takes the property from
  /// the result, or reads the public export of `ModuleReadPlan::bindings` that calls it.
  fn share_module_reads(
    &mut self,
    module_idx: ModuleIdx,
    plan: &ModuleReadPlan,
    reads: &PropertyReads,
    order_state: &mut OrderWrapState,
    shared: &mut SharedCommonJsReads,
  ) {
    let ModuleReadPlan { owner, properties, next_hop, bindings } = plan;
    let owner = *owner;
    let base_name = self.link_output.module_table[module_idx].repr_name().to_string();
    let accessor_name = concat_string!("read_", base_name);
    let symbols = &mut self.link_output.symbol_db;
    let accessor = symbols.create_facade_root_symbol_ref(module_idx, &accessor_name);
    let values =
      symbols.create_facade_root_symbol_ref(module_idx, &concat_string!(base_name, "_values"));
    add_shared_read_statement(order_state, owner, vec![values, accessor], vec![]);
    shared.declarations.entry(owner).or_default().push(SharedReadDeclaration::ModuleAccessor {
      accessor,
      values,
      properties: properties.clone(),
    });

    // The function that each chunk on a path declares: the accessor in the owner, and a forward
    // in each other chunk.
    let mut callee_in = FxHashMap::from_iter([(owner, accessor)]);
    let mut callers = FxHashSet::default();
    for property in properties {
      let uses = reads
        .get(&(ReadSource::Module(module_idx), property.clone()))
        .expect("the plan has only properties that the chunks read");
      for &(chunk_idx, export_ref, read_use) in uses {
        if let Some(&binding) = bindings.get(&(chunk_idx, export_ref)) {
          shared.add_binding(chunk_idx, export_ref, read_use, binding);
          continue;
        }
        let callee = if chunk_idx == owner {
          accessor
        } else {
          self.shared_read_callee(
            next_hop[&chunk_idx],
            next_hop,
            &mut callee_in,
            module_idx,
            &accessor_name,
            order_state,
            shared,
          )
        };
        if callers.insert(chunk_idx) {
          add_shared_read_statement(order_state, chunk_idx, vec![], vec![callee]);
        }
        let calls = match read_use {
          ReadUse::Export => &mut shared.calls,
          ReadUse::NamespaceGetter => &mut shared.namespace_calls,
        };
        calls
          .insert((chunk_idx, export_ref), SharedRead { callee, property: Some(property.clone()) });
      }
    }
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

  /// The public export that the read `export_ref` of the chunk `chunk_idx` can read in place of the
  /// property, if the chunk cannot call the accessor. `uses` is all reads of the property, and
  /// `can_call` tells if a chunk can call the accessor. The binding is the read of the property in
  /// the chunk that declares the namespace of the read (`namespace_chunk`). These conditions are
  /// necessary:
  /// - That chunk can call the accessor.
  /// - The chunk of the read imports that chunk. The chunk of the read imports the namespace from
  ///   that chunk, so the import of the binding keeps the same edges.
  /// - That chunk reads the property in an export with a name of its entry (`is_entry_binding`).
  ///   The import gets this name, so it adds no key (`deconflict_exported_names`).
  /// - That chunk does not import the chunk of the read through static imports
  ///   (`ReadGraph::reaches`). The binding has its value when that chunk has run. In a cycle of
  ///   chunks, the chunk of the read can run, and read the binding, before that chunk has run.
  #[expect(clippy::too_many_arguments)]
  fn entry_binding(
    &self,
    chunk_graph: &ChunkGraph,
    link_state: &CrossChunkLinkState,
    order_state: &OrderWrapState,
    graph: &ReadGraph,
    uses: &[ReadOfProperty],
    (chunk_idx, export_ref): (ChunkIdx, SymbolRef),
    can_call: impl Fn(ChunkIdx) -> bool,
  ) -> Option<SymbolRef> {
    let host = self
      .namespace_chunk(link_state, export_ref)
      .filter(|host| can_call(*host) && graph.imports(chunk_idx, *host))?;
    let binding = uses.iter().find_map(|&(use_chunk, host_export, host_use)| {
      (use_chunk == host
        && host_use == ReadUse::Export
        && self.is_entry_binding(chunk_graph, link_state, order_state, host, host_export))
      .then_some(host_export)
    })?;
    (!graph.reaches(chunk_graph, link_state, host, chunk_idx)).then_some(binding)
  }

  /// Whether another chunk can read a CommonJS property through the export `export_ref` of the
  /// chunk `chunk_idx`. These conditions are necessary:
  /// - The chunk is an entry chunk, and `export_ref` has a name of the entry
  ///   (`index_chunk_exported_symbols`). An import of the binding then gets this name.
  /// - The chunk declares the binding, so an import of the binding comes from this chunk.
  /// - The entry module runs before the exports of the chunk read the property. Thus the binding
  ///   has its value when the chunk has run. The module is not wrapped, or it has an ES module
  ///   wrapper that the chunk calls before its exports (`render_wrapped_entry_chunk`). With
  ///   `strictExecutionOrder`, the chunk can call other wrappers in place of this wrapper
  ///   (`consumer_local_namespace_targets`). Then the entry has no binding.
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
    let meta = &self.link_output.metas[module];
    let runs_before_exports = match meta.wrap_kind() {
      WrapKind::None => !order_state.has_order_wrapper(module),
      WrapKind::Esm => {
        meta.wrapper_ref.is_some() && order_state.consumer_local_namespace_targets(module).is_none()
      }
      WrapKind::Cjs => false,
    };
    runs_before_exports
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
