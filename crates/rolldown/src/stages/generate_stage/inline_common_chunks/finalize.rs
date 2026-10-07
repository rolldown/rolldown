use rolldown_common::{
  Chunk, ChunkIdx, ModuleRenderArgs, ModuleRenderOutput, SymbolRef, UsedSymbolRefs,
};
use rolldown_error::{BuildDiagnostic, BuildResult, Severity};
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};

use super::callee_guard::bare_bridge_callee;
use crate::{
  chunk_graph::ChunkGraph,
  module_finalizers::ScopeHoistingFinalizerContext,
  stages::generate_stage::{
    FinalEsmInitMetadata, GenerateStage, Sealed, order_wrap_state::OrderWrapState,
    resolve_file_urls::ResolvedFileUrls,
  },
  type_alias::IndexEcmaAst,
};

/// One record's modules, finalized and printed for one carrier.
#[derive(Debug)]
pub struct CarriedRender {
  pub record: ChunkIdx,
  /// One entry per module of the record, in the record's module order.
  pub modules: Vec<ModuleRenderOutput>,
}

impl GenerateStage<'_> {
  /// Every module wrapper (`init_*`, `require_*`) as a canonical symbol: the modules' own and the
  /// ones the order lowering declares. The `__esm`/`__commonJS` wrappers never read `this`, so a
  /// call of one through a bridge stays bare.
  pub(in crate::stages::generate_stage) fn wrapper_refs(
    &self,
    order_state: &OrderWrapState,
  ) -> FxHashSet<SymbolRef> {
    let symbol_db = &self.link_output.symbol_db;
    self
      .link_output
      .metas
      .iter()
      .filter_map(|meta| meta.wrapper_ref)
      .chain(order_state.wrapper_refs())
      .map(|wrapper_ref| symbol_db.canonical_ref_for(wrapper_ref))
      .collect()
  }

  /// Finalizes and prints every carried record once per carrier. A record's module is never
  /// finalized in place: every rendering works on its own clone of the module's AST, with the
  /// names of the carrier it is printed for and the record as the reader of bridges, so a copy
  /// fits its carrier exactly like the carrier's own modules do. Runs after the ordinary in-place
  /// finalization, which skips the records' modules. See
  /// internal-docs/inline-common-chunks/implementation.md.
  pub(in crate::stages::generate_stage) fn finalize_inline_copies(
    &mut self,
    chunk_graph: &ChunkGraph,
    ast_table: &IndexEcmaAst,
    resolved_file_urls: &ResolvedFileUrls,
    used_symbol_refs: &UsedSymbolRefs,
    order_state: &OrderWrapState,
    final_esm_init_metadata: &Sealed<FinalEsmInitMetadata>,
  ) -> BuildResult<()> {
    let carriers = self.inline_state.carriers();
    if carriers.is_empty() {
      return Ok(());
    }
    // The finalizer's warnings are the same in every copy of a module, so only the first
    // carrier's are kept; the errors, the guards' included, are kept from every copy.
    let mut first_carrier_of = FxHashMap::default();
    for &file in &carriers {
      for &record in self.inline_state.carried_by(file) {
        first_carrier_of.entry(record).or_insert(file);
      }
    }
    let has_enum_inlining = self.link_output.has_enum_inlining;
    let wrapper_refs = self.wrapper_refs(order_state);

    let render = |record: ChunkIdx, chunk: &Chunk, file: ChunkIdx| {
      let mut diagnostics = vec![];
      let callees = self.inline_state.bridge_callees(
        file,
        chunk_graph,
        &wrapper_refs,
        &self.link_output.symbol_db,
      );
      let modules: Vec<ModuleRenderOutput> = chunk_graph.chunk_table[record]
        .modules
        .iter()
        .map(|&module_idx| {
          let module = self.link_output.module_table[module_idx]
            .as_normal()
            .expect("a record holds only normal modules");
          let mut ast =
            ast_table[module_idx].as_ref().expect("should have ast").clone_with_another_arena();
          let ast_scope = &self.link_output.symbol_db[module_idx].as_ref().unwrap().ast_scopes;
          let ctx = ScopeHoistingFinalizerContext {
            idx: module_idx,
            chunk,
            chunk_idx: record,
            file_idx: file,
            chunk_graph,
            symbol_db: &self.link_output.symbol_db,
            linking_info: &self.link_output.metas[module_idx],
            module,
            stmt_infos: &self.link_output.stmt_infos[module_idx],
            index_stmt_infos: &self.link_output.stmt_infos,
            modules: &self.link_output.module_table.modules,
            linking_infos: &self.link_output.metas,
            order_wrap_state: order_state,
            final_esm_init_metadata,
            used_symbol_refs,
            runtime: &self.link_output.runtime,
            options: self.options,
            file_emitter: &self.plugin_driver.file_emitter,
            constant_value_map: &self.link_output.global_constant_symbol_map,
            safely_merge_cjs_ns_map: &self.link_output.safely_merge_cjs_ns_map,
            retained_export_symbols: &self.link_output.retained_export_symbols,
            resolved_paths: self.resolved_paths.as_ref(),
            resolved_file_urls,
            has_enum_inlining,
            inline_state: &self.inline_state,
          };
          let (_, _, module_diagnostics) = ctx.finalize_normal_module(&mut ast, ast_scope);
          diagnostics.extend(module_diagnostics);
          // The selection rules keep every shape the finalizer turns into an import or export
          // declaration as a file; a declaration that reaches a factory would be a syntax error
          // in the carrier, so it fails the build here instead.
          if ast.program().body.iter().any(oxc::ast::ast::Statement::is_module_declaration) {
            diagnostics.push(BuildDiagnostic::unhandleable_error(anyhow::anyhow!(
              "`experimentalInlineCommonChunks`: module `{}` keeps an import or export declaration after finalization, which cannot be printed inside a shared chunk factory; keep it a file with the option's `exclude` until this is fixed.",
              module.stable_id
            )));
          }
          diagnostics.extend(bare_bridge_callee(ast.program(), &callees, &module.stable_id));
          // A factory body sits one level inside the `__share` arrow function.
          module.render(self.options, &ModuleRenderArgs::Ecma { ast: &ast }, 1)
        })
        .collect();
      (modules, diagnostics)
    };

    let rendered: Vec<(ChunkIdx, Vec<CarriedRender>, Vec<BuildDiagnostic>)> = carriers
      .par_iter()
      .map(|&file| {
        let chunk = &chunk_graph.chunk_table[file];
        let mut diagnostics = vec![];
        let renders = self
          .inline_state
          .carried_by(file)
          .iter()
          .map(|&record| {
            let (modules, module_diagnostics) = render(record, chunk, file);
            let first = first_carrier_of.get(&record) == Some(&file);
            diagnostics.extend(
              module_diagnostics
                .into_iter()
                .filter(|diagnostic| first || diagnostic.severity() == Severity::Error),
            );
            CarriedRender { record, modules }
          })
          .collect();
        (file, renders, diagnostics)
      })
      .collect();

    let mut diagnostics = vec![];
    for (file, renders, file_diagnostics) in rendered {
      diagnostics.extend(file_diagnostics);
      self.inline_renders.insert(file, renders);
    }
    let has_error = diagnostics.iter().any(|diagnostic| diagnostic.severity() == Severity::Error);
    self.link_output.diagnostics.extend(diagnostics);
    if has_error {
      let errors = self.link_output.diagnostics.extract_errors();
      Err(errors)?;
    }
    Ok(())
  }
}
