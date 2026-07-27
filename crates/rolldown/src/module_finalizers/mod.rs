use bitflags::bitflags;
use oxc::allocator::GetAllocator;
use oxc::ast::ast::{BindingIdentifier, CallExpression, IdentifierName, ObjectPropertyKind};
use oxc::ast::builder::{AstBuilder, GetAstBuilder, NONE};
use oxc::semantic::{NodeId, ReferenceId, ScopeFlags, SymbolId};
use oxc::{
  allocator::{self, Allocator, CloneIn, Dummy, IntoIn, ReplaceWith, TakeIn},
  ast::ast::{
    self, ClassElement, Expression, IdentifierReference, ImportExpression, NumberBase, Statement,
    VariableDeclarationKind,
  },
  span::{GetSpan, GetSpanMut, SPAN, Span},
};
use rolldown_common::{
  AstScopes, Chunk, ChunkIdx, ConcatenateWrappedModuleKind, ExportsKind, ImportRecordIdx,
  ImportRecordMeta, InlineConstMode, MemberExprRefResolution, Module, ModuleIdx, ModuleType,
  NamespaceAlias, NormalModule, OutputExports, OutputFormat, Platform,
  RenderedConcatenatedModuleParts, Specifier, SymbolRef, WrapKind,
};
use rolldown_ecmascript::ToSourceString;
use rolldown_ecmascript_utils::{
  BindingIdentifierFactoryExt as _, BindingPatternExt, CallExpressionExt,
  CallExpressionFactoryExt as _, ClassElementFactoryExt as _, ExpressionExt,
  ExpressionFactoryExt as _, IdentifierNameFactoryExt as _, StatementExt, StatementFactoryExt as _,
  parse_injected_expression,
};
use rolldown_error::EmptyImportMetaKind;
use std::borrow::Cow;

mod finalizer_context;
mod impl_visit_mut;
use finalizer_context::ModuleWrapperMode;
pub use finalizer_context::ScopeHoistingFinalizerContext;
use oxc_str::{CompactStr, Ident};
use rolldown_std_utils::absolutize_path_buf;
use rolldown_utils::ecmascript::is_validate_identifier_name;
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
use rustc_hash::{FxHashMap, FxHashSet};
use sugar_path::SugarPath;

use crate::esm_init_obligations::{
  ObligationPurpose, WrappedEsmInitTargetContext,
  collect_wrapped_esm_init_targets_for_import_record, record_is_init_obligation,
};
use crate::utils;
use crate::utils::external_import_interop::import_record_needs_interop;

mod hmr;
mod rename;

/// Helper enum for `try_rewrite_cjs_member_expr_assignment_target` to handle both static and computed member properties.
enum CjsMemberProperty<'a, 'ast> {
  Static(&'a str),
  Computed(&'a ast::Expression<'ast>),
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct TraverseState: u8 {
        const TopLevel = 1;
        /// - `if (test) {} else {}`
        /// - test ? a : b
        /// - test1 || test2
        /// - test1 && test2
        /// - test1 ?? test2
        const SmartInlineConst = 1 << 1;
        const IsRootLevel = 1 << 2;
    }
}

bitflags! {
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub struct FinalizedExprProcessHint: u8 {
      const FromCjsWrapKindEntry = 1;
  }
}

/// Represents different ways to identify a name for `keep_names` functionality.
#[derive(Clone, Debug, Copy)]
pub enum KeepNameId<'a> {
  /// A symbol ID from the AST's symbol table.
  SymbolId(SymbolId),
  /// A reference ID from the AST's reference table.
  ReferenceId(ReferenceId),
  /// A string name used directly (e.g., for "default" exports).
  CompactStr(&'a CompactStr),
}

/// Finalizer for emitting output code with scope hoisting.
pub struct ScopeHoistingFinalizer<'me, 'ast: 'me> {
  pub ctx: ScopeHoistingFinalizerContext<'me>,
  pub scope: &'me AstScopes,
  pub alloc: &'ast Allocator,
  pub ast_builder: AstBuilder<'ast>,
  /// Wrapped-ESM importees whose `init_*()` call was already emitted while finalizing this
  /// module, so the various init-emission paths don't emit duplicates.
  pub generated_init_esm_importee_ids: FxHashSet<ModuleIdx>,
  pub scope_stack: Vec<ScopeFlags>,
  pub state: TraverseState,
  pub top_level_var_bindings: FxIndexSet<Ident<'ast>>,
  pub cur_stmt_index: usize,
  pub keep_name_statement_to_insert: Vec<(usize, CompactStr, CompactStr)>,
  pub needs_hosted_top_level_binding: bool,
  pub module_namespace_included: bool,
  pub transferred_import_record: FxIndexMap<ImportRecordIdx, String>,
  pub rendered_concatenated_wrapped_module_parts: RenderedConcatenatedModuleParts,
  pub json_module_inlined_prop: Option<Box<FxHashMap<SymbolId, ast::Expression<'ast>>>>,
  /// Reference ids of `import.meta.ROLLDOWN_FILE_URL_*` accesses that no emitted file matches.
  ///
  /// Deduplicated by reference id, because `try_rewrite_member_expr` runs *twice* on every member
  /// expression it fails to rewrite: `visit_expression` calls it, and on `None` the arm falls
  /// through to `walk_expression`, which re-dispatches the very same node into
  /// `visit_member_expression`, where the identical lookup is attempted again. Pushing
  /// `BuildDiagnostic`s straight into a `Vec` here would report each unknown reference id twice.
  ///
  /// Deduplicating also makes a reference id that is accessed several times report once, matching
  /// Rollup, which throws on the first access it renders.
  pub missing_file_reference_ids: FxIndexMap<CompactStr, Span>,
  /// Code returned by `resolveFileUrl` that failed to parse, as `(plugin name, message)`.
  /// Collected here because the finalizer is sync and rayon-parallel; `finalize_modules`
  /// turns these into plugin-attributed build errors once the parallel pass is done.
  pub resolve_file_url_errors: Vec<(Cow<'static, str>, String)>,
  /// Spans of the `import.meta` accesses this finalizer could not rewrite away, and so replaced
  /// with an empty object.
  ///
  /// Keyed by span, because an `import.meta.<prop>` that fails to rewrite is reached twice: once
  /// as the member expression (which knows the property) and once as the bare `import.meta`
  /// object it walks into. The first insert wins, so the property-aware one is kept.
  pub surviving_import_meta_spans: FxIndexMap<Span, EmptyImportMetaKind>,
}

impl<'me, 'ast> ScopeHoistingFinalizer<'me, 'ast> {
  pub fn is_global_identifier_reference(&self, id_ref: &IdentifierReference) -> bool {
    let Some(reference_id) = id_ref.reference_id.get() else {
      // Some `IdentifierReference`s constructed by bundler don't have a `ReferenceId`. They might be global variables.
      // But we don't care about them in this method. This method is only used to check if a `IdentifierReference` from user code is a global variable.
      return false;
    };
    self.scope.is_unresolved(reference_id)
  }

  pub fn canonical_name_for(&self, symbol: SymbolRef) -> &'me str {
    self.ctx.symbol_db.canonical_name_for_or_original(symbol, &self.ctx.chunk.canonical_names)
  }

  pub fn canonical_name_for_runtime(&self, name: &str) -> &'me str {
    let sym_ref = self.ctx.runtime.resolve_symbol(name);
    self.canonical_name_for(sym_ref)
  }

  pub fn canonical_ref_for_runtime(&self, name: &str) -> SymbolRef {
    self.ctx.runtime.resolve_symbol(name)
  }

  pub fn finalized_expr_for_runtime_symbol(&self, name: &str) -> ast::Expression<'ast> {
    let (expr, _) =
      self.finalized_expr_for_symbol_ref(self.ctx.runtime.resolve_symbol(name), false, false);
    expr
  }

  /// Build the `init_x()` call expression for a wrapped (`WrapKind::Esm`) importee.
  ///
  /// - `mark_pure_if_noop`: annotate the call `/* @__PURE__ */` when the importee's init is a
  ///   no-op (empty wrapped-ESM closure), letting the default `dce-only` minify drop it.
  /// - `await_if_tla`: wrap the call in `await` when the importee is TLA-tainted.
  fn wrapped_esm_init_call_expr(
    &self,
    importee_idx: ModuleIdx,
    call_span: Span,
    mark_pure_if_noop: bool,
    await_if_tla: bool,
  ) -> ast::Expression<'ast> {
    let importee_linking_info = &self.ctx.linking_infos[importee_idx];
    let target = self
      .ctx
      .order_wrap_state
      .esm_init_target(importee_idx, importee_linking_info)
      .expect("wrapped ESM init call should have an init target");
    // `init_foo`
    let (wrapper_ref_expr, _) =
      self.finalized_expr_for_symbol_ref(target.wrapper_ref, false, false);
    // `init_foo()`
    let init_call = ast::Expression::new_call_expression_with_pure(
      call_span,
      wrapper_ref_expr,
      NONE,
      oxc::allocator::Vec::new_in(&self.ast_builder),
      false,
      mark_pure_if_noop && self.ctx.final_esm_init_metadata.init_is_noop(importee_idx),
      &self.ast_builder,
    );
    if await_if_tla && target.tla_tainted {
      // `await init_foo()`
      ast::Expression::AwaitExpression(ast::AwaitExpression::boxed(
        SPAN,
        init_call,
        &self.ast_builder,
      ))
    } else {
      init_call
    }
  }

  /// Whether a wrapper symbol can be referenced from the chunk being finalized: it is either
  /// declared in this chunk or registered as a cross-chunk import — exactly the symbols
  /// deconfliction assigned a canonical name for. Finalizers run after cross-chunk imports are
  /// computed, so a synthesized call to any other wrapper would render as a bare identifier
  /// with no backing import (`ReferenceError` at runtime).
  ///
  /// Skipping the init call in that case is sound: cross-chunk wrapper imports are registered
  /// whenever a chunk depends on a symbol *owned by* the wrapped module
  /// (`add_depended_symbol_with_wrapped_esm_init`), so an unreachable wrapper means every
  /// access flows through the forwarding barrel's namespace object instead. That namespace
  /// dependency imports the barrel's chunk, which executes first and performs the init via the
  /// barrel's own lowered statements.
  fn wrapper_is_reachable_in_chunk(&self, wrapper_ref: SymbolRef) -> bool {
    let canonical_ref = self.ctx.symbol_db.canonical_ref_for(wrapper_ref);
    self.ctx.chunk.canonical_names.contains_key(&canonical_ref)
  }

  fn wrapped_esm_init_stmt_for_import_record(
    &mut self,
    rec_idx: ImportRecordIdx,
  ) -> Option<Statement<'ast>> {
    // A fresh `AstBuilder` (a free wrapper over the arena reference) lets the `&mut self` iterator
    // below stay borrowed while we still construct nodes through `ast_builder`. That decouples node
    // construction from the borrow without the throwaway heap `Vec` the previous `.collect()`
    // needed: the common 0/1 cases now allocate nothing, and only the rare sequence case
    // allocates — straight in the arena.
    let ast_builder = AstBuilder::new(self.alloc);
    let targets = collect_wrapped_esm_init_targets_for_import_record(
      &WrappedEsmInitTargetContext {
        importer: self.ctx.module,
        importer_meta: self.ctx.linking_info,
        modules: self.ctx.modules,
        metas: self.ctx.linking_infos,
        stmt_infos: self.ctx.index_stmt_infos,
        symbol_db: self.ctx.symbol_db,
        constant_value_map: self.ctx.constant_value_map,
        inline_const_mode: self.ctx.options.optimization.inline_const.map(|config| config.mode),
        order_wrap_state: self.ctx.order_wrap_state,
        strict_execution_order: self.ctx.options.is_strict_execution_order_enabled(),
      },
      rec_idx,
      |symbol_ref| self.ctx.used_symbol_refs.contains(&symbol_ref),
      |wrapper_ref| self.wrapper_is_reachable_in_chunk(wrapper_ref),
      |forwarding_module_idx| {
        self.ctx.chunk_graph.module_to_chunk[forwarding_module_idx] == Some(self.ctx.chunk_idx)
      },
    );
    let mut init_exprs = targets.into_iter().filter_map(|module_idx| {
      if !self.generated_init_esm_importee_ids.insert(module_idx) {
        return None;
      }
      // The shared target resolver only collects modules with a reachable `wrapper_ref`.
      Some(self.wrapped_esm_init_call_expr(module_idx, SPAN, true, true))
    });
    // Drive the iterator by hand. Every branch consumes it to exhaustion, so each owner's
    // `generated_init_esm_importee_ids` insert still runs (the global dedup must observe all of
    // them) regardless of how many statements we end up emitting.
    let first = init_exprs.next()?;
    let Some(second) = init_exprs.next() else {
      return Some(ast::Statement::new_expression_statement(SPAN, first, &ast_builder));
    };
    let mut exprs = oxc::allocator::Vec::with_capacity_in(2, &ast_builder);
    exprs.push(first);
    exprs.push(second);
    exprs.extend(init_exprs);
    Some(ast::Statement::new_expression_statement(
      SPAN,
      ast::Expression::new_sequence_expression(SPAN, exprs, &ast_builder),
      &ast_builder,
    ))
  }

  /// If return true the import stmt should be removed,
  /// or transform the import stmt to target form.
  fn transform_or_remove_import_export_stmt(
    &mut self,
    stmt: &mut Statement<'ast>,
    rec_idx: ImportRecordIdx,
  ) -> bool {
    let rec = &self.ctx.module.import_records[rec_idx];
    let Some(resolved_module_idx) = rec.resolved_module else { return true };
    let Module::Normal(importee) = &self.ctx.modules[resolved_module_idx] else {
      return true;
    };
    let importee_linking_info = &self.ctx.linking_infos[importee.idx];
    match importee_linking_info.wrap_kind() {
      WrapKind::None => {
        // Emission consumes the shared obligation gate; this transform only runs for *included*
        // statements (excluded ones take `remove_unused_top_level_stmt`'s early branch).
        if record_is_init_obligation(
          ObligationPurpose::Emit,
          self.ctx.order_wrap_state,
          self.ctx.idx,
          rec,
          rec_idx,
          true,
        ) && let Some(init_stmt) = self.wrapped_esm_init_stmt_for_import_record(rec_idx)
        {
          *stmt = init_stmt;
          return false;
        }
        // Remove this statement by ignoring it
      }
      WrapKind::Cjs => {
        // Check if this CJS module's namespace can be merged with other imports
        let merge_info = self.ctx.safely_merge_cjs_ns_map.get(&resolved_module_idx);

        // Consider user reference a module use relative path e.g.
        // ```js
        // import React from './node_modules/react/index.js';
        // ```
        if merge_info.is_some() {
          let chunk_idx = self.ctx.chunk_idx;
          if let Some(symbol_ref_to_be_merged) =
            self.ctx.chunk_graph.finalized_cjs_ns_map_idx_vec[chunk_idx].get(&rec.namespace_ref)
          {
            if symbol_ref_to_be_merged != &rec.namespace_ref {
              return true;
            }
          }
        }

        // Replace the statement with something like `var import_foo = __toESM(require_foo())`
        // or `var import_foo = require_foo()` if only named imports are used

        // `require_foo`
        let (importee_wrapper_ref_name, hint) = self.finalized_expr_for_symbol_ref(
          importee_linking_info.wrapper_ref.unwrap(),
          false,
          false,
        );

        let require_call = if hint.contains(FinalizedExprProcessHint::FromCjsWrapKindEntry) {
          importee_wrapper_ref_name
        } else {
          ast::Expression::new_call_expression(
            SPAN,
            importee_wrapper_ref_name,
            NONE,
            oxc::allocator::Vec::new_in(&self.ast_builder),
            false,
            &self.ast_builder,
          )
        };

        // Check if we need __toESM or can use require_foo() directly
        let needs_toesm = if let Some(info) = merge_info {
          info.needs_interop
        } else {
          import_record_needs_interop(self.ctx.module, rec_idx)
        };
        let init_expr = if needs_toesm {
          // `__toESM`
          let to_esm_fn_name = self.finalized_expr_for_runtime_symbol("__toESM");
          Expression::new_to_esm_wrapper(
            to_esm_fn_name,
            require_call,
            self.ctx.module.should_consider_node_esm_spec_for_static_import(),
            &self.ast_builder,
          )
        } else {
          require_call
        };

        // `import_foo`
        let binding_name_for_wrapper_call_ret = self.canonical_name_for(rec.namespace_ref);
        *stmt =
          Statement::new_var_decl(binding_name_for_wrapper_call_ret, init_expr, &self.ast_builder);

        if self.transferred_import_record.contains_key(&rec_idx) {
          self.transferred_import_record.insert(rec_idx, stmt.to_source_string());
          return true;
        }
        return false;
      }
      // Replace the import statement with `init_foo()` if `ImportDeclaration` is not a plain import
      // or the importee have side effects.
      WrapKind::Esm => {
        if matches!(
          importee_linking_info.concatenated_wrapped_module_kind,
          ConcatenateWrappedModuleKind::Inner
        ) || self.generated_init_esm_importee_ids.contains(&importee.idx)
        {
          return true;
        }
        self.generated_init_esm_importee_ids.insert(importee.idx);
        // `init_foo()` / `await init_foo()`
        let init_expr = self.wrapped_esm_init_call_expr(importee.idx, stmt.span(), false, true);
        *stmt = ast::Statement::new_expression_statement(SPAN, init_expr, &self.ast_builder);

        if self.transferred_import_record.contains_key(&rec_idx) {
          self.transferred_import_record.insert(rec_idx, stmt.to_source_string());
          return true;
        }
        return false;
      }
    }
    true
  }

  /// `optimize_namespace_alias_transform` is a flag to determine whether optimize interop code with commonjs
  /// e.g.
  /// We could try to rewrite `import_cjs.default.exported` into `import_cjs.exported`
  fn finalized_expr_for_symbol_ref(
    &self,
    symbol_ref: SymbolRef,
    preserve_this_semantic_if_needed: bool,
    optimize_namespace_alias_transform: bool,
  ) -> (ast::Expression<'ast>, FinalizedExprProcessHint) {
    if !symbol_ref.is_declared_in_root_scope(self.ctx.symbol_db) {
      // No fancy things on none root scope symbols
      return (
        Expression::new_id_ref_expr(SPAN, self.canonical_name_for(symbol_ref), &self.ast_builder),
        FinalizedExprProcessHint::empty(),
      );
    }

    let mut canonical_ref = self.ctx.symbol_db.canonical_ref_for(symbol_ref);
    let mut canonical_symbol = self.ctx.symbol_db.get(canonical_ref);
    let namespace_alias = canonical_symbol.namespace_alias.as_ref();
    if let Some(ns_alias) = namespace_alias {
      if let Some(expr) = self.try_inline_constant_from_namespace_alias(symbol_ref, ns_alias) {
        return expr;
      }
      canonical_ref = ns_alias.namespace_ref;
      canonical_symbol = self.ctx.symbol_db.get(canonical_ref);
    }
    if let Some(meta) = self.ctx.constant_value_map.get(&canonical_ref) {
      if !self.ctx.options.optimization.is_inline_const_smart_mode()
        || (self.state.contains(TraverseState::SmartInlineConst) || meta.safe_to_inline)
      {
        return (
          meta.value.to_expression(self.ast_builder.builder()),
          FinalizedExprProcessHint::empty(),
        );
      }
    }
    let mut hint = FinalizedExprProcessHint::empty();
    let mut expr = if self.ctx.modules[canonical_ref.owner].is_external() {
      // For mixed-mode externals, ESM importers use the node-mode binding name
      if self.ctx.module.should_consider_node_esm_spec_for_static_import() {
        if let Some(node_mode_name) = self.ctx.chunk.node_mode_external_ns_names.get(&canonical_ref)
        {
          Expression::new_id_ref_expr(SPAN, node_mode_name.as_str(), &self.ast_builder)
        } else {
          Expression::new_id_ref_expr(
            SPAN,
            self.canonical_name_for(canonical_ref),
            &self.ast_builder,
          )
        }
      } else {
        Expression::new_id_ref_expr(SPAN, self.canonical_name_for(canonical_ref), &self.ast_builder)
      }
    } else {
      match self.ctx.options.format {
        rolldown_common::OutputFormat::Cjs => {
          let chunk_idx_of_canonical_symbol = canonical_symbol.chunk_idx.unwrap_or_else(|| {
            // Scoped symbols don't get assigned a `ChunkIdx`. There are skipped for performance reason, because they are surely
            // belong to the chunk they are declared in and won't link to other chunks.
            let symbol_name = canonical_ref.name(self.ctx.symbol_db);
            panic!("{canonical_ref:?} {symbol_name:?} is not in any chunk, which is unexpected");
          });
          let cur_chunk_idx = self.ctx.chunk_graph.module_to_chunk[self.ctx.idx]
            .expect("This module should be in a chunk");
          let is_symbol_in_other_chunk = cur_chunk_idx != chunk_idx_of_canonical_symbol;
          if is_symbol_in_other_chunk {
            let (expr, extra_hint) = self.finalized_expr_for_cross_chunk_symbol(
              cur_chunk_idx,
              chunk_idx_of_canonical_symbol,
              canonical_ref,
            );
            hint.insert(extra_hint);
            expr
          } else {
            Expression::new_id_ref_expr(
              SPAN,
              self.canonical_name_for(canonical_ref),
              &self.ast_builder,
            )
          }
        }
        _ => Expression::new_id_ref_expr(
          SPAN,
          self.canonical_name_for(canonical_ref),
          &self.ast_builder,
        ),
      }
    };

    if let Some(ns_alias) = namespace_alias {
      if !optimize_namespace_alias_transform {
        expr = ast::Expression::StaticMemberExpression(ast::StaticMemberExpression::boxed(
          SPAN,
          expr,
          IdentifierName::new_id_name(SPAN, &ns_alias.property_name, &self.ast_builder),
          false,
          &self.ast_builder,
        ));
      }

      if preserve_this_semantic_if_needed {
        expr = Expression::new_seq_in_parens(
          ast::Expression::new_numeric_literal(
            SPAN,
            0.0,
            Some("0".into()),
            NumberBase::Decimal,
            &self.ast_builder,
          ),
          expr,
          &self.ast_builder,
        );
      }
    }

    (expr, hint)
  }

  /// Generates the expression for accessing a symbol from another chunk in CJS output.
  ///
  /// In CJS output, cross-chunk symbol access uses `require()` bindings:
  /// - `import { foo } from 'foo'; console.log(foo);` becomes `console.log(require_foo.foo);`
  ///
  /// Returns the expression and any additional hints for processing.
  ///
  /// ## Cross-Chunk Symbol Resolution Matrix
  ///
  /// See `crates/rolldown/tests/rolldown/topics/exports/README.md` for the full test matrix.
  ///
  /// | # | Chunk Type | WrapKind | OutputExports | Export Name | Result |
  /// |---|------------|----------|---------------|-------------|--------|
  /// | 1 | Entry | Cjs | Default | default | `require_binding` |
  /// | 2 | Entry | Cjs | Named | default | `require_binding.default` |
  /// | 3 | Entry | Cjs | Named | named | `require_binding.default` (access wrapped module) |
  /// | 4 | Entry | Esm | Default | default | N/A (invalid: ESM wrap adds extra exports) |
  /// | 5 | Entry | Esm | Named | default | `require_binding.default` |
  /// | 6 | Entry | Esm | Named | named | `require_binding.exportName` |
  /// | 7 | Entry | None | Default | default | `require_binding` |
  /// | 8 | Entry | None | Named | default | `require_binding.default` |
  /// | 9 | Entry | None | Named | named | `require_binding.exportName` |
  /// | 10-15 | Common | * | Named | * | `require_binding.exportName` |
  fn finalized_expr_for_cross_chunk_symbol(
    &self,
    cur_chunk_idx: ChunkIdx,
    target_chunk_idx: ChunkIdx,
    canonical_ref: SymbolRef,
  ) -> (ast::Expression<'ast>, FinalizedExprProcessHint) {
    let require_binding = &self.ctx.chunk_graph.chunk_table[cur_chunk_idx]
      .require_binding_names_for_other_chunks[&target_chunk_idx];

    let chunk = &self.ctx.chunk_graph.chunk_table[target_chunk_idx];

    // Determine chunk type: Entry (symbol owner is entry module) vs Common
    let is_entry_chunk_for_symbol = chunk.entry_module_idx() == Some(canonical_ref.owner);

    // Get wrap kind for entry chunks
    let wrap_kind = self.ctx.linking_infos[canonical_ref.owner].wrap_kind();

    // For CJS-wrapped entry chunks, we access the wrapped module directly via `require_binding`
    // or `require_binding.default` depending on OutputExports.
    // See https://github.com/rolldown/rolldown/blob/16349a4efa8d841d3b5ca8e2ebabf24a1e1c406f/crates/rolldown/src/utils/chunk/render_chunk_exports.rs?plain=1#L159-L182
    if is_entry_chunk_for_symbol && matches!(wrap_kind, WrapKind::Cjs) {
      // Cases #1-3: Entry + Cjs
      let expr = match chunk.output_exports {
        // Case #1: Entry + Cjs + Default + default → require_binding
        OutputExports::Default => {
          Expression::new_id_ref_expr(SPAN, require_binding, &self.ast_builder)
        }
        // Cases #2-3: Entry + Cjs + Named → require_binding.default (the wrapped module)
        _ => Expression::new_member_access_expr(require_binding, "default", &self.ast_builder),
      };
      return (expr, FinalizedExprProcessHint::FromCjsWrapKindEntry);
    }

    // All other cases: Entry with Esm/None WrapKind, or Common chunks
    // These use the exported name from exports_to_other_chunks
    let exported_name = &self.ctx.chunk_graph.chunk_table[target_chunk_idx].exports_to_other_chunks
      [&canonical_ref][0];
    let is_default_export = exported_name.as_str() == "default";

    // When OutputExports::Default, the entire module.exports IS the default value directly.
    // See https://github.com/rolldown/rolldown/issues/7833
    let expr = match (&chunk.output_exports, is_default_export) {
      // Case #7: Entry + None + Default + default → require_binding
      (OutputExports::Default, true) => {
        Expression::new_id_ref_expr(SPAN, require_binding, &self.ast_builder)
      }
      // Cases #5, #8, #10, #12, #14: Named + default → require_binding.default
      // Cases #6, #9, #11, #13, #15: Named + named → require_binding.exportName
      _ => Expression::new_member_access_expr(require_binding, exported_name, &self.ast_builder),
    };

    (expr, FinalizedExprProcessHint::empty())
  }

  fn try_inline_constant_from_namespace_alias(
    &self,
    original_ref: SymbolRef,
    namespace_alias: &NamespaceAlias,
  ) -> Option<(ast::Expression<'ast>, FinalizedExprProcessHint)> {
    let inline_options = self.ctx.options.optimization.inline_const?;
    let canonical_ref = self.ctx.symbol_db.canonical_ref_for(original_ref);
    let named_import = self.ctx.module.named_imports.get(&canonical_ref)?;

    if !matches!(&named_import.imported, Specifier::Literal(lit) if lit != "default") {
      return None;
    }

    let import_record = &self.ctx.module.import_records[named_import.record_idx];
    let importee = import_record
      .resolved_module
      .and_then(|module_idx| self.ctx.modules[module_idx].as_normal())?;

    let resolved_export =
      self.ctx.linking_infos[importee.idx].resolved_exports.get(&namespace_alias.property_name)?;

    // Don't inline when there are conflicting CJS sources — the value could differ per branch
    // TODO(hana): Optimize this with conditional inlining
    if resolved_export.cjs_conflicting_symbol_refs.is_some() {
      return None;
    }

    let export_symbol = resolved_export.symbol_ref;
    let canonical_export_ref = self.ctx.symbol_db.canonical_ref_for(export_symbol);

    let constant_meta = self.ctx.constant_value_map.get(&canonical_export_ref)?;

    if matches!(inline_options.mode, InlineConstMode::Smart)
      && (!self.state.contains(TraverseState::SmartInlineConst) || !constant_meta.safe_to_inline)
    {
      return None;
    }
    Some((
      constant_meta.value.to_expression(self.ast_builder.builder()),
      FinalizedExprProcessHint::empty(),
    ))
  }

  /// Try to inline an enum member access from an expression. Handles:
  /// - `Direction.Up` (static member with identifier object)
  /// - `ns.Direction.Up` (chained static member via namespace import)
  /// - `Direction["Up"]` (computed member with string literal key)
  /// - `Direction?.Up` / `Direction?.["Up"]` (optional chain — enum bindings
  ///   are always defined, so `?.` is equivalent to `.`)
  fn try_inline_enum_access(&self, expr: &ast::Expression<'_>) -> Option<ast::Expression<'ast>> {
    let member = expr.get_member_expr()?;
    let (object, property_name) = match member {
      ast::MemberExpression::StaticMemberExpression(m) => (&m.object, m.property.name.as_str()),
      ast::MemberExpression::ComputedMemberExpression(m) => {
        let ast::Expression::StringLiteral(prop) = &m.expression else { return None };
        (&m.object, prop.value.as_str())
      }
      ast::MemberExpression::PrivateFieldExpression(_) => return None,
    };
    if let ast::Expression::Identifier(ident) = object {
      return self.try_inline_enum_member(ident, property_name);
    }
    // `ns.Direction.Up` — namespace-import resolution. Only for direct (non-chain)
    // static access; the chained-namespace optional case isn't handled.
    if !matches!(expr, ast::Expression::ChainExpression(_))
      && let ast::MemberExpression::StaticMemberExpression(sm) = member
    {
      return self.try_inline_chained_enum_member(sm);
    }
    None
  }

  /// Try to inline an enum member access like `Direction.Up` → `0`.
  /// Resolves the identifier to its canonical symbol, then looks up the enum member
  /// value in the owning module's `enum_member_value_map`.
  fn try_inline_enum_member(
    &self,
    ident: &ast::IdentifierReference<'_>,
    property_name: &str,
  ) -> Option<ast::Expression<'ast>> {
    let ref_id = ident.reference_id.get()?;
    let symbol_id = self.scope.scoping().get_reference(ref_id).symbol_id()?;
    let symbol_ref: SymbolRef = (self.ctx.idx, symbol_id).into();
    self.try_inline_enum_member_by_ref(symbol_ref, property_name)
  }

  /// Try to inline a chained enum member access like `ns.c.x` → `"c"`.
  /// `ns` is a namespace import (`import * as ns`), `c` is a named export (enum), `x` is the member.
  ///
  /// This is separate from `try_rewrite_member_expr` because `resolved_member_expr_refs` resolves
  /// `ns.c` → identifier `c` with `.x` as a remaining prop. The post-rewrite enum check only
  /// matches `Identifier.property` patterns, so by the time `new_member_expr_or_ident_ref` rebuilds
  /// `c.x`, the inlining window has passed. This method resolves all three levels in one pass.
  fn try_inline_chained_enum_member(
    &self,
    outer_expr: &ast::StaticMemberExpression<'_>,
  ) -> Option<ast::Expression<'ast>> {
    // The object must be a StaticMemberExpression (e.g., `ns.c`)
    let ast::Expression::StaticMemberExpression(inner_expr) = &outer_expr.object else {
      return None;
    };
    // The inner object must be an identifier (e.g., `ns`)
    let ast::Expression::Identifier(ns_ident) = &inner_expr.object else {
      return None;
    };

    // Resolve `ns` to its symbol
    let ref_id = ns_ident.reference_id.get()?;
    let symbol_id = self.scope.scoping().get_reference(ref_id).symbol_id()?;
    let symbol_ref: SymbolRef = (self.ctx.idx, symbol_id).into();
    let canonical_ref = self.ctx.symbol_db.canonical_ref_for(symbol_ref);

    // Find which module this namespace belongs to.
    // For `import * as ns from './enums'`, canonical_ref.owner is the importee module.
    let importee = self.ctx.modules[canonical_ref.owner].as_normal()?;

    // Find the exported symbol for the inner property name (e.g., `c`)
    let resolved_export = self.ctx.linking_infos[importee.idx]
      .resolved_exports
      .get(inner_expr.property.name.as_str())?;

    // Don't inline when there are conflicting CJS sources — the value could differ per branch
    if resolved_export.cjs_conflicting_symbol_refs.is_some() {
      return None;
    }

    let canonical_export = self.ctx.symbol_db.canonical_ref_for(resolved_export.symbol_ref);

    // Now try to inline the outer property (e.g., `x`) as an enum member
    self.try_inline_enum_member_by_ref(canonical_export, outer_expr.property.name.as_str())
  }

  fn try_inline_enum_member_by_ref(
    &self,
    symbol_ref: SymbolRef,
    property_name: &str,
  ) -> Option<ast::Expression<'ast>> {
    let canonical_ref = self.ctx.symbol_db.canonical_ref_for(symbol_ref);
    let module = self.ctx.modules[canonical_ref.owner].as_normal()?;
    let symbol_name = canonical_ref.name(self.ctx.symbol_db);
    let member_map = module.ecma_view.enum_member_value_map.get(symbol_name)?;
    let meta = member_map.get(property_name)?;
    Some(meta.value.to_expression(self.ast_builder.builder()))
  }

  fn var_declaration_to_expr_seq_and_bindings(
    &self,
    decl: &mut ast::VariableDeclaration<'ast>,
    traverse_state: TraverseState,
  ) -> Option<(Expression<'ast>, Vec<Ident<'ast>>)> {
    let should_hoist = (decl.kind.is_var() && traverse_state.contains(TraverseState::TopLevel))
      || (decl.kind.is_lexical() && traverse_state.contains(TraverseState::IsRootLevel));
    if !should_hoist {
      return None;
    }
    let mut ret = vec![];
    let exprs = decl.declarations.take_in(&self.alloc).into_iter().filter_map(|var_decl| {
      ret.extend(var_decl.id.get_binding_identifiers().iter().map(|item| item.name));
      // Turn `var ... = ...` to `... = ...`
      let init_expr = var_decl.init?;
      let left = var_decl.id.into_assignment_target(&self.ast_builder);
      Some(ast::Expression::AssignmentExpression(ast::AssignmentExpression::boxed(
        SPAN,
        ast::AssignmentOperator::Assign,
        left,
        init_expr,
        &self.ast_builder,
      )))
    });
    Some((
      ast::Expression::new_sequence_expression(
        SPAN,
        oxc::allocator::Vec::from_iter_in(exprs, &self.ast_builder),
        &self.ast_builder,
      ),
      ret,
    ))
  }

  fn generate_declaration_of_module_namespace_object(&self) -> Vec<ast::Statement<'ast>> {
    if !self.module_namespace_included {
      return vec![];
    }

    let binding_name_for_namespace_object_ref =
      self.canonical_name_for(self.ctx.module.namespace_object_ref);

    // construct `{ prop_name: () => returned, ... }`
    let mut arg_obj_expr = ast::ObjectExpression::dummy(self.alloc);

    // Even if the module namespace is included, some exports may not be used due to `optimize_facade_dynamic_entry_chunks`
    // https://github.com/rolldown/rolldown/blob/d6d65f9080e427cd9feef56eb7a110fbcf6c1414/crates/rolldown/src/stages/generate_stage/chunk_optimizer.rs#L347-L354
    arg_obj_expr.properties.extend(self.ctx.linking_info.canonical_exports(false).filter_map(
      |(export, resolved_export)| {
        // Even if the symbol is not marked as used (generated inside module),
        // it should be included in the namespace export.
        let is_inlinable_constant = self
          .ctx
          .constant_value_map
          .get(&self.ctx.symbol_db.canonical_ref_for(resolved_export.symbol_ref))
          .is_some_and(|meta| !meta.commonjs_export);
        if !self.ctx.retained_export_symbols.contains(&resolved_export.symbol_ref)
          && !is_inlinable_constant
        {
          return None;
        }
        // prop_name: () => returned
        let prop_name = export;
        let (returned, _) =
          self.finalized_expr_for_symbol_ref(resolved_export.symbol_ref, false, false);
        // `__proto__` has special semantics in object literals - it sets the prototype
        // instead of creating a property. Use computed property syntax for it.
        let key = if is_validate_identifier_name(prop_name) && prop_name != "__proto__" {
          ast::PropertyKey::StaticIdentifier(
            IdentifierName::new_id_name(SPAN, prop_name, &self.ast_builder).into_in(self.alloc),
          )
        } else {
          ast::PropertyKey::StringLiteral(ast::StringLiteral::boxed(
            SPAN,
            oxc::ast::ast::Str::from_str_in(prop_name, &self.ast_builder),
            None,
            &self.ast_builder,
          ))
        };
        Some(ast::ObjectPropertyKind::ObjectProperty(ast::ObjectProperty::boxed(
          SPAN,
          ast::PropertyKind::Init,
          key,
          Expression::new_arrow_returning(returned, &self.ast_builder),
          false,
          false,
          prop_name == "__proto__",
          &self.ast_builder,
        )))
      },
    ));

    // if there is no export, we should generate `var ns = {}` instead of `var ns = __exportAll({})`
    // else construct `__exportAll({ prop_name: () => returned, ... })`
    let module_namespace_rhs =
      if arg_obj_expr.properties.is_empty() && !self.ctx.options.generated_code.symbols {
        Expression::ObjectExpression(oxc::allocator::Box::new_in(arg_obj_expr, &self.ast_builder))
      } else {
        let obj_expr = ast::Argument::ObjectExpression(arg_obj_expr.into_in(self.alloc));
        let args = if self.ctx.options.generated_code.symbols {
          oxc::allocator::Vec::from_iter_in([obj_expr], &self.ast_builder)
        } else {
          oxc::allocator::Vec::from_iter_in(
            [
              obj_expr,
              ast::Argument::NumericLiteral(ast::NumericLiteral::boxed(
                SPAN,
                1.0,
                None,
                NumberBase::Decimal,
                &self.ast_builder,
              )),
            ],
            &self.ast_builder,
          )
        };
        ast::Expression::new_call_expression_with_pure(
          SPAN,
          self.finalized_expr_for_runtime_symbol("__exportAll"),
          NONE,
          args,
          false,
          true,
          &self.ast_builder,
        )
      };

    // construct `var [binding_name_for_namespace_object_ref] = __exportAll(...)`
    let decl_stmt = Statement::new_var_decl(
      binding_name_for_namespace_object_ref,
      module_namespace_rhs,
      &self.ast_builder,
    );

    let export_all_externals_rec_ids = &self.ctx.linking_info.star_exports_from_external_modules;

    let mut re_export_external_stmts: Option<_> = None;
    if !export_all_externals_rec_ids.is_empty() {
      match self.ctx.options.format {
        OutputFormat::Esm => {
          let re_export_name = self.canonical_name_for_runtime("__reExport");
          let stmts = export_all_externals_rec_ids.iter().copied().flat_map(|idx| {
            let rec = &self.ctx.module.import_records[idx];
            if !self
              .ctx
              .linking_info
              .ns_star_external_re_export_emitted(rec.meta, self.ctx.options.format)
            {
              return vec![];
            }
            // importee_exports
            let importee_namespace_name = self.canonical_name_for(rec.namespace_ref);
            let Some(Module::External(module)) =
              rec.resolved_module.and_then(|module_idx| self.ctx.modules.get(module_idx))
            else {
              return vec![];
            };
            let importee_name = &module.get_import_path(self.ctx.chunk, self.ctx.resolved_paths);
            // construct `__reExport(importer_exports, importee_exports)`
            let call_expr = CallExpression::new_re_export_call(
              Expression::new_id_ref_expr(SPAN, re_export_name, &self.ast_builder),
              Expression::new_id_ref_expr(
                SPAN,
                binding_name_for_namespace_object_ref,
                &self.ast_builder,
              ),
              Expression::new_id_ref_expr(SPAN, importee_namespace_name, &self.ast_builder),
              &self.ast_builder,
            );
            vec![
              // Insert `import * as ns from 'ext'`external module in esm format
              Statement::new_import_star_stmt(
                importee_name,
                importee_namespace_name,
                &self.ast_builder,
              ),
              // Insert `__reExport(foo_exports, ns)`
              ast::Statement::new_expression_statement(
                SPAN,
                Expression::CallExpression(call_expr.into_in(self.alloc)),
                &self.ast_builder,
              ),
            ]
          });
          re_export_external_stmts = Some(stmts.collect::<Vec<_>>());
        }
        OutputFormat::Cjs | OutputFormat::Iife | OutputFormat::Umd => {
          let stmts = export_all_externals_rec_ids.iter().copied().filter_map(|idx| {
            // importer_exports
            let (importer_namespace_ref_expr, _) = self.finalized_expr_for_symbol_ref(
              self.ctx.module.namespace_object_ref,
              false,
              false,
            );
            let rec = &self.ctx.module.import_records[idx];
            let importee = rec.resolved_module.map(|module_idx| &self.ctx.modules[module_idx])?;

            let re_export_call_expr = CallExpression::new_re_export_call(
              // Insert `__reExport(importer_exports, require('ext'))`
              self.finalized_expr_for_runtime_symbol("__reExport"),
              importer_namespace_ref_expr,
              Expression::new_call_with_arg(
                ast::Expression::new_identifier(SPAN, "require", &self.ast_builder),
                ast::Expression::new_string_literal(
                  SPAN,
                  oxc::ast::ast::Str::from_str_in(importee.id().as_str(), &self.ast_builder),
                  None,
                  &self.ast_builder,
                ),
                false,
                &self.ast_builder,
              ),
              &self.ast_builder,
            );

            Some(ast::Statement::new_expression_statement(
              SPAN,
              Expression::CallExpression(re_export_call_expr.into_in(self.alloc)),
              &self.ast_builder,
            ))
          });
          re_export_external_stmts = Some(stmts.collect());
        }
      }
    }

    let mut ret = vec![decl_stmt];
    ret.extend(re_export_external_stmts.unwrap_or_default());

    ret
  }

  // Handle `import.meta.xxx`, `import.meta['xxx']`, `import.meta?.xxx` and `import.meta?.['xxx']`
  pub fn try_rewrite_import_meta_prop_expr(
    &mut self,
    member_expr: &ast::MemberExpression<'ast>,
  ) -> Option<Expression<'ast>> {
    if member_expr.object().is_import_meta() {
      let original_expr_span = member_expr.span();
      let can_polyfill_import_meta_url = self.can_polyfill_import_meta_url();

      let property_name = member_expr.static_property_name()?;
      match property_name {
        // Try to polyfill `import.meta.url`
        "url" => {
          let new_expr = if can_polyfill_import_meta_url {
            // Replace it with `require('url').pathToFileURL(__filename).href`

            // require('url')
            let require_call = ast::CallExpression::boxed(
              SPAN,
              ast::Expression::new_identifier(SPAN, "require", &self.ast_builder),
              NONE,
              oxc::allocator::Vec::from_value_in(
                ast::Argument::StringLiteral(ast::StringLiteral::boxed(
                  SPAN,
                  "url",
                  None,
                  &self.ast_builder,
                )),
                &self.ast_builder,
              ),
              false,
              &self.ast_builder,
            );

            // require('url').pathToFileURL
            let require_path_to_file_url = ast::StaticMemberExpression::boxed(
              SPAN,
              ast::Expression::CallExpression(require_call),
              ast::IdentifierName::new(SPAN, "pathToFileURL", &self.ast_builder),
              false,
              &self.ast_builder,
            );

            // require('url').pathToFileURL(__filename)
            let require_path_to_file_url_call = ast::CallExpression::boxed(
              SPAN,
              ast::Expression::StaticMemberExpression(require_path_to_file_url),
              NONE,
              oxc::allocator::Vec::from_value_in(
                ast::Argument::Identifier(ast::IdentifierReference::boxed(
                  SPAN,
                  "__filename",
                  &self.ast_builder,
                )),
                &self.ast_builder,
              ),
              false,
              &self.ast_builder,
            );

            // require('url').pathToFileURL(__filename).href
            let require_path_to_file_url_href = ast::StaticMemberExpression::boxed(
              original_expr_span,
              ast::Expression::CallExpression(require_path_to_file_url_call),
              ast::IdentifierName::new(SPAN, "href", &self.ast_builder),
              false,
              &self.ast_builder,
            );
            Some(ast::Expression::StaticMemberExpression(require_path_to_file_url_href))
          } else {
            // If we don't support polyfill `import.meta.url` in this platform and format, we just keep it as it is
            // so users may handle it in their own way.
            if !self.ctx.options.format.keep_esm_import_export_syntax() {
              // Claim the span before walking reaches the bare `import.meta`, so the warning knows
              // this is an `import.meta.url`
              self.record_surviving_import_meta(
                member_expr.object().span(),
                EmptyImportMetaKind::Url,
              );
            }
            None
          };
          return new_expr;
        }
        "dirname" | "filename" => {
          let name =
            oxc::ast::ast::Str::from_str_in(&format!("__{property_name}"), &self.ast_builder);
          return can_polyfill_import_meta_url.then_some(ast::Expression::Identifier(
            ast::IdentifierReference::boxed(SPAN, name, &self.ast_builder),
          ));
        }
        _ => {}
      }
      return self.rewrite_rolldown_file_url(
        property_name,
        original_expr_span,
        member_expr.node_id(),
      );
    }
    None
  }

  fn can_polyfill_import_meta_url(&self) -> bool {
    matches!(
      (self.ctx.options.platform, &self.ctx.options.format),
      (Platform::Node, OutputFormat::Cjs)
    )
  }

  /// Remember an `import.meta` that no rewrite could get rid of, so it is left to be replaced with
  /// an empty object. Callers are responsible for only reaching this on a non-esm output, which
  /// keeps `import.meta` as-is rather than replacing it.
  pub fn record_surviving_import_meta(&mut self, span: Span, kind: EmptyImportMetaKind) {
    self.surviving_import_meta_spans.entry(span).or_insert(kind);
  }

  fn rewrite_rolldown_file_url(
    &mut self,
    property_name: &str,
    original_expr_span: Span,
    node_id: NodeId,
  ) -> Option<Expression<'ast>> {
    // rewrite `import.meta.ROLLDOWN_FILE_URL_<referenceId>`
    if let Some(reference_id) = utils::file_url::strip_file_url_prefix(property_name) {
      // A plugin's `resolveFileUrl` result wins over the default. Copy the `&'me`
      // reference out of `ctx` first, so the lookup does not borrow `self` and the
      // error path below can borrow it mutably.
      let resolved_file_urls = self.ctx.resolved_file_urls;
      if let Some(resolved) = resolved_file_urls.get(&(self.ctx.idx, node_id)) {
        // The only place this code is parsed. The driver deliberately hands it over
        // unparsed, along with the plugin that produced it.
        match parse_injected_expression(self.alloc, &resolved.code) {
          Ok(mut expr) => {
            let mut rewriter = ResolveFileUrlHookResultSpanRewriter(original_expr_span);
            oxc::ast_visit::VisitMut::visit_expression(&mut rewriter, &mut expr);
            return Some(expr);
          }
          Err(diagnostics) => {
            self.resolve_file_url_errors.push((
              resolved.plugin_name.clone(),
              format!(
                "The `resolveFileUrl` hook returned code that is not a valid expression for referenceId={reference_id}: {}
{diagnostics}",
                resolved.code
              ),
            ));
            return None;
          }
        }
      }

      // compute relative path from chunk to asset
      let Ok(asset_file_name) = self.ctx.file_emitter.get_file_name(reference_id) else {
        // Keep the span of the first access, so the diagnostic can point at the source.
        self
          .missing_file_reference_ids
          .entry(CompactStr::new(reference_id))
          .or_insert(original_expr_span);
        return None;
      };
      let output_dir =
        absolutize_path_buf(self.ctx.options.cwd.as_path().join(&self.ctx.options.out_dir));
      let absolute_asset_file_name = asset_file_name.absolutize_with(output_dir);
      let relative_asset_path = &self.ctx.chunk.relative_path_for(&absolute_asset_file_name);

      if !self.ctx.options.format.keep_esm_import_export_syntax()
        && !self.can_polyfill_import_meta_url()
      {
        // Record the origin before walking the generated `import.meta.url`. The generic URL
        // handler reaches the same span later, and first-insert-wins preserves this richer kind.
        self.record_surviving_import_meta(original_expr_span, EmptyImportMetaKind::RolldownFileUrl);
      }

      // new URL({relative_asset_path}, import.meta.url).href
      // TODO: needs import.meta.url polyfill for non esm
      let new_expr = ast::Expression::StaticMemberExpression(ast::StaticMemberExpression::boxed(
        SPAN,
        ast::Expression::new_new_expression(
          SPAN,
          ast::Expression::new_identifier(SPAN, "URL", &self.ast_builder),
          NONE,
          oxc::allocator::Vec::from_array_in(
            [
              ast::Argument::StringLiteral(ast::StringLiteral::boxed(
                SPAN,
                oxc::ast::ast::Str::from_str_in(relative_asset_path, &self.ast_builder),
                None,
                &self.ast_builder,
              )),
              ast::Argument::StaticMemberExpression(ast::StaticMemberExpression::boxed(
                SPAN,
                ast::Expression::new_import_meta(
                  // Carry the source span, so that if this generated `import.meta.url` cannot be
                  // polyfilled either, the diagnostic points at the `import.meta.ROLLDOWN_FILE_URL_*`
                  // the user actually wrote.
                  original_expr_span,
                  &self.ast_builder,
                ),
                ast::IdentifierName::new(SPAN, "url", &self.ast_builder),
                false,
                &self.ast_builder,
              )),
            ],
            &self.ast_builder,
          ),
          &self.ast_builder,
        ),
        ast::IdentifierName::new(SPAN, "href", &self.ast_builder),
        false,
        &self.ast_builder,
      ));
      return Some(new_expr);
    }
    None
  }

  pub fn handle_new_url_with_string_literal_and_import_meta_url(
    &self,
    expr: &mut ast::NewExpression<'ast>,
  ) -> Option<()> {
    let &rec_idx = self.ctx.module.new_url_references.get(&expr.node_id())?;
    let rec = &self.ctx.module.import_records[rec_idx];
    let is_callee_global_url = matches!(expr.callee.as_identifier(), Some(ident) if ident.name == "URL" && self.is_global_identifier_reference(ident));

    if !is_callee_global_url {
      return None;
    }

    let is_second_arg_import_meta_url = expr
      .arguments
      .get(1)
      .is_some_and(|arg| arg.as_expression().is_some_and(ExpressionExt::is_import_meta_url));

    if !is_second_arg_import_meta_url {
      return None;
    }

    let first_arg_expr = expr.arguments.first_mut().and_then(|a| a.as_expression_mut())?;
    // bail if not a static string literal
    match &first_arg_expr {
      ast::Expression::StringLiteral(_) => {}
      ast::Expression::TemplateLiteral(tpl) if tpl.is_no_substitution_template() => {}
      _ => return None,
    }

    let importee =
      rec.resolved_module.and_then(|module_idx| self.ctx.modules[module_idx].as_normal())?;

    // Look up the emitted asset filename via the FileEmitter bridge
    let ref_id = self.ctx.file_emitter.file_ref_for_module(&importee.id)?;
    let filename = self.ctx.file_emitter.get_file_name(&ref_id).ok()?;
    let abs_path = self.ctx.options.cwd.join(&self.ctx.options.out_dir).join(filename.as_str());
    let import_path = self.ctx.chunk.relative_path_for(abs_path.as_path());

    *first_arg_expr = ast::Expression::new_string_literal(
      first_arg_expr.span(),
      oxc::ast::ast::Str::from_str_in(&import_path, &self.ast_builder),
      None,
      &self.ast_builder,
    );
    None
  }

  /// try rewrite `foo_exports.bar` or `foo_exports['bar']`  to `bar` directly
  /// try rewrite `import.meta`
  fn try_rewrite_member_expr(
    &mut self,
    member_expr: &ast::MemberExpression<'ast>,
  ) -> Option<Expression<'ast>> {
    let span = member_expr.span();
    match self.ctx.linking_info.resolved_member_expr_refs.get(&member_expr.node_id()) {
      Some(MemberExprRefResolution {
        resolved: object_ref,
        prop_and_related_span_list: props,
        target_commonjs_exported_symbol: target_commonjs_exported_symbol_meta,
        ..
      }) => object_ref
        .map(|object_ref| {
          let mut is_inlined_commonjs_export = false;
          let object_ref_expr = if let Some(export_meta) = target_commonjs_exported_symbol_meta
            .and_then(|target_commonjs_exported_symbol_meta| {
              self.ctx.constant_value_map.get(&target_commonjs_exported_symbol_meta.0)
            }) {
            is_inlined_commonjs_export = true;
            export_meta.value.to_expression(self.ast_builder.builder())
          } else {
            let (object_ref_expr, _) = self.finalized_expr_for_symbol_ref(
              object_ref,
              false,
              target_commonjs_exported_symbol_meta
                .is_some_and(|(_symbol, is_exports_default)| !is_exports_default),
            );
            object_ref_expr
          };
          Expression::new_member_expr_or_ident_ref(
            object_ref_expr,
            // For commonjs member_expr resolving, the resolved ref is always namespace_alias,
            // so the props actually include the exported name, when inline member_expr access of commonjs exported
            // symbol, we should skip the first prop
            &props[usize::from(is_inlined_commonjs_export)..],
            span,
            &self.ast_builder,
          )
        })
        .or_else(|| {
          Some(Expression::new_member_expr_with_void_zero_object(props, span, &self.ast_builder))
        }),
      _ => self.try_rewrite_import_meta_prop_expr(member_expr),
    }
  }

  /// Try to rewrite a member expression assignment target when the object is a default import from CJS.
  /// For `import_src.log = value`, if `import_src` is from a CJS module, we need to rewrite to
  /// `import_src.default.log = value` because __toESM creates getter-only properties.
  fn try_rewrite_cjs_member_expr_assignment_target(
    &self,
    target: &ast::SimpleAssignmentTarget<'ast>,
  ) -> Option<ast::SimpleAssignmentTarget<'ast>> {
    let (id_ref, property) = match target {
      ast::SimpleAssignmentTarget::StaticMemberExpression(member_expr) => {
        let ast::Expression::Identifier(id_ref) = &member_expr.object else {
          return None;
        };
        (id_ref, CjsMemberProperty::Static(member_expr.property.name.as_str()))
      }
      ast::SimpleAssignmentTarget::ComputedMemberExpression(member_expr) => {
        let ast::Expression::Identifier(id_ref) = &member_expr.object else {
          return None;
        };
        (id_ref, CjsMemberProperty::Computed(&member_expr.expression))
      }
      _ => return None,
    };

    // Resolve the identifier to check if it's a CJS default import
    let reference_id = id_ref.reference_id.get()?;
    let symbol_id = self.scope.symbol_id_for(reference_id)?;
    let symbol_ref: SymbolRef = (self.ctx.idx, symbol_id).into();
    let canonical_ref = self.ctx.symbol_db.canonical_ref_for(symbol_ref);
    let symbol = self.ctx.symbol_db.get(canonical_ref);

    // Check if this symbol has a namespace_alias with property_name "default"
    // This indicates it's a default import from a CJS module
    let ns_alias = symbol.namespace_alias.as_ref()?;
    if ns_alias.property_name.as_str() != "default" {
      return None;
    }

    // Build: ns_name.default. The resolved_member_expr_refs lookup is keyed by post-semantic
    // NodeId, so this synthetic expression won't match scan-time records.
    let ns_name = self.canonical_name_for(ns_alias.namespace_ref);
    let ns_id_ref = Expression::new_id_ref_expr(SPAN, ns_name, &self.ast_builder);
    let default_access =
      ast::Expression::StaticMemberExpression(ast::StaticMemberExpression::boxed(
        SPAN,
        ns_id_ref,
        ast::IdentifierName::new(SPAN, "default", &self.ast_builder),
        false,
        &self.ast_builder,
      ));

    // Create: ns_name.default.property or ns_name.default[expression]
    match property {
      CjsMemberProperty::Static(property_name) => {
        let final_access = ast::StaticMemberExpression::boxed(
          SPAN,
          default_access,
          IdentifierName::new_id_name(SPAN, property_name, &self.ast_builder),
          false,
          &self.ast_builder,
        );
        Some(ast::SimpleAssignmentTarget::StaticMemberExpression(final_access))
      }
      CjsMemberProperty::Computed(expr) => {
        // Finalize the computed key expression (e.g. inline constants) so that an
        // inlined value is emitted instead of a reference to a tree-shaken binding.
        let finalized_expr = match expr {
          ast::Expression::Identifier(ident_ref) => self
            .try_rewrite_identifier_reference_expr(ident_ref, false)
            .unwrap_or_else(|| expr.clone_in(self.alloc)),
          _ => expr.clone_in(self.alloc),
        };
        let final_access = ast::ComputedMemberExpression::boxed(
          SPAN,
          default_access,
          finalized_expr,
          false,
          &self.ast_builder,
        );
        Some(ast::SimpleAssignmentTarget::ComputedMemberExpression(final_access))
      }
    }
  }

  /// Returns `(original_name, canonical_name)` for keep_names processing.
  /// Returns `Some` only if the name has been deconflicted (renamed).
  fn get_keep_name_info(&self, id: KeepNameId) -> Option<(&'me str, &'me str)> {
    let symbol_ref: SymbolRef = match id {
      KeepNameId::SymbolId(symbol_id) => (self.ctx.idx, symbol_id).into(),
      KeepNameId::ReferenceId(reference_id) => {
        let symbol_id = self.scope.symbol_id_for(reference_id)?;
        (self.ctx.idx, symbol_id).into()
      }
      KeepNameId::CompactStr(_) => {
        return None;
      }
    };

    let original_name = symbol_ref.name(self.ctx.symbol_db);
    let canonical_name = self.canonical_name_for(symbol_ref);
    (original_name != canonical_name).then_some((original_name, canonical_name))
  }

  /// rewrite toplevel `class ClassName {}` to `var ClassName = class {}`
  ///
  /// Takes the class by value so its box can be reused as the class expression;
  /// gives it back unchanged when no transformation applies.
  fn get_transformed_class_decl(
    &self,
    mut class: allocator::Box<'ast, ast::Class<'ast>>,
  ) -> Result<ast::Declaration<'ast>, allocator::Box<'ast, ast::Class<'ast>>> {
    let Some(scope_id) = class.scope_id.get() else { return Err(class) };

    if self.scope.scoping().scope_parent_id(scope_id) != Some(self.scope.scoping().root_scope_id())
    {
      return Err(class);
    }

    let Some(id) = class.id.take() else { return Err(class) };

    if let Some(symbol_id) = id.symbol_id.get() {
      if self.ctx.module.self_referenced_class_decl_symbol_ids.contains(&symbol_id) {
        // class T { static a = new T(); }
        // needs to rewrite to `var T = class T { static a = new T(); }`
        let mut id = id.clone();
        let new_name = self.canonical_name_for((self.ctx.idx, symbol_id).into());
        id.name = oxc::ast::ast::Str::from_str_in(new_name, &self.ast_builder).into();
        class.id = Some(id);
      }
    }
    Ok(ast::Declaration::new_variable_declaration(
      class.span,
      VariableDeclarationKind::Var,
      oxc::allocator::Vec::from_value_in(
        ast::VariableDeclarator::new(
          SPAN,
          VariableDeclarationKind::Var,
          ast::BindingPattern::BindingIdentifier(oxc::allocator::Box::new_in(
            id,
            &self.ast_builder,
          )),
          NONE,
          Some(Expression::ClassExpression(class)),
          false,
          &self.ast_builder,
        ),
        &self.ast_builder,
      ),
      false,
      &self.ast_builder,
    ))
  }

  fn try_rewrite_global_require_call(
    &self,
    call_expr: &mut ast::CallExpression<'ast>,
  ) -> Option<Expression<'ast>> {
    if call_expr.is_global_require_call(self.scope) {
      //  `require` calls that can't be recognized by rolldown are ignored in scanning, so they were not stored in `NormalModule#imports`.
      //  we just keep these `require` calls as it is
      if let Some(rec_idx) = self.ctx.module.imports.get(&call_expr.node_id()).copied() {
        let rec = &self.ctx.module.import_records[rec_idx];
        let module_idx = rec.resolved_module?;
        // use `__require` instead of `require`
        if rec.meta.contains(ImportRecordMeta::CallRuntimeRequire) {
          *call_expr.callee.get_inner_expression_mut() =
            self.finalized_expr_for_runtime_symbol("__require");
        }
        let rewrite_ast = match &self.ctx.modules[module_idx] {
          Module::Normal(importee) => {
            match importee.module_type {
              ModuleType::Json => {
                // Nodejs treats json files as an esm module with a default export and rolldown follows this behavior.
                // And to make sure the runtime behavior is correct, we need to rewrite `require('xxx.json')` to `require('xxx.json').default` to align with the runtime behavior of nodejs.

                // Rewrite `require(...)` to `require_xxx(...)` or `(init_xxx(), __toCommonJS(xxx_exports).default)`
                let importee_linking_info = &self.ctx.linking_infos[importee.idx];
                let (wrap_ref_expr, hint) = self.finalized_expr_for_symbol_ref(
                  importee_linking_info.wrapper_ref.unwrap(),
                  false,
                  false,
                );
                if matches!(importee.exports_kind, ExportsKind::CommonJs) {
                  if hint.contains(FinalizedExprProcessHint::FromCjsWrapKindEntry) {
                    Some(wrap_ref_expr)
                  } else {
                    Some(ast::Expression::CallExpression(ast::CallExpression::boxed(
                      SPAN,
                      wrap_ref_expr,
                      NONE,
                      oxc::allocator::Vec::new_in(&self.ast_builder),
                      false,
                      &self.ast_builder,
                    )))
                  }
                } else {
                  let (ns_name, _) =
                    self.finalized_expr_for_symbol_ref(importee.namespace_object_ref, false, false);
                  let to_commonjs_ref_name = self.finalized_expr_for_runtime_symbol("__toCommonJS");
                  Some(Expression::new_seq_in_parens(
                    ast::Expression::CallExpression(ast::CallExpression::boxed(
                      SPAN,
                      wrap_ref_expr,
                      NONE,
                      oxc::allocator::Vec::new_in(&self.ast_builder),
                      false,
                      &self.ast_builder,
                    )),
                    ast::Expression::StaticMemberExpression(ast::StaticMemberExpression::boxed(
                      SPAN,
                      Expression::new_call_with_arg(
                        to_commonjs_ref_name,
                        ns_name,
                        false,
                        &self.ast_builder,
                      ),
                      ast::IdentifierName::new(SPAN, "default", &self.ast_builder),
                      false,
                      &self.ast_builder,
                    )),
                    &self.ast_builder,
                  ))
                }
              }
              _ => {
                // Rewrite `require(...)` to `require_xxx(...)` or `(init_xxx(), __toCommonJS(xxx_exports))`
                let importee_linking_info = &self.ctx.linking_infos[importee.idx];
                // `init_xxx` or `require_xxx`
                let (wrap_ref_expr, hint) = self.finalized_expr_for_symbol_ref(
                  importee_linking_info.wrapper_ref.unwrap(),
                  false,
                  false,
                );

                // `init_xxx()` or `require_xxx()` or `require_xxx`
                let wrap_ref_call_expr =
                  if hint.contains(FinalizedExprProcessHint::FromCjsWrapKindEntry) {
                    wrap_ref_expr
                  } else {
                    ast::Expression::CallExpression(ast::CallExpression::boxed_with_pure(
                      SPAN,
                      wrap_ref_expr,
                      NONE,
                      oxc::allocator::Vec::new_in(&self.ast_builder),
                      false,
                      self.ctx.final_esm_init_metadata.init_is_noop(importee.idx),
                      &self.ast_builder,
                    ))
                  };

                if matches!(importee.exports_kind, ExportsKind::CommonJs)
                  || rec.meta.contains(ImportRecordMeta::IsRequireUnused)
                {
                  // `init_xxx()`
                  Some(wrap_ref_call_expr)
                } else {
                  // `xxx_exports`
                  let (namespace_object_ref_expr, _) =
                    self.finalized_expr_for_symbol_ref(importee.namespace_object_ref, false, false);

                  let is_json_module = rec.meta.contains(ImportRecordMeta::JsonModule);

                  // `__toCommonJS`
                  let to_commonjs_expr = self.finalized_expr_for_runtime_symbol("__toCommonJS");
                  // `__toCommonJS(xxx_exports)`
                  let to_commonjs_call_expr =
                    ast::Expression::CallExpression(ast::CallExpression::boxed(
                      SPAN,
                      to_commonjs_expr,
                      NONE,
                      oxc::allocator::Vec::from_value_in(
                        ast::Argument::from(namespace_object_ref_expr),
                        &self.ast_builder,
                      ),
                      false,
                      &self.ast_builder,
                    ));

                  let final_expr = if is_json_module {
                    // `__toCommonJS(xxx_exports).default`
                    Expression::from(ast::MemberExpression::new_static_member_expression(
                      SPAN,
                      to_commonjs_call_expr,
                      ast::IdentifierName::new(SPAN, "default", &self.ast_builder),
                      false,
                      &self.ast_builder,
                    ))
                  } else {
                    to_commonjs_call_expr
                  };

                  // `(init_xxx(), __toCommonJS(xxx_exports))`
                  Some(Expression::new_seq_in_parens(
                    wrap_ref_call_expr,
                    final_expr,
                    &self.ast_builder,
                  ))
                }
              }
            }
          }
          Module::External(importee) => {
            let request_path =
              call_expr.arguments.get_mut(0).expect("require should have an argument");
            // Rewrite `require('xxx')` to `require('fs')`, if there is an alias that maps 'xxx' to 'fs'
            *request_path = ast::Argument::StringLiteral(ast::StringLiteral::boxed(
              request_path.span(),
              oxc::ast::ast::Str::from_str_in(
                &importee.get_import_path(self.ctx.chunk, self.ctx.resolved_paths),
                &self.ast_builder,
              ),
              None,
              &self.ast_builder,
            ));
            None
          }
        };
        return rewrite_ast;
      }
    }
    None
  }

  fn try_rewrite_inline_dynamic_import_expr(
    &self,
    import_expr: &oxc::allocator::Box<'ast, ImportExpression<'ast>>,
  ) -> Option<Expression<'ast>> {
    let rec_idx = self.ctx.module.imports.get(&import_expr.node_id())?;
    let rec = &self.ctx.module.import_records[*rec_idx];
    let importee_id = rec.resolved_module?;

    if rec.meta.contains(ImportRecordMeta::DeadDynamicImport) {
      // `Promise.resolve().then(() => /* @__PURE__ */ Object.freeze({ __proto__: null }))`
      return Some(Expression::new_promise_resolve_then(
        Expression::new_call_with_arg(
          Expression::new_member_access_expr("Object", "freeze", &self.ast_builder),
          ast::Expression::ObjectExpression(ast::ObjectExpression::boxed(
            SPAN,
            oxc::allocator::Vec::from_value_in(
              ast::ObjectPropertyKind::new_object_property(
                SPAN,
                ast::PropertyKind::Init,
                ast::PropertyKey::new_static_identifier(SPAN, "__proto__", &self.ast_builder),
                ast::Expression::NullLiteral(ast::NullLiteral::boxed(SPAN, &self.ast_builder)),
                false,
                false,
                false,
                &self.ast_builder,
              ),
              &self.ast_builder,
            ),
            &self.ast_builder,
          )),
          true,
          &self.ast_builder,
        ),
        &self.ast_builder,
      ));
    }

    if self.ctx.options.code_splitting.is_disabled() {
      match &self.ctx.modules[importee_id] {
        Module::Normal(importee) => {
          let importee_linking_info = &self.ctx.linking_infos[importee_id];
          let new_expr = match importee_linking_info.wrap_kind() {
            WrapKind::Esm => {
              // Rewrite `import('./foo.mjs')` to `(init_foo(), foo_exports)`
              let importee_linking_info = &self.ctx.linking_infos[importee_id];

              // `init_foo`
              let importee_wrapper_ref_name =
                self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());

              // `foo_exports`
              let importee_namespace_name = self.canonical_name_for(importee.namespace_object_ref);

              if importee_linking_info.is_tla_or_contains_tla_dependency {
                // `init_foo().then(function() { return foo_exports })`
                Some(Expression::new_callee_then_call(
                  ast::Expression::new_call_expression(
                    SPAN,
                    Expression::new_id_ref_expr(SPAN, importee_wrapper_ref_name, &self.ast_builder),
                    NONE,
                    oxc::allocator::Vec::new_in(&self.ast_builder),
                    false,
                    &self.ast_builder,
                  ),
                  Expression::new_id_ref_expr(SPAN, importee_namespace_name, &self.ast_builder),
                  &self.ast_builder,
                ))
              } else {
                //  Promise.resolve().then(function() { return (init_foo(), foo_exports) })
                Some(Expression::new_promise_resolve_then(
                  Expression::new_seq_in_parens(
                    ast::Expression::new_call_expression(
                      SPAN,
                      Expression::new_id_ref_expr(
                        SPAN,
                        importee_wrapper_ref_name,
                        &self.ast_builder,
                      ),
                      NONE,
                      oxc::allocator::Vec::new_in(&self.ast_builder),
                      false,
                      &self.ast_builder,
                    ),
                    Expression::new_id_ref_expr(SPAN, importee_namespace_name, &self.ast_builder),
                    &self.ast_builder,
                  ),
                  &self.ast_builder,
                ))
              }
            }
            WrapKind::Cjs => {
              //  `__toESM(require_foo())`
              let to_esm_fn_name = self.canonical_name_for_runtime("__toESM");
              let importee_wrapper_ref_name =
                self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
              Some(Expression::new_promise_resolve_then(
                Expression::new_to_esm_wrapper(
                  Expression::new_id_ref_expr(SPAN, to_esm_fn_name, &self.ast_builder),
                  ast::Expression::new_call_expression(
                    SPAN,
                    Expression::new_id_ref_expr(SPAN, importee_wrapper_ref_name, &self.ast_builder),
                    NONE,
                    oxc::allocator::Vec::new_in(&self.ast_builder),
                    false,
                    &self.ast_builder,
                  ),
                  self.ctx.module.should_consider_node_esm_spec_for_dynamic_import(),
                  &self.ast_builder,
                ),
                &self.ast_builder,
              ))
            }
            WrapKind::None => {
              // The nature of `import()` is to load the module dynamically/lazily, so imported modules would
              // must be wrapped, so we could make sure the module is executed lazily.
              if cfg!(debug_assertions) {
                unreachable!()
              }
              None
            }
          };
          return new_expr;
        }
        Module::External(_) => {
          // iife format doesn't support external module
        }
      }
    }
    None
  }

  #[expect(clippy::too_many_lines)]
  fn remove_unused_top_level_stmt(&mut self, program: &mut ast::Program<'ast>) -> usize {
    let mut last_import_stmt_idx = None;

    let old_body = program.body.take_in(&self.alloc);
    // the first statement info is the namespace variable declaration
    // skip first statement info to make sure `program.body` has same index as `stmt_infos`
    old_body.into_iter().enumerate().zip(self.ctx.stmt_infos.iter_enumerated().skip(1)).for_each(
      |((_top_stmt_idx, mut top_stmt), (stmt_info_idx, stmt_info))| {
        let is_order_runtime_stmt =
          self.ctx.order_wrap_state.forces_runtime_stmt(self.ctx.runtime, self.ctx.idx, stmt_info);
        let is_stmt_included =
          self.ctx.linking_info.stmt_info_included.has_bit(stmt_info_idx) || is_order_runtime_stmt;

        if !is_stmt_included {
          // For ESM-wrapped modules, excluded re-export statements still need init calls for
          // correct initialization order. Their targets are precomputed by the generate
          // stage's `Sealed<FinalEsmInitMetadata>`; emitting the calls (with the module-wide dedup
          // below) is all that happens here.
          if let Some(targets) = self
            .ctx
            .final_esm_init_metadata
            .transitive_init_targets(self.ctx.idx)
            .and_then(|targets_by_stmt| targets_by_stmt.get(&stmt_info_idx))
          {
            for &importee_idx in targets {
              if self.generated_init_esm_importee_ids.insert(importee_idx) {
                // An excluded re-export can forward to a TLA-tainted wrapper. The current module
                // is then TLA-tainted as well, so its async init body must await the forwarded
                // promise before later statements observe the importee's bindings.
                let init_expr = self.wrapped_esm_init_call_expr(importee_idx, SPAN, true, true);
                program.body.push(ast::Statement::new_expression_statement(
                  SPAN,
                  init_expr,
                  &self.ast_builder,
                ));
              }
            }
          }
          let overlay_records = self
            .ctx
            .order_wrap_state
            .import_overlays_for_statement(self.ctx.idx, stmt_info_idx)
            .map(|(key, overlay)| {
              (
                key.record,
                overlay.reexports_dynamic_exports,
                !overlay.retained_reexport_path.is_empty(),
              )
            })
            .collect::<Vec<_>>();
          for (rec_idx, reexports_dynamic_exports, has_retained_reexport_path) in overlay_records {
            if !has_retained_reexport_path
              && let Some(init_stmt) = self.wrapped_esm_init_stmt_for_import_record(rec_idx)
            {
              program.body.push(init_stmt);
            }
            if !reexports_dynamic_exports {
              continue;
            }
            let Some(importee_idx) = self.ctx.module.import_records[rec_idx].resolved_module else {
              continue;
            };
            let Some(importee) = self.ctx.modules[importee_idx].as_normal() else {
              continue;
            };
            let (importer_namespace_ref, _) = self.finalized_expr_for_symbol_ref(
              self.ctx.module.namespace_object_ref,
              false,
              false,
            );
            let (importee_namespace_ref, _) =
              self.finalized_expr_for_symbol_ref(importee.namespace_object_ref, false, false);
            let call_expr = CallExpression::new_re_export_call(
              self.finalized_expr_for_runtime_symbol("__reExport"),
              importer_namespace_ref,
              importee_namespace_ref, &self.ast_builder
            );
            program.body.push(ast::Statement::new_expression_statement(
              top_stmt.span(),
              Expression::CallExpression(call_expr.into_in(self.alloc)),
              &self.ast_builder,
            ));
          }
          return;
        }

        let is_module_decl = is_stmt_included && top_stmt.is_module_declaration_with_source();

        if let Some(import_decl) = top_stmt.as_import_declaration() {
          let span = import_decl.span;
          let rec_idx = self.ctx.module.imports[&import_decl.node_id()];
          if self.transform_or_remove_import_export_stmt(&mut top_stmt, rec_idx) {
            for comment in &mut program.comments {
              if comment.attached_to == span.start {
                comment.attached_to = 0;
              }
              if comment.attached_to > span.start {
                break;
              }
            }
            return;
          }
        } else if let Some(export_all_decl) = top_stmt.as_export_all_declaration() {
          let rec_idx = self.ctx.module.imports[&export_all_decl.node_id()];
          // "export * as ns from 'path'"
          if let Some(_alias) = &export_all_decl.exported {
            if self.transform_or_remove_import_export_stmt(&mut top_stmt, rec_idx) {
              return;
            }
          } else {
            // "export * from 'path'"
            let rec = &self.ctx.module.import_records[rec_idx];
            let Some(module_idx) = rec.resolved_module else { return };
            match &self.ctx.modules[module_idx] {
              Module::Normal(importee) => {
                let importee_linking_info = &self.ctx.linking_infos[importee.idx];
                // Same shared obligation gate as the import-declaration path; the statement is
                // included by construction here.
                if record_is_init_obligation(
                  ObligationPurpose::Emit,
                  self.ctx.order_wrap_state,
                  self.ctx.idx,
                  rec,
                  rec_idx,
                  true,
                ) && matches!(importee_linking_info.wrap_kind(), WrapKind::None)
                  && let Some(init_stmt) = self.wrapped_esm_init_stmt_for_import_record(rec_idx)
                {
                  program.body.push(init_stmt);
                }

                if matches!(importee_linking_info.wrap_kind(), WrapKind::Esm)
                // If it is a inner concatenated module, we should not call its wrapper here
                  && !matches!(
                    importee_linking_info.concatenated_wrapped_module_kind,
                    ConcatenateWrappedModuleKind::Inner
                  )
                {
                  let (wrapper_ref, _) = self.finalized_expr_for_symbol_ref(
                    importee_linking_info.wrapper_ref.unwrap(),
                    false,
                    false,
                  );
                  let mut init_expr = ast::Expression::new_call_expression(
                    SPAN,
                    wrapper_ref,
                    NONE,
                    oxc::allocator::Vec::new_in(&self.ast_builder),
                    false,
                    &self.ast_builder,
                  );
                  if importee_linking_info.is_tla_or_contains_tla_dependency {
                    init_expr = ast::Expression::AwaitExpression(ast::AwaitExpression::boxed(
                      SPAN,
                      init_expr,
                      &self.ast_builder,
                    ));
                  }
                  program.body.push(ast::Statement::new_expression_statement(
                    SPAN,
                    init_expr,
                    &self.ast_builder,
                  ));
                }

                match importee.exports_kind {
                  ExportsKind::Esm => {
                    if importee_linking_info.has_dynamic_exports {
                      // exports
                      let (importer_namespace_ref, _) = self.finalized_expr_for_symbol_ref(
                        self.ctx.module.namespace_object_ref,
                        false,
                        false,
                      );
                      // otherExports
                      let (importee_namespace_ref, _) = self.finalized_expr_for_symbol_ref(
                        importee.namespace_object_ref,
                        false,
                        false,
                      );

                      let call_expr = CallExpression::new_re_export_call(
                        self.finalized_expr_for_runtime_symbol("__reExport"),
                        importer_namespace_ref,
                        importee_namespace_ref, &self.ast_builder
                      );
                      // __reExport(exports, otherExports)
                      let stmt =
                        ast::Statement::ExpressionStatement(ast::ExpressionStatement::boxed(
                          SPAN,
                          Expression::CallExpression(call_expr.into_in(self.alloc)),
                          &self.ast_builder,
                        ));
                      program.body.push(stmt);
                    }
                  }
                  ExportsKind::CommonJs => {
                    // If **commonjs** treeshake is enabled, the module_namespace is included on
                    // demand, we should skip generate related `__reExport` statements
                    // See: https://github.com/rolldown/rolldown/blob/60fc81ada3955ce84b38a5edbb33a169d1f89f15/crates/rolldown/src/stages/link_stage/reference_needed_symbols.rs?plain=1#L148-L150
                    if !self.module_namespace_included {
                      return;
                    }

                    let re_export_fn_name = self.finalized_expr_for_runtime_symbol("__reExport");

                    // importer_exports
                    let (importer_namespace_ref, _) = self.finalized_expr_for_symbol_ref(
                      self.ctx.module.namespace_object_ref,
                      false,
                      false,
                    );

                    // __toESM
                    let to_esm_fn_ref = self.finalized_expr_for_runtime_symbol("__toESM");

                    // require_foo
                    let (importee_wrapper_ref_expr, _) = self.finalized_expr_for_symbol_ref(
                      importee_linking_info.wrapper_ref.unwrap(),
                      false,
                      false,
                    );

                    let call_expr = CallExpression::new_re_export_call(
                      re_export_fn_name,
                      importer_namespace_ref,
                      Expression::new_to_esm_wrapper(
                        to_esm_fn_ref,
                        ast::Expression::CallExpression(ast::CallExpression::boxed(
                          SPAN,
                          importee_wrapper_ref_expr,
                          NONE,
                          oxc::allocator::Vec::new_in(&self.ast_builder),
                          false,
                          &self.ast_builder,
                        )),
                        self.ctx.module.should_consider_node_esm_spec_for_static_import(), &self.ast_builder
                      ), &self.ast_builder
                    );

                    // __reExport(importer_exports, __toESM(require_foo()))
                    let stmt =
                      ast::Statement::ExpressionStatement(ast::ExpressionStatement::boxed(
                        SPAN,
                        Expression::CallExpression(call_expr.into_in(self.alloc)),
                        &self.ast_builder,
                      ));
                    program.body.push(stmt);
                  }
                  ExportsKind::None => {}
                }
              }
              Module::External(_importee) => {
                match self.ctx.options.format {
                  rolldown_common::OutputFormat::Esm
                  | rolldown_common::OutputFormat::Iife
                  | rolldown_common::OutputFormat::Umd
                  | rolldown_common::OutputFormat::Cjs => {
                    // Just remove the statement
                    return;
                  }
                }
              }
            }

            return;
          }
        } else if let ast::Statement::ExportDefaultDeclaration(default_decl) = top_stmt {
          use ast::ExportDefaultDeclarationKind;
          let default_decl_span = default_decl.span;
          match default_decl.unbox().declaration {
            // Special case: when exporting an identifier that's already the default export symbol
            ast::ExportDefaultDeclarationKind::Identifier(id)
              if self.scope.scoping().get_reference(id.reference_id()).symbol_id().is_some_and(
                |symbol_id| symbol_id == self.ctx.module.default_export_ref.symbol,
              ) =>
            {
              // "let a = ..;export default a" => "let a = ..;" (no transformation needed)
              return;
            }
            decl @ ast::match_expression!(ExportDefaultDeclarationKind) => {
              let mut init_expr = decl.into_expression();
              let canonical_name_for_default_export_ref =
                self.canonical_name_for(self.ctx.module.default_export_ref);

              // Check if we need to add __name() helper for anonymous function/class expressions or arrow functions
              if self.ctx.options.keep_names {
                let inner_expr = init_expr.without_parentheses_mut();
                let needs_inline_name = match inner_expr {
                  ast::Expression::FunctionExpression(func) if func.id.is_none() => true,
                  ast::Expression::ClassExpression(class_expression)
                    if class_expression.id.is_none() =>
                  {
                    if let Some(element) = self.keep_name_helper_for_class(
                      Some(KeepNameId::CompactStr(&CompactStr::from("default"))),
                      &class_expression.body,
                    ) {
                      class_expression.body.body.insert(0, element);
                    }
                    false
                  }
                  ast::Expression::ArrowFunctionExpression(_) => true,
                  _ => false,
                };

                if needs_inline_name {
                  // Wrap the expression inline: `__name(<expr>, "default")`
                  // This matches esbuild's output and allows tree-shaking with PURE annotation
                  let name_ref = self.canonical_ref_for_runtime("__name");
                  let (finalized_callee, _) =
                    self.finalized_expr_for_symbol_ref(name_ref, false, false);
                  init_expr = Expression::new_keep_name_call(
                    "default",
                    init_expr,
                    finalized_callee,
                    true, // pure annotation for tree-shaking
                    &self.ast_builder,
                  );
                }
              }

              top_stmt =
                Statement::new_var_decl(canonical_name_for_default_export_ref, init_expr, &self.ast_builder);
            }
            ast::ExportDefaultDeclarationKind::FunctionDeclaration(mut func) => {
              // "export default function() {}" => "function default() {}"
              // "export default function foo() {}" => "function foo() {}"
              if func.id.is_none() {
                let canonical_name_for_default_export_ref =
                  self.canonical_name_for(self.ctx.module.default_export_ref);
                func.id =
                  Some(BindingIdentifier::new_id(SPAN, canonical_name_for_default_export_ref, &self.ast_builder));

                // When keep_names is enabled, preserve "default" as the function name
                if self.ctx.options.keep_names {
                  // current statement will be pushed to program.body, so the insert position is program.body.len() + 1
                  let insert_position = program.body.len() + 1;
                  self.keep_name_statement_to_insert.push((
                    insert_position,
                    CompactStr::new("default"),
                    CompactStr::new(canonical_name_for_default_export_ref),
                  ));
                }
              }
              top_stmt = ast::Statement::FunctionDeclaration(func);
            }
            ast::ExportDefaultDeclarationKind::ClassDeclaration(mut class) => {
              // "export default class {}" => "class default {}"
              // "export default class Foo {}" => "class Foo {}"
              if class.id.is_none() {
                let canonical_name_for_default_export_ref =
                  self.canonical_name_for(self.ctx.module.default_export_ref);
                class.id =
                  Some(BindingIdentifier::new_id(SPAN, canonical_name_for_default_export_ref, &self.ast_builder));

                // When keep_names is enabled, preserve "default" as the class name
                // Skip if class has static name property
                if self.ctx.options.keep_names {
                  let default_name = CompactStr::from("default");
                  if let Some(element) = self.keep_name_helper_for_class(
                    Some(KeepNameId::CompactStr(&default_name)),
                    &class.body,
                  ) {
                    class.body.body.insert(0, element);
                  }
                }
              }

              // Class should be handled specially, because the `ClassDecl` will be transformed again.
              class.span = default_decl_span;
              top_stmt = ast::Statement::ClassDeclaration(class);
            }
            ast::ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => {
              unreachable!("TypeScript declarations are stripped before the finalizer runs")
            }
          }

          // Transfer span of ExportDefaultDeclaration to FunctionDeclaration to preserve the
          // comments
          *top_stmt.span_mut() = default_decl_span;
        } else if matches!(&top_stmt, ast::Statement::ExportNamedDeclaration(named_decl) if named_decl.source.is_none())
        {
          let ast::Statement::ExportNamedDeclaration(named_decl) = top_stmt else { unreachable!() };
          let named_decl_span = named_decl.span;
          if let Some(mut decl) = named_decl.unbox().declaration {
            // `export var foo = 1` => `var foo = 1`
            // `export function foo() {}` => `function foo() {}`
            // `export class Foo {}` => `class Foo {}`

            *decl.span_mut() = named_decl_span;
            top_stmt = ast::Statement::from(decl);
          } else {
            // `export { foo }`
            // Remove this statement by ignoring it
            return;
          }
        } else if let Some(named_decl) = top_stmt.as_export_named_declaration_mut() {
          // `export { foo } from 'path'`
          let rec_idx = self.ctx.module.imports[&named_decl.node_id()];
          if self.transform_or_remove_import_export_stmt(&mut top_stmt, rec_idx) {
            return;
          }
        }

        if self.ctx.options.top_level_var {
          if let Statement::VariableDeclaration(var_decl) = &mut top_stmt {
            var_decl.kind = ast::VariableDeclarationKind::Var;
            for decl in &mut var_decl.declarations {
              decl.kind = VariableDeclarationKind::Var;
            }
          }
          if let Statement::ClassDeclaration(class_decl) = top_stmt {
            top_stmt = match self.get_transformed_class_decl(class_decl) {
              Ok(decl) => Statement::from(decl),
              Err(class_decl) => Statement::ClassDeclaration(class_decl),
            };
          }
        }
        program.body.push(top_stmt);
        if is_module_decl {
          last_import_stmt_idx = Some(program.body.len());
        }
      },
    );
    last_import_stmt_idx.unwrap_or(0)
  }

  fn process_fn(
    &self,
    symbol_binding_id: Option<KeepNameId>,
    name_binding_id: Option<KeepNameId>,
  ) -> Option<(usize, CompactStr, CompactStr)> {
    if !self.ctx.options.keep_names {
      return None;
    }
    let (original_name, _) = self.get_keep_name_info(name_binding_id?)?;
    let (_, canonical_name) = self.get_keep_name_info(symbol_binding_id?)?;
    let original_name: CompactStr = CompactStr::new(original_name);
    let new_name = CompactStr::new(canonical_name);
    let insert_position = self.cur_stmt_index + 1;
    Some((insert_position, original_name, new_name))
  }

  fn process_keep_name_for_expression(
    &self,
    keep_name_id: Option<KeepNameId>,
    expr: &mut ast::Expression<'ast>,
  ) {
    // Don't rewrite `__name` runtime helper itself.
    if !self.ctx.options.keep_names || self.ctx.runtime.id() == self.ctx.idx {
      return;
    }

    match expr {
      ast::Expression::ClassExpression(class_expression) => {
        // Named class expressions are handled in visit_expression
        if class_expression.id.is_some() {
          return;
        }
        if let Some(element) = self.keep_name_helper_for_class(keep_name_id, &class_expression.body)
        {
          class_expression.body.body.insert(0, element);
        }
      }
      ast::Expression::FunctionExpression(fn_expression) => {
        // Named function expressions are handled in visit_expression
        if fn_expression.id.is_some() {
          return;
        }
        if let Some((_insert_position, original_name, _)) =
          self.process_fn(keep_name_id, keep_name_id)
        {
          let name_ref = self.canonical_ref_for_runtime("__name");
          let (finalized_callee, _) = self.finalized_expr_for_symbol_ref(name_ref, false, false);
          expr.replace_with(|fn_expr| {
            Expression::new_keep_name_call(
              &original_name,
              fn_expr,
              finalized_callee,
              true,
              &self.ast_builder,
            )
          });
        }
      }
      ast::Expression::ArrowFunctionExpression(_fn_expr) => {
        if let Some((_insert_position, original_name, _)) =
          self.process_fn(keep_name_id, keep_name_id)
        {
          let name_ref = self.canonical_ref_for_runtime("__name");
          let (finalized_callee, _) = self.finalized_expr_for_symbol_ref(name_ref, false, false);
          expr.replace_with(|fn_expr| {
            Expression::new_keep_name_call(
              &original_name,
              fn_expr,
              finalized_callee,
              true,
              &self.ast_builder,
            )
          });
        }
      }
      _ => {}
    }
  }

  fn keep_name_helper_for_class(
    &self,
    id: Option<KeepNameId>,
    class_body: &ast::ClassBody<'ast>,
  ) -> Option<ClassElement<'ast>> {
    if !self.ctx.options.keep_names {
      return None;
    }
    let keep_name_id = id?;
    // Skip if the class already has a static `name` property/method
    if Self::class_body_has_static_name(class_body) {
      return None;
    }
    let original_name = match keep_name_id {
      KeepNameId::CompactStr(name) => {
        // CompactStr variant doesn't need conflict resolution - it's already a direct name
        name.clone()
      }
      KeepNameId::SymbolId(_) | KeepNameId::ReferenceId(_) => {
        let (original_name, _) = self.get_keep_name_info(keep_name_id)?;
        let original_name: CompactStr = CompactStr::new(original_name);
        original_name
      }
    };

    let name_ref = self.canonical_ref_for_runtime("__name");
    let (finalized_callee, _) = self.finalized_expr_for_symbol_ref(name_ref, false, false);
    Some(ClassElement::new_static_block_keep_name(
      &original_name,
      finalized_callee,
      &self.ast_builder,
    ))
  }

  /// Check if a class body has a static `name` property, method, or accessor.
  fn class_body_has_static_name(body: &ast::ClassBody<'ast>) -> bool {
    body.body.iter().any(|element| match element {
      ClassElement::MethodDefinition(method) => {
        method.r#static && method.key.static_name().is_some_and(|name| name == "name")
      }
      ClassElement::PropertyDefinition(prop) => {
        prop.r#static && prop.key.static_name().is_some_and(|name| name == "name")
      }
      ClassElement::AccessorProperty(accessor) => {
        accessor.r#static && accessor.key.static_name().is_some_and(|name| name == "name")
      }
      _ => false,
    })
  }

  /// Inserts `__name()` call statements for keeping function/class names.
  /// This method processes the pending keep_name insertions in reverse order.
  fn insert_keep_name_statements(
    &self,
    statements: &mut allocator::Vec<'ast, ast::Statement<'ast>>,
  ) {
    for (stmt_index, original_name, new_name) in self.keep_name_statement_to_insert.iter().rev() {
      let name_ref = self.canonical_ref_for_runtime("__name");
      let (finalized_callee, _) = self.finalized_expr_for_symbol_ref(name_ref, false, false);
      let target = Expression::new_id_ref_expr(SPAN, new_name, &self.ast_builder);
      statements.insert(
        *stmt_index,
        ast::Statement::new_expression_statement(
          SPAN,
          Expression::new_keep_name_call(
            original_name,
            target,
            finalized_callee,
            false,
            &self.ast_builder,
          ),
          &self.ast_builder,
        ),
      );
    }
  }

  /// Rewrites a dynamic import expression when the importee is a merged user defined chunk or a common chunk.
  ///
  /// This handles two cases:
  /// 1. If the importer and importee are in the same chunk:
  ///    Convert `import('./some-module.js')` to `Promise.resolve().then(() => importee_namespace)`
  /// 2. If the importee is in a different chunk:
  ///    Convert `import('./some-module.js')` to `import('./some-module.js').then(n => n.ns)`
  ///
  /// Returns `Some(Expression)` if the import was rewritten, `None` otherwise.
  fn rewrite_dynamic_import_for_merged_entry(
    &self,
    expr: &mut ast::ImportExpression<'ast>,
    importee: &NormalModule,
    importee_chunk: &Chunk,
    importee_chunk_idx: ChunkIdx,
  ) -> Option<Expression<'ast>> {
    let importee_idx = importee.idx;
    // to make sure the semantic is correct after chunk merging optimization.
    let needs_namespace_extraction = self
      .ctx
      .chunk_graph
      .common_chunk_exported_facade_chunk_namespace
      .get(&importee_chunk_idx)
      .is_some_and(|set| set.contains(&importee_idx));

    if !needs_namespace_extraction {
      return None;
    }

    let is_importer_importee_in_same_chunk = importee_chunk.modules.contains(&self.ctx.idx);
    if is_importer_importee_in_same_chunk {
      let importee_meta = &self.ctx.linking_infos[importee.idx];

      let finalized_expr = match importee_meta.wrap_kind() {
        WrapKind::Cjs => {
          let importee_wrapper_ref = self.ctx.linking_infos[importee.idx].wrapper_ref.unwrap();

          let (finalized_importee_wrapper_ref, _) =
            self.finalized_expr_for_symbol_ref(importee_wrapper_ref, false, false);

          let finalized_to_esm = self.finalized_expr_for_runtime_symbol("__toESM");

          // require_xxx()
          let wrapper_ref_call_expr = ast::Expression::new_call_expression(
            SPAN,
            finalized_importee_wrapper_ref,
            NONE,
            oxc::allocator::Vec::new_in(&self.ast_builder),
            false,
            &self.ast_builder,
          );

          // __toESM(require_xxx(), isNodeMode)
          Expression::new_to_esm_wrapper(
            finalized_to_esm,
            wrapper_ref_call_expr,
            self.ctx.module.should_consider_node_esm_spec_for_dynamic_import(),
            &self.ast_builder,
          )
        }
        WrapKind::Esm => {
          // (init_xxx(), namespace_exports)
          let importee_wrapper_ref = self.ctx.linking_infos[importee.idx].wrapper_ref.unwrap();

          let (finalized_importee_wrapper_ref, _) =
            self.finalized_expr_for_symbol_ref(importee_wrapper_ref, false, false);

          let (finalized_namespace, _) =
            self.finalized_expr_for_symbol_ref(importee.namespace_object_ref, false, false);

          // init_xxx()
          let wrapper_call_expr = ast::Expression::CallExpression(ast::CallExpression::boxed(
            SPAN,
            finalized_importee_wrapper_ref,
            NONE,
            oxc::allocator::Vec::new_in(&self.ast_builder),
            false,
            &self.ast_builder,
          ));

          // (init_xxx(), namespace_exports)
          Expression::new_seq_in_parens(wrapper_call_expr, finalized_namespace, &self.ast_builder)
        }
        WrapKind::None => {
          // Order-wrapped dynamic entries never reach this rewrite: their eliminated facades
          // are restored in `restore_order_wrap_entry_facades`.
          debug_assert!(
            self
              .ctx
              .order_wrap_state
              .esm_init_target(importee_idx, &self.ctx.linking_infos[importee_idx])
              .is_none()
          );
          let (finalized_expr, _) =
            self.finalized_expr_for_symbol_ref(importee.namespace_object_ref, false, false);
          finalized_expr
        }
      };

      Some(Expression::new_promise_resolve_then(finalized_expr, &self.ast_builder))
    } else {
      // If the dynamic entry point is merged into another common chunk, we should
      // convert `import('./some-module.js')` to `import('./some-module.js').then(n => n.ns)`
      //                                                                                 ^^ points to the dynamic entry module namespace
      // For CJS format, convert to `Promise.resolve().then(() => require('./some-module.js')).then(n => n.ns)`
      // For CJS modules with wrap_kind Cjs, convert to `import('./chunk.js').then(n => __toESM(n.require_xxx()))`
      // For ESM modules with wrap_kind Esm, convert to `import('./chunk.js').then(n => (n.init_xxx(), n.namespace))`
      let importee_meta = &self.ctx.linking_infos[importee_idx];
      let wrap_kind = importee_meta.wrap_kind();

      // For wrapped modules (CJS/ESM), look up the wrapper_ref; for others, look up the namespace symbol
      let primary_export_symbol = match wrap_kind {
        WrapKind::Cjs | WrapKind::Esm => importee_meta.wrapper_ref,
        WrapKind::None => self.ctx.modules[importee_idx].namespace_object_ref(),
      };

      let primary_export_name = primary_export_symbol.and_then(|sym| {
        importee_chunk.exports_to_other_chunks.get(&sym).and_then(|names| names.first())
      });

      // For ESM wrapped modules, we also need the namespace symbol
      let namespace_export_name = if matches!(wrap_kind, WrapKind::Esm) {
        self.ctx.modules[importee_idx].namespace_object_ref().and_then(|ns_ref| {
          importee_chunk.exports_to_other_chunks.get(&ns_ref).and_then(|names| names.first())
        })
      } else {
        None
      };

      match primary_export_name {
        Some(name) => {
          let base_expr = if matches!(self.ctx.options.format, OutputFormat::Cjs) {
            let import_path = self.ctx.chunk.import_path_for(importee_chunk);
            // require('./some-module.js')
            let require_call_expr = ast::Expression::CallExpression(ast::CallExpression::boxed(
              SPAN,
              ast::Expression::new_identifier(SPAN, "require", &self.ast_builder),
              NONE,
              oxc::allocator::Vec::from_value_in(
                ast::Argument::StringLiteral(ast::StringLiteral::boxed(
                  expr.span,
                  oxc::ast::ast::Str::from_str_in(&import_path, &self.ast_builder),
                  None,
                  &self.ast_builder,
                )),
                &self.ast_builder,
              ),
              false,
              &self.ast_builder,
            ));
            // Promise.resolve().then(() => require('./some-module.js'))
            Expression::new_promise_resolve_then(require_call_expr, &self.ast_builder)
          } else {
            let import_expr = expr.take_in_box(&self.ast_builder.allocator());
            Expression::ImportExpression(import_expr)
          };

          match wrap_kind {
            WrapKind::Cjs => {
              // For CJS modules: import('./chunk.js').then(n => __toESM(n.require_xxx()))
              let finalized_to_esm = self.finalized_expr_for_runtime_symbol("__toESM");
              let call_expr = CallExpression::new_then_call_cjs_wrapper_with_to_esm(
                base_expr,
                name,
                finalized_to_esm,
                self.ctx.module.should_consider_node_esm_spec_for_dynamic_import(),
                &self.ast_builder,
              );
              Some(Expression::CallExpression(call_expr))
            }
            WrapKind::Esm => {
              // For ESM modules: import('./chunk.js').then(n => (n.init_xxx(), n.namespace))
              if let Some(ns_name) = namespace_export_name {
                let call_expr = CallExpression::new_then_call_esm_wrapper_with_namespace(
                  base_expr,
                  name,
                  ns_name,
                  &self.ast_builder,
                );
                Some(Expression::CallExpression(call_expr))
              } else {
                tracing::warn!(
                  "ESM wrapped module {:?} in chunk {:?} has wrapper but no namespace export.",
                  importee_idx,
                  importee_chunk_idx
                );
                None
              }
            }
            WrapKind::None => {
              debug_assert!(
                self
                  .ctx
                  .order_wrap_state
                  .esm_init_target(importee_idx, &self.ctx.linking_infos[importee_idx])
                  .is_none()
              );
              let call_expr =
                CallExpression::new_then_extract_property(base_expr, name, &self.ast_builder);
              Some(Expression::CallExpression(call_expr))
            }
          }
        }
        None => {
          tracing::warn!(
            "Merged dynamic entry module {:?} in chunk {:?} has no export name in exports_to_other_chunks. \
            This indicates an inconsistent state in the chunk graph where the module is marked as merged \
            but its namespace export is not properly tracked.",
            importee_idx,
            importee_chunk_idx
          );
          None
        }
      }
    }
  }

  #[expect(clippy::too_many_lines)]
  fn try_rewrite_import_expression(&self, node: &mut ast::Expression<'ast>) -> bool {
    let ast::Expression::ImportExpression(expr) = node else {
      return false;
    };
    if expr.options.is_some() {
      return false;
    }

    let (Some(str), Some(rec_idx)) =
      (expr.source.as_static_module_request(), self.ctx.module.imports.get(&expr.node_id()))
    else {
      if matches!(self.ctx.options.format, OutputFormat::Cjs)
        && !self.ctx.options.dynamic_import_in_cjs
      {
        // Transform `import(expr)` to `Promise.resolve().then(() => __toESM(require(expr)))`
        let to_esm_fn_name = self.finalized_expr_for_runtime_symbol("__toESM");
        node.replace_with(|old| {
          let ast::Expression::ImportExpression(import_expr) = old else { unreachable!() };
          let require_call = Expression::new_call_with_arg(
            ast::Expression::new_identifier(SPAN, "require", &self.ast_builder),
            import_expr.unbox().source,
            false,
            &self.ast_builder,
          );
          let wrapped = Expression::new_to_esm_wrapper(
            to_esm_fn_name,
            require_call,
            self.ctx.module.should_consider_node_esm_spec_for_dynamic_import(),
            &self.ast_builder,
          );
          Expression::new_promise_resolve_then(wrapped, &self.ast_builder)
        });
        return true;
      }
      return false;
    };

    let mut needs_to_esm_helper = false;
    let rec = &self.ctx.module.import_records[*rec_idx];
    let Some(importee_idx) = rec.resolved_module else { return true };

    match &self.ctx.modules[importee_idx] {
      Module::Normal(importee) => {
        let Some(&importee_chunk_idx) =
          self.ctx.chunk_graph.entry_module_to_entry_chunk.get(&importee_idx)
        else {
          // TODO: probably we should add the reason why it is replaced with `void 0`(just like webpack) when upstream support codegen with specific operation
          *node = ast::Expression::new_void_0(SPAN, &self.ast_builder);
          return true;
        };
        let Some(importee_chunk) = self.ctx.chunk_graph.chunk_table.get(importee_chunk_idx) else {
          return false;
        };

        let import_path = self.ctx.chunk.import_path_for(importee_chunk);
        expr.source = Expression::StringLiteral(ast::StringLiteral::boxed(
          expr.source.span(),
          oxc::ast::ast::Str::from_str_in(&import_path, &self.ast_builder),
          None,
          &self.ast_builder,
        ));

        if let Some(rewritten_expr) = self.rewrite_dynamic_import_for_merged_entry(
          expr,
          importee,
          importee_chunk,
          importee_chunk_idx,
        ) {
          // the `toESM` is properly handled inside `rewrite_dynamic_import_for_merged_entry`
          *node = rewritten_expr;
          return true;
        }

        // Convert `import('./foo.mjs')` to `Promise.resolve().then(function() { return require('foo.mjs') })`
        // when format is CJS
        if matches!(self.ctx.options.format, OutputFormat::Cjs) {
          // require('foo.mjs')
          let mut require_call_expr = ast::Expression::CallExpression(ast::CallExpression::boxed(
            SPAN,
            ast::Expression::new_identifier(SPAN, "require", &self.ast_builder),
            NONE,
            oxc::allocator::Vec::from_value_in(
              ast::Argument::StringLiteral(ast::StringLiteral::boxed(
                expr.span,
                oxc::ast::ast::Str::from_str_in(&import_path, &self.ast_builder),
                None,
                &self.ast_builder,
              )),
              &self.ast_builder,
            ),
            false,
            &self.ast_builder,
          ));

          if importee.exports_kind.is_commonjs() {
            // Inline __toDynamicImportESM: __toESM(require('foo.mjs').default, isNodeMode)
            let to_esm_fn_name = self.finalized_expr_for_runtime_symbol("__toESM");

            // require('foo.mjs').default
            let require_default_expr =
              ast::Expression::StaticMemberExpression(ast::StaticMemberExpression::boxed(
                SPAN,
                require_call_expr,
                ast::IdentifierName::new(SPAN, "default", &self.ast_builder),
                false,
                &self.ast_builder,
              ));

            // __toESM(require('foo.mjs').default, isNodeMode)
            require_call_expr = Expression::new_to_esm_wrapper(
              to_esm_fn_name,
              require_default_expr,
              self.ctx.module.should_consider_node_esm_spec_for_dynamic_import(),
              &self.ast_builder,
            );
          }

          *node = Expression::new_promise_resolve_then(require_call_expr, &self.ast_builder);
          return true;
        }

        needs_to_esm_helper = importee.exports_kind.is_commonjs();
      }
      Module::External(importee) => {
        let import_path = importee.get_import_path(self.ctx.chunk, self.ctx.resolved_paths);
        if str != import_path {
          expr.source = Expression::StringLiteral(ast::StringLiteral::boxed(
            expr.source.span(),
            oxc::ast::ast::Str::from_str_in(&import_path, &self.ast_builder),
            None,
            &self.ast_builder,
          ));
        }
        // Convert `import("external")` to `Promise.resolve().then(() => __toESM(require("external")))`
        // when format is CJS and dynamicImportInCjs is false
        if matches!(self.ctx.options.format, OutputFormat::Cjs)
          && !self.ctx.options.dynamic_import_in_cjs
        {
          let to_esm_fn_name = self.finalized_expr_for_runtime_symbol("__toESM");
          node.replace_with(|old| {
            let ast::Expression::ImportExpression(import_expr) = old else { unreachable!() };
            let require_call = Expression::new_call_with_arg(
              ast::Expression::new_identifier(SPAN, "require", &self.ast_builder),
              import_expr.unbox().source,
              false,
              &self.ast_builder,
            );
            let wrapped = Expression::new_to_esm_wrapper(
              to_esm_fn_name,
              require_call,
              self.ctx.module.should_consider_node_esm_spec_for_dynamic_import(),
              &self.ast_builder,
            );
            Expression::new_promise_resolve_then(wrapped, &self.ast_builder)
          });
          return true;
        }
      }
    }

    if needs_to_esm_helper {
      // Turn `import('./some-cjs-module.js')` into `import('./some-cjs-module.js').then((m) => __toESM(m.default, isNodeMode))`
      // Inline __toDynamicImportESM

      // __toESM
      let to_esm_fn_name = self.finalized_expr_for_runtime_symbol("__toESM");

      // `import('./some-cjs-module.js')`
      node.replace_with(|original_import_expr| {
        // Build arrow function: (m) => __toESM(m.default, isNodeMode)
        // m.default
        let m_default_expr =
          ast::Expression::StaticMemberExpression(ast::StaticMemberExpression::boxed(
            SPAN,
            ast::Expression::new_identifier(SPAN, "m", &self.ast_builder),
            ast::IdentifierName::new(SPAN, "default", &self.ast_builder),
            false,
            &self.ast_builder,
          ));

        // __toESM(m.default, isNodeMode)
        let to_esm_call = Expression::new_to_esm_wrapper(
          to_esm_fn_name,
          m_default_expr,
          self.ctx.module.should_consider_node_esm_spec_for_dynamic_import(),
          &self.ast_builder,
        );

        // (m) => __toESM(m.default, isNodeMode)
        let arrow_fn = ast::ArrowFunctionExpression::boxed(
          SPAN,
          true,  // expression
          false, // async
          NONE,
          ast::FormalParameters::new(
            SPAN,
            ast::FormalParameterKind::ArrowFormalParameters,
            oxc::allocator::Vec::from_value_in(
              ast::FormalParameter::new(
                SPAN,
                oxc::allocator::Vec::new_in(&self.ast_builder),
                ast::BindingPattern::new_binding_identifier(
                  SPAN,
                  oxc::ast::ast::Str::from_str_in("m", &self.ast_builder),
                  &self.ast_builder,
                ),
                NONE,
                NONE,
                false,
                None,
                false,
                false,
                &self.ast_builder,
              ),
              &self.ast_builder,
            ),
            NONE,
            &self.ast_builder,
          ),
          NONE,
          ast::FunctionBody::new(
            SPAN,
            oxc::allocator::Vec::new_in(&self.ast_builder),
            oxc::allocator::Vec::from_value_in(
              ast::Statement::new_expression_statement(SPAN, to_esm_call, &self.ast_builder),
              &self.ast_builder,
            ),
            &self.ast_builder,
          ),
          &self.ast_builder,
        );

        // `import('./some-cjs-module.js').then
        let callee = ast::StaticMemberExpression::boxed(
          SPAN,
          original_import_expr,
          ast::IdentifierName::new(SPAN, "then", &self.ast_builder),
          false,
          &self.ast_builder,
        );

        // `import('./some-cjs-module.js').then((m) => __toESM(m.default, isNodeMode))`
        let call_expr = ast::CallExpression::boxed(
          SPAN,
          ast::Expression::StaticMemberExpression(callee),
          NONE,
          oxc::allocator::Vec::from_value_in(
            ast::Argument::ArrowFunctionExpression(arrow_fn),
            &self.ast_builder,
          ),
          false,
          &self.ast_builder,
        );

        ast::Expression::CallExpression(call_expr)
      });
    }

    true
  }

  /// if the json module prop needs to inline, we would just rewrite the inlined prop to
  /// `EmptyStatement`
  fn try_inline_json_module_prop(&mut self, it: &mut Statement<'ast>) -> Option<()> {
    let json_module_inlined_prop = self.json_module_inlined_prop.as_mut()?;
    let decl = it.as_declaration_mut()?;
    let ast::Declaration::VariableDeclaration(var_decl) = decl else {
      return None;
    };
    let first_decl = var_decl.declarations.first_mut()?;
    // Bail out early if there's no initializer; both arms below rely on it being `Some`.
    first_decl.init.as_ref()?;
    // For synthesis json module, only last symbol of var stmt is `None`, since it is a generated
    // manually.
    let id = first_decl.id.get_binding_identifier()?.symbol_id.get();
    match id {
      Some(id) => {
        let symbol_ref: SymbolRef = (self.ctx.idx, id).into();
        if !self
          .ctx
          .module
          .json_module_none_self_reference_included_symbol
          .as_ref()?
          .contains(&symbol_ref)
        {
          json_module_inlined_prop.insert(id, first_decl.init.take()?);
          *it = ast::Statement::new_empty_statement(SPAN, &self.ast_builder);
        }
      }
      None => {
        // It is `json export default stmt`. e.g.
        // ```js
        // ...snip
        // export default { foo, bar };
        // ```
        //
        let Some(Expression::ObjectExpression(obj_expr)) = first_decl.init.as_mut() else {
          return None;
        };

        for ele in &mut obj_expr.properties {
          let ObjectPropertyKind::ObjectProperty(prop) = ele else {
            continue;
          };
          let Some(identifier) = prop.value.as_identifier() else {
            continue;
          };
          let reference_id = identifier.reference_id();
          let Some(symbol_id) = self.scope.symbol_id_for(reference_id) else {
            continue;
          };
          let Some(replaced_expr) = json_module_inlined_prop.remove(&symbol_id) else {
            continue;
          };
          prop.value = replaced_expr;
        }
      }
    }

    Some(())
  }
}

struct ResolveFileUrlHookResultSpanRewriter(Span);

impl oxc::ast_visit::VisitMut<'_> for ResolveFileUrlHookResultSpanRewriter {
  fn visit_span(&mut self, span: &mut Span) {
    *span = self.0;
  }
}
