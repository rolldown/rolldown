mod cjs_export_analyzer;
pub mod const_eval;
pub mod dynamic_import;
mod hmr;
pub mod impl_visit;
mod import_analyzer;
mod independent_init;
mod new_url;
pub mod stmt_eval_analyzer;
mod top_level_import_read;

use arcstr::ArcStr;
use const_eval::{ConstEvalCtx, eval_can_assign, try_extract_const_literal};
use oxc::ast::ast::{BindingPattern, Expression, ImportExpression};
use oxc::ast::{AstKind, ast};
use oxc::semantic::{NodeId, Reference, ScopeFlags, Scoping};
use oxc::span::SPAN;
use oxc::{
  ast::{
    Comment,
    ast::{
      ExportAllDeclaration, ExportDeclaration, ExportDefaultDeclaration, ExportFromDeclaration,
      ExportNamedDeclaration, IdentifierReference, ImportDeclaration, ModuleDeclaration, Program,
    },
  },
  ast_visit::{VisitJs, walk_js},
  semantic::SymbolId,
  span::{GetSpan, Span},
};
use oxc_index::IndexVec;
use oxc_str::{CompactStr, JSStr};
use rolldown_common::dynamic_import_usage::{DynamicImportExportsUsage, DynamicImportUsageInfo};
use rolldown_common::{
  ConstExportMeta, ConstantValue, DynamicImportExprInfo, EcmaModuleAstUsage, EcmaViewMeta,
  ExportsKind, FlatOptions, HmrInfo, ImportAttribute, ImportKind, ImportRecordIdx,
  ImportRecordMeta, LocalExport, MemberExprProp, MemberExprRef, ModuleDefFormat, ModuleId,
  ModuleIdx, NamedImport, RawImportRecord, RolldownFileUrlReference, Specifier, StmtEvalFlags,
  StmtInfo, StmtInfoIdx, StmtInfoMeta, StmtInfos, SymbolRef, SymbolRefDbForModule, SymbolRefFlags,
  TaggedSymbolRef, ThisExprReplaceKind, generate_replace_this_expr_map,
};
use rolldown_ecmascript_utils::FunctionExt;
use rolldown_error::{BuildDiagnostic, BuildResult, CjsExportSpan};
use rolldown_std_utils::PathExt;
use rolldown_utils::concat_string;
use rolldown_utils::ecmascript::{legitimize_identifier_name, to_usv_str};
use rolldown_utils::indexmap::FxIndexMap;
use rustc_hash::{FxHashMap, FxHashSet};
use std::borrow::Cow;
use sugar_path::SugarPath;

use bitflags::bitflags;

use crate::SharedOptions;
use crate::ast_scanner::cjs_export_analyzer::{
  CommonJsAstType, CommonjsExportSymbolUsage, is_object_define_property_es_module,
};

bitflags! {
  #[derive(Debug, Clone, Copy, Default)]
  /// Tracks untranspiled syntax encountered during scanning.
  pub(crate) struct UntranspiledSyntax: u8 {
    const Jsx = 1 << 0;
  }
}

#[derive(Debug)]
pub struct ScanResult {
  /// Using `IndexMap` to make sure the order of the named imports always sorted by the span of the
  /// module
  pub named_imports: FxIndexMap<SymbolRef, NamedImport>,
  pub named_exports: FxHashMap<CompactStr, LocalExport>,
  /// Used to store all exports in commonjs module, why not reuse `named_exports`?
  /// Because It is legal to use commonjs exports syntax in a es module, here is an example:
  /// ```js
  /// export const foo = 1;
  /// exports.foo = 20;
  /// ```
  /// Although the `exports.foo` will just be treated as a global variable assign, if we reused
  /// `named_exports`, the `foo` will be overridden by the `exports.foo` and cause a bug(Because we
  /// may not know if it is a esm at the time, a simple case would be swap the order of two export
  /// stmt).
  pub commonjs_exports: FxHashMap<CompactStr, Vec<LocalExport>>,
  pub stmt_infos: StmtInfos,
  pub import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub default_export_ref: SymbolRef,
  /// Represents [Module Namespace Object](https://tc39.es/ecma262/#sec-module-namespace-exotic-objects)
  pub namespace_object_ref: SymbolRef,
  pub imports: FxHashMap<NodeId, ImportRecordIdx>,
  pub dummy_record_set: FxHashSet<NodeId>,
  pub exports_kind: ExportsKind,
  pub warnings: Vec<BuildDiagnostic>,
  pub errors: Vec<BuildDiagnostic>,
  pub ecma_view_meta: EcmaViewMeta,
  /// Whether the module is a commonjs module
  /// The reason why we can't reuse `cjs_exports_ident` and `cjs_module_ident` is that
  /// any `module` or `exports` in the top-level scope should be treated as a commonjs module.
  /// `cjs_exports_ident` and `cjs_module_ident` only only recorded when they are appear in
  /// lhs of AssignmentExpression
  pub ast_usage: EcmaModuleAstUsage,
  /// The span of the first top-level `await` keyword, if any. Routed into
  /// a centralized map on the link stage instead of being stored on every
  /// `EcmaView`, since top-level await is rare.
  pub tla_keyword_span: Option<Span>,
  pub symbol_ref_db: SymbolRefDbForModule,
  /// https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/js_parser/js_parser_lower_class.go#L2277-L2283
  /// used for check if current class decl symbol was referenced in its class scope
  /// We needs to record the info in ast scanner since after that the ast maybe touched, etc
  /// (naming deconflict)
  pub self_referenced_class_decl_symbol_ids: FxHashSet<SymbolId>,
  /// hashbang only works if it's literally the first character.So we need to generate it in chunk
  /// level rather than module level, or a syntax error will be raised if there are multi modules
  /// has hashbang. Storing the span of hashbang used for hashbang codegen in chunk level
  pub hashbang_range: Option<Span>,
  /// we don't know the ImportRecord related ModuleIdx yet, so use ImportRecordIdx as key
  /// temporarily
  pub dynamic_import_rec_exports_usage: FxHashMap<ImportRecordIdx, DynamicImportExportsUsage>,
  /// See `EcmaView::dynamic_import_called_names`.
  pub dynamic_import_called_names: FxHashMap<ImportRecordIdx, FxHashSet<CompactStr>>,
  /// `new URL('...', import.meta.url)`
  pub new_url_references: FxHashMap<NodeId, ImportRecordIdx>,
  /// See `EcmaView::namespace_in_checks`.
  pub namespace_in_checks: FxHashMap<NodeId, CompactStr>,
  /// `import.meta.ROLLDOWN_FILE_URL_<referenceId>[_<urlId>]`, one entry per occurrence.
  pub rolldown_file_url_references: Vec<RolldownFileUrlReference>,
  pub this_expr_replace_map: FxHashMap<NodeId, ThisExprReplaceKind>,
  /// See `EcmaView::cjs_this_ref`.
  pub cjs_this_ref: Option<SymbolRef>,
  pub hmr_info: HmrInfo,
  pub hmr_hot_ref: Option<SymbolRef>,
  pub directive_range: Vec<Span>,
  pub constant_export_map: FxHashMap<SymbolId, ConstExportMeta>,
  pub import_attribute_map: FxHashMap<ImportRecordIdx, ImportAttribute>,
  /// Temporary storage for node IDs of `require()` calls in `module.exports = require(...)` patterns.
  /// Resolved to `cjs_reexport_import_record_ids` after scanning completes.
  pub cjs_reexport_require_node_ids: Vec<NodeId>,
  /// Import record indices for `module.exports = require(...)` patterns.
  pub cjs_reexport_import_record_ids: Vec<ImportRecordIdx>,
}

pub struct AstScannerImmutableCtx<'me, 'ast> {
  idx: ModuleIdx,
  source: &'me ArcStr,
  module_type: ModuleDefFormat,
  id: &'me ModuleId,
  comments: &'me oxc::allocator::Vec<'me, Comment>,
  options: &'me SharedOptions,
  flat_options: FlatOptions,
  allocator: &'ast oxc::allocator::Allocator,
}

pub struct AstScanner<'me, 'ast> {
  immutable_ctx: AstScannerImmutableCtx<'me, 'ast>,
  current_stmt_info: StmtInfo,
  current_stmt_idx: StmtInfoIdx,
  result: ScanResult,
  esm_export_keyword: Option<Span>,
  esm_import_keyword: Option<Span>,
  /// cjs ident span used for emit `commonjs_variable_in_esm` warning
  cjs_exports_ident: Option<Span>,
  cjs_module_ident: Option<Span>,
  cur_class_decl: Option<SymbolId>,
  visit_path: Vec<AstKind<'ast>>,
  scope_stack: Vec<ScopeFlags>,
  /// Reused scratch buffer for building facade namespace symbol names in `add_import_record`,
  /// so each import record doesn't allocate a fresh `String` just to hand a `&str` to oxc.
  namespace_name_buf: String,
  dynamic_import_usage_info: DynamicImportUsageInfo,
  /// Node IDs of top-level `this` expressions.
  top_level_this_expr_set: FxHashSet<NodeId>,
  /// A flag to resolve `this` appear with propertyKey in class
  is_nested_this_inside_class: bool,
  /// The last top-level statement that read `this` inside a function or class.
  last_nested_this_stmt_idx: Option<StmtInfoIdx>,
  /// Used in commonjs module it self
  cjs_named_exports_usage: FxHashMap<CompactStr, CommonjsExportSymbolUsage>,
  /// Set when `module.exports = <value>` is detected. All prior `exports.xxx`
  /// constants are stale since the entire exports object is replaced at runtime.
  has_module_exports_reassignment: bool,
  /// Whether the module uses its exports object as `note_commonjs_exports_use` doesn't allow.
  commonjs_exports_escape: bool,
  /// How many times `exports.name = ...` or `module.exports.name = ...` defines each name.
  commonjs_export_definitions: FxHashMap<CompactStr, u32>,
  /// Whether the current position is at the top level (all parent scopes are block or top-level).
  /// A cache of [AstScanner::is_valid_tla_scope].
  is_top_level: bool,
  current_comment_idx: usize,
  untranspiled_syntax: UntranspiledSyntax,
  /// Symbol IDs of namespace imports (`import * as ns from '...'`).
  /// Used to treat property reads on namespace objects as side-effect-free.
  namespace_object_symbol_ids: FxHashSet<SymbolId>,
}

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  #[expect(clippy::too_many_arguments)]
  pub fn new(
    idx: ModuleIdx,
    scoping: Scoping,
    repr_name: &'me str,
    module_type: ModuleDefFormat,
    source: &'me ArcStr,
    file_path: &'me ModuleId,
    comments: &'me oxc::allocator::Vec<'me, Comment>,
    options: &'me SharedOptions,
    allocator: &'ast oxc::allocator::Allocator,
    flat_options: FlatOptions,
  ) -> Self {
    let root_scope_id = scoping.root_scope_id();
    let mut symbol_ref_db = SymbolRefDbForModule::new(scoping, idx, root_scope_id);
    // This is used for converting "export default foo;" => "var default_symbol = foo;"
    let legitimized_repr_name = legitimize_identifier_name(repr_name);
    let default_export_ref = symbol_ref_db
      .create_facade_root_symbol_ref(&concat_string!(legitimized_repr_name, "_default"));

    let name = concat_string!(legitimized_repr_name, "_exports");
    let namespace_object_ref = symbol_ref_db.create_facade_root_symbol_ref(&name);

    let hmr_hot_ref = options.experimental.dev_mode.as_ref().map(|_| {
      symbol_ref_db.create_facade_root_symbol_ref(&concat_string!(legitimized_repr_name, "_hot"))
    });

    let result = ScanResult {
      named_imports: FxIndexMap::default(),
      named_exports: FxHashMap::default(),
      stmt_infos: StmtInfos::new(),
      import_records: IndexVec::new(),
      default_export_ref,
      namespace_object_ref,
      imports: FxHashMap::default(),
      exports_kind: ExportsKind::None,
      warnings: Vec::new(),
      errors: Vec::new(),
      ast_usage: EcmaModuleAstUsage::empty()
        .union(EcmaModuleAstUsage::AllStaticExportPropertyAccess),
      tla_keyword_span: None,
      symbol_ref_db,
      self_referenced_class_decl_symbol_ids: FxHashSet::default(),
      hashbang_range: None,
      dynamic_import_rec_exports_usage: FxHashMap::default(),
      dynamic_import_called_names: FxHashMap::default(),
      new_url_references: FxHashMap::default(),
      namespace_in_checks: FxHashMap::default(),
      rolldown_file_url_references: Vec::new(),
      this_expr_replace_map: FxHashMap::default(),
      cjs_this_ref: None,
      hmr_info: HmrInfo::default(),
      hmr_hot_ref,
      directive_range: vec![],
      dummy_record_set: FxHashSet::default(),
      commonjs_exports: FxHashMap::default(),
      constant_export_map: FxHashMap::default(),
      ecma_view_meta: EcmaViewMeta::default(),
      import_attribute_map: FxHashMap::default(),
      cjs_reexport_require_node_ids: Vec::new(),
      cjs_reexport_import_record_ids: Vec::new(),
    };

    Self {
      immutable_ctx: AstScannerImmutableCtx {
        allocator,
        idx,
        source,
        module_type,
        id: file_path,
        comments,
        options,
        flat_options,
      },
      current_stmt_info: StmtInfo::default(),
      current_stmt_idx: StmtInfos::NAMESPACE_STMT_IDX,
      result,
      esm_export_keyword: None,
      esm_import_keyword: None,
      cjs_module_ident: None,
      cjs_exports_ident: None,
      cur_class_decl: None,
      visit_path: vec![],
      scope_stack: vec![],
      namespace_name_buf: String::new(),
      dynamic_import_usage_info: DynamicImportUsageInfo::default(),
      top_level_this_expr_set: FxHashSet::default(),
      is_nested_this_inside_class: false,
      last_nested_this_stmt_idx: None,
      cjs_named_exports_usage: FxHashMap::from_iter([(
        "__esModule".into(),
        CommonjsExportSymbolUsage {
          read: 0,
          write: 0,
          bailout: true,
        },
      )]),
      has_module_exports_reassignment: false,
      commonjs_exports_escape: false,
      commonjs_export_definitions: FxHashMap::default(),
      is_top_level: false,
      current_comment_idx: 0,
      untranspiled_syntax: UntranspiledSyntax::empty(),
      namespace_object_symbol_ids: FxHashSet::default(),
    }
  }

  /// including such scenario:
  /// ```js
  /// class T {
  ///   [await foo]() {}
  /// }
  ///
  /// await foo;
  /// {
  ///   await foo;
  /// }
  /// for await (const let value of list) {
  /// }
  /// ```
  pub fn is_valid_tla_scope(&self) -> bool {
    self
      .scope_stack
      .iter()
      .rev()
      .all(|flag| flag.is_top() || flag.is_block() || flag.is_catch_clause())
  }

  pub fn is_root_scope(&self) -> bool {
    self.scope_stack.iter().rev().all(|flag| flag.is_top())
  }

  pub fn scan(mut self, program: &Program<'ast>) -> BuildResult<ScanResult> {
    self.visit_program(program);
    let mut exports_kind = ExportsKind::None;

    if let Some(esm_export_keyword) = self.esm_export_keyword {
      exports_kind = ExportsKind::Esm;
      if let Some(start) = self.cjs_module_ident {
        self.result.warnings.push(
          BuildDiagnostic::commonjs_variable_in_esm(
            self.immutable_ctx.id.to_string(),
            self.immutable_ctx.source.clone(),
            esm_export_keyword,
            CjsExportSpan::Module(start),
          )
          .with_severity_warning(),
        );
      }
      if let Some(start) = self.cjs_exports_ident {
        self.result.warnings.push(
          BuildDiagnostic::commonjs_variable_in_esm(
            self.immutable_ctx.id.to_string(),
            self.immutable_ctx.source.clone(),
            esm_export_keyword,
            CjsExportSpan::Exports(start),
          )
          .with_severity_warning(),
        );
      }
    } else if self
      .result
      .ast_usage
      .intersects(EcmaModuleAstUsage::ModuleOrExports)
      || self
        .result
        .ast_usage
        .contains(EcmaModuleAstUsage::TopLevelReturn)
    {
      exports_kind = ExportsKind::CommonJs;
    } else {
      // TODO(hyf0): Should add warnings if the module type doesn't satisfy the exports kind.
      match self.immutable_ctx.module_type {
        ModuleDefFormat::Cjs | ModuleDefFormat::CjsPackageJson | ModuleDefFormat::Cts => {
          exports_kind = ExportsKind::CommonJs;
        }
        ModuleDefFormat::EsmMjs | ModuleDefFormat::EsmPackageJson | ModuleDefFormat::EsmMts => {
          exports_kind = ExportsKind::Esm;
        }
        ModuleDefFormat::Unknown => {
          if self.esm_import_keyword.is_some() {
            exports_kind = ExportsKind::Esm;
          }
        }
      }
    }
    // Filtering out all facade export like `exports.test = 1`,
    // It is legal to use commonjs exports syntax in a es module, although
    // they will be treated as normal global variable assign.
    if matches!(exports_kind, ExportsKind::Esm) {
      self.result.commonjs_exports.clear();
    }

    // https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/js_parser/js_parser.go#L12551-L12604
    // Since AstScan is immutable, we defer transformation in module finalizer
    if !self.top_level_this_expr_set.is_empty() {
      self.result.this_expr_replace_map = generate_replace_this_expr_map(
        &self.top_level_this_expr_set,
        if exports_kind.is_commonjs() {
          // Top-level `this` is the module's exports object, an implicit `exports` reference.
          // See internal-docs/linking/determine-module-exports-kind/implementation.md.
          self.result.ast_usage.insert(EcmaModuleAstUsage::ExportsRef);
          self.result.cjs_this_ref =
            Some(self.result.symbol_ref_db.create_facade_root_symbol_ref("this"));
          ThisExprReplaceKind::Exports
        } else {
          ThisExprReplaceKind::Context
        },
      );
    }

    self.result.exports_kind = exports_kind;

    // Resolve CJS re-export require node IDs to import record indices.
    self.result.cjs_reexport_import_record_ids = self
      .result
      .cjs_reexport_require_node_ids
      .iter()
      .filter_map(|node_id| self.result.imports.get(node_id).copied())
      .collect();

    // If some commonjs module facade exports was used locally, we need to explicitly mark them as
    // has side effects, so that they should not be removed in linking stage.
    let mut bailout_inlined_cjs_exports_symbol_ids = FxHashSet::default();
    for (name, usage) in &self.cjs_named_exports_usage {
      if let Some(resolved_list) = self.result.commonjs_exports.get(name) {
        for resolved in resolved_list {
          if !usage.can_be_removed() {
            let stmt_info_idx_list = self
              .result
              .stmt_infos
              .declared_stmts_by_symbol(&resolved.referenced)
              .to_vec();
            for idx in stmt_info_idx_list {
              self.result.stmt_infos[idx].eval_flags |= StmtEvalFlags::UnknownSideEffect;
            }
          }
          if !usage.can_be_inlined() {
            bailout_inlined_cjs_exports_symbol_ids.insert(resolved.referenced.symbol);
          }
        }
      }
    }

    // For cjs module with hmr enabled, bundler will generates code that references `module`.
    // https://github.com/rolldown/rolldown/issues/4129
    //
    // Even if a cjs module doesn't export anything, the correct `module` reference in module scope
    // still needs to be registered with the HMR runtime.
    //
    // Following cases need to be registered:
    // - `ExportsKind::None` (no exports)
    // - `ExportsKind::CommonJs` (commonjs module)
    if self.immutable_ctx.options.is_dev_mode_enabled()
      && matches!(exports_kind, ExportsKind::None | ExportsKind::CommonJs)
    {
      self.result.ast_usage.insert(EcmaModuleAstUsage::ModuleRef);
    }

    // `module.exports = <value>` replaces the entire exports object, so all
    // prior `exports.xxx` writes are stale and must not be inlined.
    // So does an exports object that escapes, or that a direct `eval` or `with` can reach, and a
    // top-level `return`, after which a definition may not run, while an export defined more than
    // once keeps only its own constant out (#693).
    if self.has_module_exports_reassignment
      || self.commonjs_exports_escape
      || self.result.ecma_view_meta.contains(EcmaViewMeta::Eval)
      || self.result.ast_usage.contains(EcmaModuleAstUsage::TopLevelReturn)
    {
      for exports in self.result.commonjs_exports.values() {
        for local_export in exports {
          bailout_inlined_cjs_exports_symbol_ids.insert(local_export.referenced.symbol);
        }
      }
    }
    for (name, definitions) in &self.commonjs_export_definitions {
      if *definitions > 1
        && let Some(exports) = self.result.commonjs_exports.get(name)
      {
        for local_export in exports {
          bailout_inlined_cjs_exports_symbol_ids.insert(local_export.referenced.symbol);
        }
      }
    }
    self
      .result
      .constant_export_map
      .retain(|symbol_id, constant_meta| {
        (constant_meta.commonjs_export
          && !bailout_inlined_cjs_exports_symbol_ids.contains(symbol_id))
          || self
            .result
            .symbol_ref_db
            .flags
            .get(symbol_id)
            .is_some_and(|flag| flag.contains(SymbolRefFlags::IsNotReassigned))
      });

    if cfg!(debug_assertions) {
      use rustc_hash::FxHashSet;
      let mut scanned_symbols_in_root_scope = self
        .result
        .stmt_infos
        .iter()
        .flat_map(|stmt_info| {
          stmt_info
            .declared_symbols
            .iter()
            .map(rolldown_common::TaggedSymbolRef::inner)
        })
        .collect::<FxHashSet<_>>();
      for (name, symbol_id) in self
        .result
        .symbol_ref_db
        .scoping()
        .get_bindings(self.result.symbol_ref_db.scoping().root_scope_id())
      {
        let symbol_ref: SymbolRef = (self.immutable_ctx.idx, *symbol_id).into();
        let scope_id = self.result.symbol_ref_db.symbol_scope_id(*symbol_id);
        if !scanned_symbols_in_root_scope.remove(&symbol_ref) {
          Err(anyhow::format_err!(
            "Symbol ({name:?}, {symbol_id:?}, {scope_id:?}) is declared in the top-level scope but doesn't get scanned by the scanner",
          ))?;
        }
      }
    }
    Ok(self.result)
  }

  fn set_esm_export_keyword(&mut self, span: Span) {
    self.esm_export_keyword.get_or_insert(span);
  }

  fn declare_normal_symbol_ref(&mut self, id: SymbolId) {
    self
      .current_stmt_info
      .declared_symbols
      .push(TaggedSymbolRef::normal((self.immutable_ctx.idx, id).into()));
  }

  fn declare_link_only_symbol_ref(&mut self, id: SymbolId) {
    self
      .current_stmt_info
      .declared_symbols
      .push(TaggedSymbolRef::link_only(
        (self.immutable_ctx.idx, id).into(),
      ));
  }

  fn get_root_binding(&self, name: &str) -> Option<SymbolId> {
    self
      .result
      .symbol_ref_db
      .scoping()
      .get_root_binding(name.into())
  }

  /// `is_dummy` means if it the import record is created during ast transformation.
  ///
  /// `import_expression_node_id` - The AST node ID of the ImportExpression node,
  /// required for dynamic imports to track their position for optimization purposes.
  /// Should be `None` for static imports and `Some(node_id)` for dynamic imports.
  fn add_import_record(
    &mut self,
    module_request: &str,
    kind: ImportKind,
    span: Span,
    importer_span: Span,
    init_meta: ImportRecordMeta,
    import_expression_node_id: Option<NodeId>,
  ) -> ImportRecordIdx {
    // If 'foo' in `import ... from 'foo'` is finally a commonjs module, we will convert the import statement
    // to `var import_foo = __toESM(require_foo())`, so we create a symbol for `import_foo` here. Notice that we
    // just create the symbol. If the symbol is finally used would be determined in the linking stage.
    self.namespace_name_buf.clear();
    self.namespace_name_buf.push_str("#LOCAL_NAMESPACE_IN_");
    self
      .namespace_name_buf
      .push_str(itoa::Buffer::new().format(self.current_stmt_idx.raw()));
    self.namespace_name_buf.push('#');
    let namespace_ref: SymbolRef = self
      .result
      .symbol_ref_db
      .create_facade_root_symbol_ref(&self.namespace_name_buf);
    let rec = RawImportRecord::new(
      CompactStr::from(module_request),
      kind,
      namespace_ref,
      span,
      importer_span,
      None,
      // The first index stmt is reserved for the facade statement that constructs Module Namespace
      // Object
      import_expression_node_id.map(|node_id| {
        Box::new(DynamicImportExprInfo {
          stmt_info_idx: self.current_stmt_idx,
          node_id,
        })
      }),
    )
    .with_meta(init_meta);

    let id = self.result.import_records.push(rec);
    self.current_stmt_info.import_records.push(id);
    id
  }

  fn add_named_import(
    &mut self,
    local: SymbolId,
    imported: &str,
    record_idx: ImportRecordIdx,
    span_imported: Span,
  ) {
    self.result.named_imports.insert(
      (self.immutable_ctx.idx, local).into(),
      NamedImport {
        imported: CompactStr::new(imported).into(),
        imported_as: (self.immutable_ctx.idx, local).into(),
        span_imported,
        record_idx,
      },
    );
  }

  fn add_star_import(&mut self, local: SymbolId, record_idx: ImportRecordIdx, span_imported: Span) {
    self.result.named_imports.insert(
      (self.immutable_ctx.idx, local).into(),
      NamedImport {
        imported: Specifier::Star,
        imported_as: (self.immutable_ctx.idx, local).into(),
        record_idx,
        span_imported,
      },
    );
    self.namespace_object_symbol_ids.insert(local);
  }

  fn add_local_export(&mut self, export_name: &str, local: SymbolId, span: Span) {
    let symbol_ref: SymbolRef = (self.immutable_ctx.idx, local).into();

    // If there is any write reference to the local variable, it is reassigned.
    let is_reassigned = self
      .result
      .symbol_ref_db
      .get_resolved_references(local)
      .any(Reference::is_write);

    let ref_flags = symbol_ref.flags_mut(&mut self.result.symbol_ref_db);
    if !is_reassigned {
      ref_flags.insert(SymbolRefFlags::IsNotReassigned);
    }

    self.result.named_exports.insert(
      export_name.into(),
      LocalExport {
        referenced: (self.immutable_ctx.idx, local).into(),
        span,
        came_from_commonjs: false,
      },
    );
  }

  /// Called after visiting a top-level statement that reads no `this` inside a function or class.
  /// See internal-docs/code-splitting/implementation.md.
  fn mark_functions_never_reading_this(&mut self, stmt: &ast::Statement<'ast>) {
    // A direct `eval` can read `this`, or reassign a binding without a write reference.
    if self.result.symbol_ref_db.scoping().root_scope_flags().contains_direct_eval() {
      return;
    }
    let declaration = match stmt {
      ast::Statement::ExportDeclaration(decl) => Some(&decl.declaration),
      ast::Statement::ExportDefaultDeclaration(decl) => {
        match &decl.declaration {
          ast::ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
            if let Some(id) = &func.id {
              self.mark_never_reads_this(id.symbol_id());
            } else {
              self.mark_default_export_never_reads_this();
            }
          }
          ast::ExportDefaultDeclarationKind::FunctionExpression(_)
          | ast::ExportDefaultDeclarationKind::ArrowFunctionExpression(_) => {
            self.mark_default_export_never_reads_this();
          }
          _ => {}
        }
        return;
      }
      _ => stmt.as_declaration(),
    };
    match declaration {
      Some(ast::Declaration::FunctionDeclaration(func)) => {
        if let Some(id) = &func.id {
          self.mark_never_reads_this(id.symbol_id());
        }
      }
      Some(ast::Declaration::VariableDeclaration(var_decl)) => {
        for decl in &var_decl.declarations {
          if let BindingPattern::BindingIdentifier(id) = &decl.id
            && matches!(
              decl.init,
              Some(Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_))
            )
          {
            self.mark_never_reads_this(id.symbol_id());
          }
        }
      }
      _ => {}
    }
  }

  fn mark_never_reads_this(&mut self, symbol_id: SymbolId) {
    let scoping = self.result.symbol_ref_db.scoping();
    // A repeated `var` declaration assigns without a write reference.
    if scoping.symbol_redeclarations(symbol_id).is_empty()
      && !scoping.get_resolved_references(symbol_id).any(Reference::is_write)
    {
      let symbol_ref: SymbolRef = (self.immutable_ctx.idx, symbol_id).into();
      symbol_ref.flags_mut(&mut self.result.symbol_ref_db).insert(SymbolRefFlags::NeverReadsThis);
    }
  }

  /// The binding of an anonymous default export has no name, so no code can reassign it.
  fn mark_default_export_never_reads_this(&mut self) {
    self
      .result
      .default_export_ref
      .flags_mut(&mut self.result.symbol_ref_db)
      .insert(SymbolRefFlags::NeverReadsThis);
  }

  fn add_local_default_export(&mut self, local: SymbolId, span: Span) {
    // The default symbol ref never get reassigned.
    let symbol_ref: SymbolRef = (self.immutable_ctx.idx, local).into();
    symbol_ref
      .flags_mut(&mut self.result.symbol_ref_db)
      .insert(SymbolRefFlags::IsNotReassigned);

    self.result.named_exports.insert(
      "default".into(),
      LocalExport {
        referenced: symbol_ref,
        span,
        came_from_commonjs: false,
      },
    );
  }

  /// Record `export { [imported] as [export_name] } from ...` statement.
  ///
  /// Notice that we will pretend
  /// ```js
  /// export { [imported] as [export_name] } from '...'
  /// ```
  /// to be
  /// ```js
  /// import { [imported] as [generated] } from '...'
  /// export { [generated] as [export_name] }
  /// ```
  /// Reasons are:
  /// - No extra logic for dealing with re-exports concept.
  /// - Cjs compatibility. We need a [generated] binding to holds the value reexport from commonjs. For example
  /// ```js
  /// export { foo } from 'commonjs'
  /// ```
  /// would be converted to
  /// ```js
  /// const import_commonjs = __toESM(require_commonjs())
  /// const [generated] = import_commonjs.foo
  /// export { [generated] as foo }
  /// ```
  /// `export { foo } from 'commonjs'` would be converted to `const import_commonjs = require()` in the linking stage.
  fn add_re_export(
    &mut self,
    export_name: &str,
    imported: &str,
    record_idx: ImportRecordIdx,
    span_imported: Span,
  ) {
    // We will pretend `export { [imported] as [export_name] }` to be `import `
    let ident = if export_name == "default" {
      let importee_repr = self.result.import_records[record_idx]
        .module_request
        .as_path()
        .representative_file_name();
      let importee_repr = legitimize_identifier_name(&importee_repr);
      Cow::Owned(concat_string!(importee_repr, "_default"))
    } else {
      // the export_name could be a string literal
      legitimize_identifier_name(export_name)
    };
    let generated_imported_as_ref = self
      .result
      .symbol_ref_db
      .create_facade_root_symbol_ref(ident.as_ref());
    // Re-exporting a default passes on a CommonJS module's exports object, which the importers of
    // the re-export may change (#693).
    if imported == "default" {
      *generated_imported_as_ref.flags_mut(&mut self.result.symbol_ref_db) |=
        SymbolRefFlags::HasComputedMemberWrite;
    }

    self
      .current_stmt_info
      .declared_symbols
      .push(rolldown_common::TaggedSymbolRef::normal(
        generated_imported_as_ref,
      ));
    let name_import = NamedImport {
      imported: imported.into(),
      imported_as: generated_imported_as_ref,
      record_idx,
      span_imported,
    };
    self.result.named_exports.insert(
      export_name.into(),
      LocalExport {
        referenced: generated_imported_as_ref,
        span: name_import.span_imported,
        came_from_commonjs: false,
      },
    );
    self
      .result
      .named_imports
      .insert(generated_imported_as_ref, name_import);
  }

  fn add_star_re_export(
    &mut self,
    export_name: &str,
    record_idx: ImportRecordIdx,
    span_for_export_name: Span,
  ) {
    let generated_imported_as_ref = self
      .result
      .symbol_ref_db
      .create_facade_root_symbol_ref(legitimize_identifier_name(export_name).as_ref());
    // So does re-exporting a namespace, through its `default` (#693).
    *generated_imported_as_ref.flags_mut(&mut self.result.symbol_ref_db) |=
      SymbolRefFlags::HasComputedMemberWrite;
    self
      .current_stmt_info
      .declared_symbols
      .push(rolldown_common::TaggedSymbolRef::normal(
        generated_imported_as_ref,
      ));
    let name_import = NamedImport {
      imported: Specifier::Star,
      span_imported: span_for_export_name,
      imported_as: generated_imported_as_ref,
      record_idx,
    };

    self.result.named_exports.insert(
      export_name.into(),
      LocalExport {
        referenced: generated_imported_as_ref,
        span: name_import.span_imported,
        came_from_commonjs: false,
      },
    );
    self
      .result
      .named_imports
      .insert(generated_imported_as_ref, name_import);
  }

  fn scan_export_all_decl(&mut self, decl: &ExportAllDeclaration) {
    let id = self.add_import_record(
      &to_usv_str(decl.source.value),
      ImportKind::Import,
      decl.source.span(),
      decl.span,
      if decl.source.span().is_empty() {
        ImportRecordMeta::IsUnspannedImport
      } else {
        ImportRecordMeta::empty()
      },
      None,
    );
    if let Some(exported) = &decl.exported {
      // export * as ns from '...'
      self.add_star_re_export(exported.name().as_str(), id, decl.span);
      self.result.import_records[id]
        .meta
        .insert(ImportRecordMeta::IsReExportOnly);
    } else {
      // export * from '...'
      self.result.import_records[id]
        .meta
        .insert(ImportRecordMeta::IsExportStar);
      self
        .result
        .ecma_view_meta
        .insert(EcmaViewMeta::HasStarExport);
    }
    self.result.imports.insert(decl.node_id(), id);
    if let Some(ref with_clause) = decl.with_clause {
      self
        .result
        .import_attribute_map
        .insert(id, ImportAttribute::from_with_clause(with_clause));
    }
  }

  fn visit_function_decl(&mut self, it: &ast::Function<'ast>, flags: oxc::semantic::ScopeFlags) {
    self
      .current_stmt_info
      .meta
      .insert(StmtInfoMeta::KeepNamesType);
    walk_js::walk_function(self, it, flags);
  }

  fn visit_class_decl(&mut self, it: &ast::Class<'ast>) {
    let previous_class_decl_id = self.cur_class_decl.take();
    self.cur_class_decl = self.get_class_id(it);
    self
      .current_stmt_info
      .meta
      .insert(StmtInfoMeta::KeepNamesType);
    walk_js::walk_class(self, it);
    self.cur_class_decl = previous_class_decl_id;
  }

  fn scan_export_from_decl(&mut self, decl: &ExportFromDeclaration<'ast>) {
    let source = &decl.source;
    let record_idx = self.add_import_record(
      &to_usv_str(source.value),
      ImportKind::Import,
      source.span(),
      decl.span,
      if source.span().is_empty() {
        ImportRecordMeta::IsUnspannedImport
      } else {
        ImportRecordMeta::empty()
      },
      None,
    );
    decl.specifiers.iter().for_each(|spec| {
      self.validate_attributed_json_binding(
        decl.with_clause.as_deref(), source, spec.local.name().as_str(), spec.local.span(),
      );
      self.add_re_export(
        spec.exported.name().as_str(),
        spec.local.name().as_str(),
        record_idx,
        spec.local.span(),
      );
    });
    if let Some(ref with_clause) = decl.with_clause {
      self
        .result
        .import_attribute_map
        .insert(record_idx, ImportAttribute::from_with_clause(with_clause));
    }
    self.result.imports.insert(decl.node_id(), record_idx);
    self.result.import_records[record_idx]
      .meta
      .insert(ImportRecordMeta::IsReExportOnly);
  }

  fn scan_export_named_decl(&mut self, decl: &ExportNamedDeclaration<'ast>) {
    decl.specifiers.iter().for_each(|spec| {
      if let Some(local_symbol_id) = self.get_root_binding(spec.local.name().as_str()) {
        self.add_local_export(spec.exported.name().as_str(), local_symbol_id, spec.span);
      }
    });
  }

  fn scan_export_decl(&mut self, export_decl: &ExportDeclaration<'ast>) {
    match &export_decl.declaration {
      ast::Declaration::VariableDeclaration(var_decl) => {
        var_decl.declarations.iter().for_each(|decl| {
          decl
            .id
            .get_binding_identifiers()
            .into_iter()
            .for_each(|id| {
              self.add_local_export(&id.name, id.symbol_id(), id.span);
            });
          if let BindingPattern::BindingIdentifier(ref binding) = decl.id {
            let symbol_id = binding.symbol_id();
            if let Some(value) = self.extract_constant_value_from_expr(decl.init.as_ref()) {
              self.add_constant_symbol(symbol_id, ConstExportMeta::new(value, false));
            }
            let (is_side_effect_free_function, is_pure_annotation_only) = decl
              .init
              .as_ref()
              .map(|expr| match expr {
                Expression::FunctionExpression(func) => {
                  let empty = func.is_side_effect_free();
                  (empty || func.pure, func.pure && !empty)
                }
                Expression::ArrowFunctionExpression(func) => {
                  let empty = func.is_side_effect_free();
                  (empty || func.pure, func.pure && !empty)
                }
                _ => (false, false),
              })
              .unwrap_or((false, false));
            if is_side_effect_free_function {
              self
                .result
                .ecma_view_meta
                .insert(EcmaViewMeta::TopExportedSideEffectsFreeFunction);
              let flags = self
                .result
                .symbol_ref_db
                .flags
                .entry(symbol_id)
                .or_default();
              flags.insert(SymbolRefFlags::SideEffectsFreeFunction);
              if is_pure_annotation_only {
                flags.insert(SymbolRefFlags::PureAnnotationOnly);
              }
            }
          }
        });
      }
      ast::Declaration::FunctionDeclaration(fn_decl) => {
        let binding_id = fn_decl.id.as_ref().unwrap();
        let symbol_id = binding_id.symbol_id();
        self.add_local_export(binding_id.name.as_str(), symbol_id, binding_id.span);
        let empty = fn_decl.is_side_effect_free();
        if empty || fn_decl.pure {
          self
            .result
            .ecma_view_meta
            .insert(EcmaViewMeta::TopExportedSideEffectsFreeFunction);
          let flags = self
            .result
            .symbol_ref_db
            .flags
            .entry(symbol_id)
            .or_default();
          flags.insert(SymbolRefFlags::SideEffectsFreeFunction);
          if fn_decl.pure && !empty {
            flags.insert(SymbolRefFlags::PureAnnotationOnly);
          }
        }
      }
      ast::Declaration::ClassDeclaration(cls_decl) => {
        let id = cls_decl.id.as_ref().unwrap();
        self.add_local_export(id.name.as_str(), id.symbol_id(), id.span);
      }
      _ => {}
    }
  }

  // If the reference is a global variable, `None` will be returned.
  fn resolve_symbol_from_reference(&self, id_ref: &IdentifierReference) -> Option<SymbolId> {
    let ref_id = id_ref.reference_id.get().unwrap_or_else(|| {
      panic!(
        "Identifier `{}` at {}..{} must have a reference id",
        id_ref.name, id_ref.span.start, id_ref.span.end
      )
    });
    self.result.symbol_ref_db.ast_scopes.symbol_id_for(ref_id)
  }

  fn scan_export_default_decl(&mut self, decl: &ExportDefaultDeclaration) {
    let local_binding_for_default_export = match &decl.declaration {
      // An expression initializes a distinct binding at this statement, even when its
      // identifier already has a value. A cyclic reader can observe the interval.
      ast::ExportDefaultDeclarationKind::Identifier(_) => None,
      ast::ExportDefaultDeclarationKind::FunctionDeclaration(fn_decl) => {
        let empty = fn_decl.is_side_effect_free();
        if empty || fn_decl.pure {
          self
            .result
            .ecma_view_meta
            .insert(EcmaViewMeta::TopExportedSideEffectsFreeFunction);
          let flags = self
            .result
            .symbol_ref_db
            .flags
            .entry(self.result.default_export_ref.symbol)
            .or_default();
          flags.insert(SymbolRefFlags::SideEffectsFreeFunction);
          if fn_decl.pure && !empty {
            flags.insert(SymbolRefFlags::PureAnnotationOnly);
          }
        }
        fn_decl.id.as_ref().map(|id| {
          let symbol_id = id.symbol_id();
          self.result.default_export_ref.symbol = symbol_id;
          (symbol_id, id.span)
        })
      }
      ast::ExportDefaultDeclarationKind::ClassDeclaration(cls_decl) => {
        cls_decl.id.as_ref().map(|id| {
          let symbol_id = id.symbol_id();
          self.result.default_export_ref.symbol = symbol_id;
          (symbol_id, id.span)
        })
      }
      _ => None,
    };
    let (reference, span) = local_binding_for_default_export
      .unwrap_or((self.result.default_export_ref.symbol, Span::default()));

    if let Some(v) = self.extract_constant_value_from_expr(decl.declaration.as_expression()) {
      self.add_constant_symbol(reference, ConstExportMeta::new(v, false));
    }

    self.declare_normal_symbol_ref(reference);
    self.add_local_default_export(reference, span);
  }

  fn validate_attributed_json_binding(
    &mut self,
    attributes: Option<&ast::WithClause>,
    source: &ast::StringLiteral,
    name: &str,
    span: Span,
  ) {
    if name == "default" || !attributes.is_some_and(|attributes| {
      attributes.with_entries.iter().any(|attribute| {
        let key = match &attribute.key {
          ast::ImportAttributeKey::Identifier(key) => Some(key.name.as_str()),
          ast::ImportAttributeKey::StringLiteral(key) => key.value.as_str(),
        };
        key == Some("type") && attribute.value.value == "json"
      })
    }) {
      return;
    }
    self.result.errors.push(BuildDiagnostic::missing_export(
      self.immutable_ctx.id.to_string(),
      self.immutable_ctx.id.to_string(),
      source.value.display().to_string(),
      source.value.display().to_string(),
      self.immutable_ctx.source.clone(),
      name.into(),
      span,
      None,
    ));
  }

  fn check_deferred_import(&mut self, phase: Option<ast::ImportPhase>, span: Span) {
    if phase == Some(ast::ImportPhase::Defer)
      && !crate::utils::native_records::enabled(self.immutable_ctx.options)
    {
      self.result.errors.push(BuildDiagnostic::unsupported_feature(
        self.immutable_ctx.id.as_arc_str().clone(),
        self.immutable_ctx.source.clone(),
        span,
        "Deferred imports require preserved native ESM records without execution-order wrappers".into(),
      ));
    }
  }

  fn scan_import_decl(&mut self, decl: &ImportDeclaration) {
    self.check_deferred_import(decl.phase, decl.span);
    let rec_id = self.add_import_record(
      &to_usv_str(decl.source.value),
      ImportKind::Import,
      decl.source.span(),
      decl.span,
      if decl.source.span().is_empty() {
        ImportRecordMeta::IsUnspannedImport
      } else {
        ImportRecordMeta::empty()
      },
      None,
    );

    if let Some(ref with_clause) = decl.with_clause {
      self
        .result
        .import_attribute_map
        .insert(rec_id, ImportAttribute::from_with_clause(with_clause));
    }

    self.result.imports.insert(decl.node_id(), rec_id);

    let Some(specifiers) = &decl.specifiers else {
      return;
    };
    specifiers.iter().for_each(|spec| match spec {
      ast::ImportDeclarationSpecifier::ImportSpecifier(spec) => {
        let sym = spec.local.symbol_id();
        let imported = spec.imported.name();
        self.validate_attributed_json_binding(
          decl.with_clause.as_deref(), &decl.source, imported.as_str(), spec.imported.span(),
        );
        self.add_named_import(sym, imported.as_str(), rec_id, spec.imported.span());
      }
      ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(spec) => {
        self.add_named_import(spec.local.symbol_id(), "default", rec_id, spec.span);
      }
      ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(spec) => {
        let symbol_id = spec.local.symbol_id();
        self.add_star_import(symbol_id, rec_id, spec.local.span());
        if decl.phase == Some(ast::ImportPhase::Defer) {
          // This binding is the host's deferred namespace, not an alias for the
          // importee's eager namespace or for any of its exported bindings.
          self
            .current_stmt_info
            .declared_symbols
            .push(TaggedSymbolRef::normal(
              (self.immutable_ctx.idx, symbol_id).into(),
            ));
          self.namespace_object_symbol_ids.remove(&symbol_id);
        }
      }
    });
  }

  fn scan_module_decl(&mut self, decl: &ModuleDeclaration<'ast>) {
    match decl {
      ast::ModuleDeclaration::ImportDeclaration(decl) => {
        self
          .esm_import_keyword
          .get_or_insert(Span::new(decl.span.start, decl.span.start + 6));
        self.scan_import_decl(decl);
      }
      ast::ModuleDeclaration::ExportAllDeclaration(decl) => {
        self.set_esm_export_keyword(Span::new(decl.span.start, decl.span.start + 6));
        self.scan_export_all_decl(decl);
      }
      ast::ModuleDeclaration::ExportNamedDeclaration(decl) => {
        self.set_esm_export_keyword(Span::new(decl.span.start, decl.span.start + 6));
        self.scan_export_named_decl(decl);
      }
      ast::ModuleDeclaration::ExportDeclaration(decl) => {
        self.set_esm_export_keyword(Span::new(decl.span.start, decl.span.start + 6));
        self.scan_export_decl(decl);
      }
      ast::ModuleDeclaration::ExportFromDeclaration(decl) => {
        self.set_esm_export_keyword(Span::new(decl.span.start, decl.span.start + 6));
        self.scan_export_from_decl(decl);
      }
      ast::ModuleDeclaration::ExportDefaultDeclaration(decl) => {
        self.set_esm_export_keyword(Span::new(decl.span.start, decl.span.start + 6));
        self.scan_export_default_decl(decl);
        match &decl.declaration {
          ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            self.visit_class_decl(class);
          }
          ast::ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
            self.visit_function_decl(func, ScopeFlags::Function);
          }
          _ => {}
        }
      }
      _ => {}
    }
  }

  pub fn add_referenced_symbol(&mut self, sym_ref: SymbolRef) {
    self
      .current_stmt_info
      .referenced_symbols
      .push(sym_ref.into());
  }

  #[inline]
  pub fn add_member_expr_reference(&mut self, member_expr_ref: MemberExprRef) {
    self
      .current_stmt_info
      .referenced_symbols
      .push(member_expr_ref.into());
  }

  /// Check if this identifier reference is the object of a member expression in a write context
  /// (e.g., `A` in `A.foo = 1`, `A.foo += 1`, `A.foo++`, `delete A.foo`).
  fn is_member_write_target(&self, ident_ref: &IdentifierReference) -> bool {
    ident_ref.reference_id.get().is_some_and(|ref_id| {
      self
        .result
        .symbol_ref_db
        .scoping()
        .get_reference(ref_id)
        .flags()
        .is_member_write_target()
    })
  }

  /// Whether the value at `node`, whose parent is the visit path's entry `depth` places from its
  /// end, is only read through static members, `node.name`, or `node.default.name` where
  /// `default_hops` allows a `default` step, or is discarded. A member that is assigned, updated,
  /// deleted, destructured into, called or tagged, which passes `node` as `this`, doesn't count,
  /// nor does `__proto__` or `constructor`, which reach the object's prototype, nor any other use
  /// of the value (#693).
  fn only_member_read(&self, mut node: Span, depth: usize, mut default_hops: usize) -> bool {
    let Some(mut index) = self.visit_path.len().checked_sub(depth + 1) else {
      return false;
    };
    loop {
      let kind = self.visit_path[index];
      match kind {
        AstKind::ParenthesizedExpression(expr) => node = expr.span,
        AstKind::ExpressionStatement(_) => return true,
        AstKind::SequenceExpression(expr)
          if expr.expressions.last().is_some_and(|last| last.span() != node) =>
        {
          return true;
        }
        kind => match member_name(&kind, node) {
          Some("default") if default_hops > 0 => {
            default_hops -= 1;
            node = kind.span();
          }
          Some("__proto__" | "constructor") | None => return false,
          Some(_) => return self.is_member_read(kind.span(), index),
        },
      }
      let Some(parent) = index.checked_sub(1) else {
        return false;
      };
      index = parent;
    }
  }

  /// Whether the member expression at `node`, the visit path's entry `index`, is read: not the
  /// target of an assignment, an update, a `delete` or a destructuring, and not called, which
  /// passes its object as `this`.
  fn is_member_read(&self, mut node: Span, index: usize) -> bool {
    for kind in self.visit_path[..index].iter().rev() {
      match kind {
        AstKind::ParenthesizedExpression(expr) => node = expr.span,
        AstKind::ChainExpression(expr) => node = expr.span,
        AstKind::AssignmentExpression(expr) => return expr.left.span() != node,
        AstKind::ForInStatement(stmt) => return stmt.left.span() != node,
        AstKind::ForOfStatement(stmt) => return stmt.left.span() != node,
        AstKind::UnaryExpression(expr) => return expr.operator != ast::UnaryOperator::Delete,
        AstKind::CallExpression(call) => return call.callee.span() != node,
        AstKind::TaggedTemplateExpression(expr) => return expr.tag.span() != node,
        AstKind::UpdateExpression(_)
        | AstKind::ArrayAssignmentTarget(_)
        | AstKind::ObjectAssignmentTarget(_)
        | AstKind::AssignmentTargetRest(_)
        | AstKind::AssignmentTargetWithDefault(_)
        | AstKind::AssignmentTargetPropertyProperty(_) => return false,
        _ => return true,
      }
    }
    true
  }

  /// Records how a CommonJS module uses its own exports object at `node`, `exports`,
  /// `module.exports` or top-level `this`, whose parent is the visit path's entry `depth` places
  /// from its end. Linking inlines the module's constants only while the object doesn't escape
  /// (#693): the module may define an export with `=` on a static member, in a statement of its
  /// own at the top level, which runs unconditionally and stays constant while it is defined once,
  /// read a static member, mark itself with `Object.defineProperty(exports, '__esModule', ...)`
  /// in a statement that discards the call's result, take the object's type, discard it, or test
  /// it on the left of `&&`, which then evaluates to its right operand. `__proto__` and
  /// `constructor` reach the object's prototype, whose setters a definition would call.
  fn note_commonjs_exports_use(&mut self, mut node: Span, depth: usize) {
    let Some(mut index) = self.visit_path.len().checked_sub(depth + 1) else {
      self.commonjs_exports_escape = true;
      return;
    };
    loop {
      let kind = self.visit_path[index];
      match kind {
        AstKind::ParenthesizedExpression(expr) => node = expr.span,
        AstKind::ExpressionStatement(_) => return,
        AstKind::UnaryExpression(expr) if expr.operator == ast::UnaryOperator::Typeof => return,
        AstKind::LogicalExpression(expr)
          if expr.operator == ast::LogicalOperator::And && expr.left.span() == node =>
        {
          return;
        }
        // The mark's result is the object, so only a statement that discards it is a mark.
        AstKind::CallExpression(call)
          if call.arguments.first().is_some_and(|argument| argument.span() == node)
            && matches!(
              is_object_define_property_es_module(&self.result.symbol_ref_db.ast_scopes, call),
              Some(CommonJsAstType::EsModuleFlag)
            )
            && index.checked_sub(1).is_some_and(|parent| {
              matches!(self.visit_path[parent], AstKind::ExpressionStatement(_))
            }) =>
        {
          return;
        }
        kind => {
          let Some(name) =
            member_name(&kind, node).filter(|name| !matches!(*name, "__proto__" | "constructor"))
          else {
            break;
          };
          let member = kind.span();
          let parent = index.checked_sub(1).map(|parent| self.visit_path[parent]);
          if let Some(AstKind::AssignmentExpression(expr)) = parent
            && expr.left.span() == member
          {
            // The program's statements are the visit path's roots (`visit_program`).
            if expr.operator == ast::AssignmentOperator::Assign
              && index == 2
              && matches!(self.visit_path[0], AstKind::ExpressionStatement(_))
            {
              *self.commonjs_export_definitions.entry(name.into()).or_default() += 1;
              return;
            }
            break;
          }
          if self.is_member_read(member, index) {
            return;
          }
          break;
        }
      }
      let Some(parent) = index.checked_sub(1) else {
        break;
      };
      index = parent;
    }
    self.commonjs_exports_escape = true;
  }

  fn is_root_symbol(&self, symbol_id: SymbolId) -> bool {
    self.result.symbol_ref_db.scoping().root_scope_id()
      == self.result.symbol_ref_db.symbol_scope_id(symbol_id)
  }

  fn try_diagnostic_forbid_const_assign(&mut self, id_ref: &IdentifierReference) -> Option<()> {
    let ref_id = id_ref.reference_id.get()?;
    let reference = &self.result.symbol_ref_db.scoping().get_reference(ref_id);
    if reference.is_write() {
      let symbol_id = reference.symbol_id()?;
      // Root bindings retain const-write checks in either native scope or the
      // initialization-state finalizer. Reassignment throws at runtime, not parse time.
      if self
        .result
        .symbol_ref_db
        .scoping()
        .symbol_flags(symbol_id)
        .is_const_variable()
        && !self.is_root_symbol(symbol_id)
      {
        self
          .result
          .errors
          .push(BuildDiagnostic::forbid_const_assign(
            self.immutable_ctx.id.to_string(),
            self.immutable_ctx.source.clone(),
            self.result.symbol_ref_db.symbol_name(symbol_id).into(),
            self.result.symbol_ref_db.scoping().symbol_span(symbol_id),
            id_ref.span(),
          ));
      }
    }
    None
  }

  /// return a `Some(SymbolRef)` if the identifier referenced a top level `IdentBinding`
  fn resolve_identifier_reference(&self, ident: &IdentifierReference) -> IdentifierReferenceKind {
    match self.resolve_symbol_from_reference(ident) {
      Some(symbol_id) => {
        if self.is_root_symbol(symbol_id) {
          IdentifierReferenceKind::Root((self.immutable_ctx.idx, symbol_id).into())
        } else {
          IdentifierReferenceKind::Other
        }
      }
      None => IdentifierReferenceKind::Global,
    }
  }

  /// StaticMemberExpression or ComputeMemberExpression with static key
  pub fn try_extract_parent_static_member_expr_chain(
    &self,
    max_len: usize,
  ) -> Option<(NodeId, Span, Vec<MemberExprProp>)> {
    let mut node_id = NodeId::DUMMY;
    let mut span = SPAN;
    let mut props = vec![];
    for ancestor_ast in self.visit_path.iter().rev().take(max_len) {
      match ancestor_ast {
        AstKind::StaticMemberExpression(expr) => {
          node_id = ancestor_ast.node_id();
          span = ancestor_ast.span();
          props.push(MemberExprProp {
            name: expr.property.name.as_str().into(),
            span: expr.property.span(),
            optional: expr.optional,
          });
        }
        AstKind::ComputedMemberExpression(expr) => {
          if let Some(name) = expr.static_property_name().and_then(JSStr::as_str) {
            node_id = ancestor_ast.node_id();
            span = ancestor_ast.span();
            props.push(MemberExprProp {
              name: name.into(),
              span: expr.expression.span(),
              optional: expr.optional,
            });
          } else {
            break;
          }
        }
        _ => break,
      }
    }
    (!props.is_empty()).then_some((node_id, span, props))
  }

  /// Returns the `BinaryExpression` node, its span and `key` if the visited identifier is `ns` in
  /// `'key' in ns`.
  pub fn try_extract_parent_in_operator_key(&self) -> Option<(NodeId, Span, CompactStr)> {
    let AstKind::BinaryExpression(expr) = self.visit_path.last()? else {
      return None;
    };
    if expr.operator != ast::BinaryOperator::In {
      return None;
    }
    let Expression::StringLiteral(key) = &expr.left else {
      return None;
    };
    Some((expr.node_id(), expr.span, key.value.as_str()?.into()))
  }

  // `console` in `console.log` is a global reference
  pub fn is_global_identifier_reference(&self, ident: &IdentifierReference) -> bool {
    let symbol_id = self.resolve_symbol_from_reference(ident);
    symbol_id.is_none()
  }

  /// If it is not a top level `this` reference visit position
  pub fn is_this_nested(&self) -> bool {
    self.is_nested_this_inside_class
      || self
        .scope_stack
        .iter()
        .any(|flags| flags.contains(ScopeFlags::Function) && !flags.contains(ScopeFlags::Arrow))
  }

  pub fn in_side_try_catch_block(&self) -> bool {
    for kind in self.visit_path.iter().rev() {
      match kind {
        AstKind::TryStatement(_) => return true,
        AstKind::ArrowFunctionExpression(_) | AstKind::FunctionBody(_) | AstKind::Function(_) => {
          return false;
        }
        _ => {}
      }
    }
    false
  }

  #[inline]
  pub fn create_constant_eval_ctx(&'me self) -> ConstEvalCtx<'me, 'ast> {
    ConstEvalCtx {
      ast: oxc::ast::builder::AstBuilder::new(self.immutable_ctx.allocator),
      scope: self.result.symbol_ref_db.scoping(),
      constant_map: &self.result.constant_export_map,
      overrode_get_constant_value_from_reference_id: None,
    }
  }

  pub fn extract_constant_value_from_expr(
    &self,
    expr: Option<&Expression<'ast>>,
  ) -> Option<ConstantValue> {
    if !self.immutable_ctx.flat_options.inline_const_enabled() {
      return None;
    }
    expr.and_then(|expr| try_extract_const_literal(&self.create_constant_eval_ctx(), expr))
  }

  pub fn add_constant_symbol(&mut self, symbol_id: SymbolId, value: ConstExportMeta) {
    let scoping = self.result.symbol_ref_db.scoping();
    let is_mutated = !self.result.symbol_ref_db.is_facade_symbol(symbol_id)
      && scoping.symbol_is_mutated(symbol_id);
    if is_mutated || eval_can_assign(scoping, symbol_id) {
      return;
    }
    self.result.constant_export_map.insert(symbol_id, value);
  }

  fn is_import_expr_ignored_by_comment(&mut self, expr: &ImportExpression<'ast>) -> bool {
    let mut should_ignore = false;
    while self.current_comment_idx < self.immutable_ctx.comments.len() {
      let comment = &self.immutable_ctx.comments[self.current_comment_idx];
      if comment.attached_to >= expr.span.end {
        break;
      }
      if comment.attached_to >= expr.span.start && comment.is_vite() {
        should_ignore = true;
      }
      self.current_comment_idx += 1;
    }
    should_ignore
  }
}

#[derive(Debug, Clone, Copy)]
pub enum IdentifierReferenceKind {
  /// global variable
  Global,
  /// top level variable
  Root(SymbolRef),
  /// rest
  Other,
}

/// The property name of `kind`, a member expression of the value at `node`, when it is static or a
/// string literal (`exports['name']`); JSX members (`<ns.Component />`) included.
fn member_name<'a>(kind: &AstKind<'a>, node: Span) -> Option<&'a str> {
  match kind {
    AstKind::StaticMemberExpression(member) if member.object.span() == node => {
      Some(member.property.name.as_str())
    }
    AstKind::ComputedMemberExpression(member) if member.object.span() == node => {
      member.static_property_name().and_then(JSStr::as_str)
    }
    AstKind::JSXMemberExpression(member) if member.object.span() == node => {
      Some(member.property.name.as_str())
    }
    _ => None,
  }
}
