pub mod dynamic_import;
pub mod impl_visit;
mod import_assign_analyzer;
mod new_url;
pub mod side_effect_detector;

use arcstr::ArcStr;
use oxc::ast::ast::MemberExpression;
use oxc::ast::{ast, AstKind};
use oxc::semantic::{Reference, ReferenceId, ScopeFlags, ScopeId, SymbolTable};
use oxc::span::SPAN;
use oxc::{
  ast::{
    ast::{
      ExportAllDeclaration, ExportDefaultDeclaration, ExportNamedDeclaration, IdentifierReference,
      ImportDeclaration, ModuleDeclaration, Program,
    },
    Comment, Visit,
  },
  semantic::SymbolId,
  span::{CompactStr, GetSpan, Span},
};
use oxc_index::IndexVec;
use rolldown_common::dynamic_import_usage::{DynamicImportExportsUsage, DynamicImportUsageInfo};
use rolldown_common::{
  AstScopes, EcmaModuleAstUsage, ExportsKind, ImportKind, ImportRecordIdx, ImportRecordMeta,
  LocalExport, MemberExprRef, ModuleDefFormat, ModuleId, ModuleIdx, NamedImport, RawImportRecord,
  Specifier, StmtInfo, StmtInfos, SymbolRef, SymbolRefDbForModule, SymbolRefFlags,
  ThisExprReplaceKind,
};
use rolldown_ecmascript_utils::{BindingIdentifierExt, BindingPatternExt};
use rolldown_error::{BuildDiagnostic, BuildResult, CjsExportSpan};
use rolldown_rstr::Rstr;
use rolldown_std_utils::PathExt;
use rolldown_utils::concat_string;
use rolldown_utils::ecmascript::legitimize_identifier_name;
use rustc_hash::{FxHashMap, FxHashSet};
use sugar_path::SugarPath;

use crate::SharedOptions;

#[derive(Debug)]
pub struct ScanResult {
  pub named_imports: FxHashMap<SymbolRef, NamedImport>,
  pub named_exports: FxHashMap<Rstr, LocalExport>,
  pub stmt_infos: StmtInfos,
  pub import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub default_export_ref: SymbolRef,
  pub imports: FxHashMap<Span, ImportRecordIdx>,
  pub exports_kind: ExportsKind,
  pub warnings: Vec<BuildDiagnostic>,
  pub errors: Vec<BuildDiagnostic>,
  pub has_eval: bool,
  pub ast_usage: EcmaModuleAstUsage,
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
  pub has_star_exports: bool,
  /// we don't know the ImportRecord related ModuleIdx yet, so use ImportRecordIdx as key
  /// temporarily
  pub dynamic_import_rec_exports_usage: FxHashMap<ImportRecordIdx, DynamicImportExportsUsage>,
  /// `new URL('...', import.meta.url)`
  pub new_url_references: FxHashMap<Span, ImportRecordIdx>,
  pub this_expr_replace_map: FxHashMap<Span, ThisExprReplaceKind>,
}

pub struct AstScanner<'me, 'ast> {
  idx: ModuleIdx,
  source: &'me ArcStr,
  module_type: ModuleDefFormat,
  id: &'me ModuleId,
  scopes: &'me AstScopes,
  comments: &'me oxc::allocator::Vec<'me, Comment>,
  current_stmt_info: StmtInfo,
  result: ScanResult,
  esm_export_keyword: Option<Span>,
  esm_import_keyword: Option<Span>,
  /// Represents [Module Namespace Object](https://tc39.es/ecma262/#sec-module-namespace-exotic-objects)
  pub namespace_object_ref: SymbolRef,
  /// cjs ident span used for emit `commonjs_variable_in_esm` warning
  cjs_exports_ident: Option<Span>,
  cjs_module_ident: Option<Span>,
  /// Whether the module is a commonjs module
  /// The reason why we can't reuse `cjs_exports_ident` and `cjs_module_ident` is that
  /// any `module` or `exports` in the top-level scope should be treated as a commonjs module.
  /// `cjs_exports_ident` and `cjs_module_ident` only only recorded when they are appear in
  /// lhs of AssignmentExpression
  ast_usage: EcmaModuleAstUsage,
  cur_class_decl_and_symbol_referenced_ids: Option<(SymbolId, &'me Vec<ReferenceId>)>,
  visit_path: Vec<AstKind<'ast>>,
  scope_stack: Vec<Option<ScopeId>>,
  options: &'me SharedOptions,
  dynamic_import_usage_info: DynamicImportUsageInfo,
  ignore_comment: &'static str,
  /// "top level" `this` AstNode range in source code
  top_level_this_expr_set: FxHashSet<Span>,
  /// A flag to resolve `this` appear with propertyKey in class
  is_nested_this_inside_class: bool,
}

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    idx: ModuleIdx,
    scope: &'me AstScopes,
    symbol_table: SymbolTable,
    repr_name: &'me str,
    module_type: ModuleDefFormat,
    source: &'me ArcStr,
    file_path: &'me ModuleId,
    comments: &'me oxc::allocator::Vec<'me, Comment>,
    options: &'me SharedOptions,
  ) -> Self {
    let mut symbol_ref_db = SymbolRefDbForModule::new(symbol_table, idx, scope.root_scope_id());
    // This is used for converting "export default foo;" => "var default_symbol = foo;"
    let legitimized_repr_name = legitimize_identifier_name(repr_name);
    let default_export_ref = symbol_ref_db
      .create_facade_root_symbol_ref(concat_string!(legitimized_repr_name, "_default").into());
    // This is used for converting "export default foo;" => "var [default_export_ref] = foo;"
    // And we consider [default_export_ref] never get reassigned.
    default_export_ref.flags_mut(&mut symbol_ref_db).insert(SymbolRefFlags::IS_NOT_REASSIGNED);

    let name = concat_string!(legitimized_repr_name, "_exports");
    let namespace_object_ref = symbol_ref_db.create_facade_root_symbol_ref(name.into());

    let result = ScanResult {
      named_imports: FxHashMap::default(),
      named_exports: FxHashMap::default(),
      stmt_infos: {
        let mut stmt_infos = StmtInfos::default();
        // The first `StmtInfo` is used to represent the statement that declares and constructs Module Namespace Object
        stmt_infos.push(StmtInfo::default());
        stmt_infos
      },
      import_records: IndexVec::new(),
      default_export_ref,
      imports: FxHashMap::default(),
      exports_kind: ExportsKind::None,
      warnings: Vec::new(),
      has_eval: false,
      errors: Vec::new(),
      ast_usage: EcmaModuleAstUsage::empty(),
      symbol_ref_db,
      self_referenced_class_decl_symbol_ids: FxHashSet::default(),
      hashbang_range: None,
      has_star_exports: false,
      dynamic_import_rec_exports_usage: FxHashMap::default(),
      new_url_references: FxHashMap::default(),
      this_expr_replace_map: FxHashMap::default(),
    };

    Self {
      idx,
      scopes: scope,
      current_stmt_info: StmtInfo::default(),
      result,
      esm_export_keyword: None,
      esm_import_keyword: None,
      module_type,
      namespace_object_ref,
      cjs_module_ident: None,
      cjs_exports_ident: None,
      source,
      id: file_path,
      comments,
      ast_usage: EcmaModuleAstUsage::empty(),
      cur_class_decl_and_symbol_referenced_ids: None,
      visit_path: vec![],
      ignore_comment: options.experimental.get_ignore_comment(),
      options,
      scope_stack: vec![],
      dynamic_import_usage_info: DynamicImportUsageInfo::default(),
      top_level_this_expr_set: FxHashSet::default(),
      is_nested_this_inside_class: false,
    }
  }

  /// if current visit path is top level
  pub fn is_top_level(&self) -> bool {
    self
      .scope_stack
      .iter()
      .filter_map(|item| *item)
      .rev()
      .all(|scope| self.scopes.get_flags(scope).is_top())
  }

  pub fn scan(mut self, program: &Program<'ast>) -> BuildResult<ScanResult> {
    self.visit_program(program);
    let mut exports_kind = ExportsKind::None;

    if self.esm_export_keyword.is_some() {
      exports_kind = ExportsKind::Esm;
      if let Some(start) = self.cjs_module_ident {
        self.result.warnings.push(
          BuildDiagnostic::commonjs_variable_in_esm(
            self.id.to_string(),
            self.source.clone(),
            // SAFETY: we checked at the beginning
            self.esm_export_keyword.expect("should have start offset"),
            CjsExportSpan::Module(start),
          )
          .with_severity_warning(),
        );
      }
      if let Some(start) = self.cjs_exports_ident {
        self.result.warnings.push(
          BuildDiagnostic::commonjs_variable_in_esm(
            self.id.to_string(),
            self.source.clone(),
            // SAFETY: we checked at the beginning
            self.esm_export_keyword.expect("should have start offset"),
            CjsExportSpan::Exports(start),
          )
          .with_severity_warning(),
        );
      }
    } else if self.ast_usage.intersects(EcmaModuleAstUsage::ModuleOrExports) {
      exports_kind = ExportsKind::CommonJs;
    } else {
      // TODO(hyf0): Should add warnings if the module type doesn't satisfy the exports kind.
      match self.module_type {
        ModuleDefFormat::CJS | ModuleDefFormat::CjsPackageJson | ModuleDefFormat::Cts => {
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

    self.result.exports_kind = exports_kind;

    if cfg!(debug_assertions) {
      use rustc_hash::FxHashSet;
      let mut scanned_symbols_in_root_scope = self
        .result
        .stmt_infos
        .iter()
        .flat_map(|stmt_info| stmt_info.declared_symbols.iter())
        .collect::<FxHashSet<_>>();
      for (name, symbol_id) in self.scopes.get_bindings(self.scopes.root_scope_id()) {
        let symbol_ref: SymbolRef = (self.idx, *symbol_id).into();
        let scope_id = self.result.symbol_ref_db.get_scope_id(*symbol_id);
        if !scanned_symbols_in_root_scope.remove(&symbol_ref) {
          return Err(anyhow::format_err!(
            "Symbol ({name:?}, {symbol_id:?}, {scope_id:?}) is declared in the top-level scope but doesn't get scanned by the scanner",
          ))?;
        }
      }
      // if !scanned_top_level_symbols.is_empty() {
      //   return Err(anyhow::format_err!(
      //     "Some top-level symbols are scanned by the scanner but not declared in the top-level scope: {scanned_top_level_symbols:?}",
      //   ));
      // }
    }
    self.result.ast_usage = self.ast_usage;
    Ok(self.result)
  }

  fn set_esm_export_keyword(&mut self, span: Span) {
    self.esm_export_keyword.get_or_insert(span);
  }

  fn add_declared_id(&mut self, id: SymbolId) {
    self.current_stmt_info.declared_symbols.push((self.idx, id).into());
  }

  fn get_root_binding(&self, name: &str) -> Option<SymbolId> {
    self.scopes.get_root_binding(name)
  }

  /// `is_dummy` means if it the import record is created during ast transformation.
  fn add_import_record(
    &mut self,
    module_request: &str,
    kind: ImportKind,
    span: Span,
    init_meta: ImportRecordMeta,
  ) -> ImportRecordIdx {
    // If 'foo' in `import ... from 'foo'` is finally a commonjs module, we will convert the import statement
    // to `var import_foo = __toESM(require_foo())`, so we create a symbol for `import_foo` here. Notice that we
    // just create the symbol. If the symbol is finally used would be determined in the linking stage.
    let namespace_ref: SymbolRef = self.result.symbol_ref_db.create_facade_root_symbol_ref(
      concat_string!(
        "#LOCAL_NAMESPACE_IN_",
        itoa::Buffer::new().format(self.current_stmt_info.stmt_idx.unwrap_or_default()),
        "#"
      )
      .into(),
    );
    let rec = RawImportRecord::new(Rstr::from(module_request), kind, namespace_ref, span, None)
      .with_meta(init_meta);

    let id = self.result.import_records.push(rec);
    self.current_stmt_info.import_records.push(id);
    id
  }

  fn add_named_import(
    &mut self,
    local: SymbolId,
    imported: &str,
    record_id: ImportRecordIdx,
    span_imported: Span,
  ) {
    self.result.named_imports.insert(
      (self.idx, local).into(),
      NamedImport {
        imported: Rstr::new(imported).into(),
        imported_as: (self.idx, local).into(),
        span_imported,
        record_id,
      },
    );
  }

  fn add_star_import(&mut self, local: SymbolId, record_id: ImportRecordIdx, span_imported: Span) {
    self.result.named_imports.insert(
      (self.idx, local).into(),
      NamedImport {
        imported: Specifier::Star,
        imported_as: (self.idx, local).into(),
        record_id,
        span_imported,
      },
    );
  }

  fn add_local_export(&mut self, export_name: &str, local: SymbolId, span: Span) {
    let symbol_ref: SymbolRef = (self.idx, local).into();

    let is_const = self.result.symbol_ref_db.get_flags(local).is_const_variable();

    // If there is any write reference to the local variable, it is reassigned.
    let is_reassigned = self.scopes.get_resolved_references(local).any(Reference::is_write);

    let ref_flags = symbol_ref.flags_mut(&mut self.result.symbol_ref_db);
    if is_const {
      ref_flags.insert(SymbolRefFlags::IS_CONST);
    }
    if !is_reassigned {
      ref_flags.insert(SymbolRefFlags::IS_NOT_REASSIGNED);
    }

    self
      .result
      .named_exports
      .insert(export_name.into(), LocalExport { referenced: (self.idx, local).into(), span });
  }

  fn add_local_default_export(&mut self, local: SymbolId, span: Span) {
    self
      .result
      .named_exports
      .insert("default".into(), LocalExport { referenced: (self.idx, local).into(), span });
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
    record_id: ImportRecordIdx,
    span_imported: Span,
  ) {
    // We will pretend `export { [imported] as [export_name] }` to be `import `
    let generated_imported_as_ref =
      self.result.symbol_ref_db.create_facade_root_symbol_ref(if export_name == "default" {
        let importee_repr =
          self.result.import_records[record_id].module_request.as_path().representative_file_name();
        let importee_repr = legitimize_identifier_name(&importee_repr);
        concat_string!(importee_repr, "_default").into()
      } else {
        // the export_name could be a string literal
        legitimize_identifier_name(export_name).into()
      });

    self.current_stmt_info.declared_symbols.push(generated_imported_as_ref);
    let name_import = NamedImport {
      imported: imported.into(),
      imported_as: generated_imported_as_ref,
      record_id,
      span_imported,
    };
    if name_import.imported.is_default() {
      self.result.import_records[record_id].meta.insert(ImportRecordMeta::CONTAINS_IMPORT_DEFAULT);
    }
    self.result.named_exports.insert(
      export_name.into(),
      LocalExport { referenced: generated_imported_as_ref, span: name_import.span_imported },
    );
    self.result.named_imports.insert(generated_imported_as_ref, name_import);
  }

  fn add_star_re_export(
    &mut self,
    export_name: &str,
    record_id: ImportRecordIdx,
    span_for_export_name: Span,
  ) {
    let generated_imported_as_ref = self
      .result
      .symbol_ref_db
      .create_facade_root_symbol_ref(legitimize_identifier_name(export_name).into());
    self.current_stmt_info.declared_symbols.push(generated_imported_as_ref);
    let name_import = NamedImport {
      imported: Specifier::Star,
      span_imported: span_for_export_name,
      imported_as: generated_imported_as_ref,
      record_id,
    };

    self.result.import_records[record_id].meta.insert(ImportRecordMeta::CONTAINS_IMPORT_STAR);
    self.result.named_exports.insert(
      export_name.into(),
      LocalExport { referenced: generated_imported_as_ref, span: name_import.span_imported },
    );
    self.result.named_imports.insert(generated_imported_as_ref, name_import);
  }

  fn scan_export_all_decl(&mut self, decl: &ExportAllDeclaration) {
    let id = self.add_import_record(
      decl.source.value.as_str(),
      ImportKind::Import,
      decl.source.span(),
      if decl.source.span().is_empty() {
        ImportRecordMeta::IS_UNSPANNED_IMPORT
      } else {
        ImportRecordMeta::empty()
      },
    );
    if let Some(exported) = &decl.exported {
      // export * as ns from '...'
      self.add_star_re_export(exported.name().as_str(), id, decl.span);
    } else {
      // export * from '...'
      self.result.import_records[id].meta.insert(ImportRecordMeta::IS_EXPORT_STAR);
      self.result.has_star_exports = true;
    }
    self.result.imports.insert(decl.span, id);
  }

  fn scan_export_named_decl(&mut self, decl: &ExportNamedDeclaration) {
    if let Some(source) = &decl.source {
      let record_id = self.add_import_record(
        source.value.as_str(),
        ImportKind::Import,
        source.span(),
        if source.span().is_empty() {
          ImportRecordMeta::IS_UNSPANNED_IMPORT
        } else {
          ImportRecordMeta::empty()
        },
      );
      decl.specifiers.iter().for_each(|spec| {
        self.add_re_export(
          spec.exported.name().as_str(),
          spec.local.name().as_str(),
          record_id,
          spec.local.span(),
        );
      });
      self.result.imports.insert(decl.span, record_id);
      // `export {} from '...'`
      if decl.specifiers.is_empty() {
        self.result.import_records[record_id].meta.insert(ImportRecordMeta::IS_PLAIN_IMPORT);
      }
    } else {
      decl.specifiers.iter().for_each(|spec| {
        if let Some(local_symbol_id) = self.get_root_binding(spec.local.name().as_str()) {
          self.add_local_export(spec.exported.name().as_str(), local_symbol_id, spec.span);
        } else {
          self.result.errors.push(BuildDiagnostic::export_undefined_variable(
            self.id.to_string(),
            self.source.clone(),
            spec.local.span(),
            ArcStr::from(spec.local.name().as_str()),
          ));
        }
      });
      if let Some(decl) = decl.declaration.as_ref() {
        match decl {
          ast::Declaration::VariableDeclaration(var_decl) => {
            var_decl.declarations.iter().for_each(|decl| {
              decl.id.binding_identifiers().into_iter().for_each(|id| {
                self.add_local_export(&id.name, id.expect_symbol_id(), id.span);
              });
            });
          }
          ast::Declaration::FunctionDeclaration(fn_decl) => {
            let id = fn_decl.id.as_ref().unwrap();
            self.add_local_export(id.name.as_str(), id.expect_symbol_id(), id.span);
          }
          ast::Declaration::ClassDeclaration(cls_decl) => {
            let id = cls_decl.id.as_ref().unwrap();
            self.add_local_export(id.name.as_str(), id.expect_symbol_id(), id.span);
          }
          _ => unreachable!("doesn't support ts now"),
        }
      }
    }
  }

  // If the reference is a global variable, `None` will be returned.
  fn resolve_symbol_from_reference(&self, id_ref: &IdentifierReference) -> Option<SymbolId> {
    let ref_id = id_ref.reference_id.get().unwrap_or_else(|| {
      panic!(
        "{id_ref:#?} must have reference id in code```\n{}\n```\n",
        self.current_stmt_info.debug_label.as_deref().unwrap_or("<None>")
      )
    });
    self.scopes.symbol_id_for(ref_id)
  }
  fn scan_export_default_decl(&mut self, decl: &ExportDefaultDeclaration) {
    use oxc::ast::ast::ExportDefaultDeclarationKind;
    let local_binding_for_default_export = match &decl.declaration {
      oxc::ast::match_expression!(ExportDefaultDeclarationKind) => None,
      ast::ExportDefaultDeclarationKind::FunctionDeclaration(fn_decl) => fn_decl
        .id
        .as_ref()
        .map(|id| (rolldown_ecmascript_utils::BindingIdentifierExt::expect_symbol_id(id), id.span)),
      ast::ExportDefaultDeclarationKind::ClassDeclaration(cls_decl) => cls_decl
        .id
        .as_ref()
        .map(|id| (rolldown_ecmascript_utils::BindingIdentifierExt::expect_symbol_id(id), id.span)),
      ast::ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => unreachable!(),
    };

    let (reference, span) = local_binding_for_default_export
      .unwrap_or((self.result.default_export_ref.symbol, Span::default()));

    self.add_declared_id(reference);
    self.add_local_default_export(reference, span);
  }

  fn scan_import_decl(&mut self, decl: &ImportDeclaration) {
    let rec_id = self.add_import_record(
      decl.source.value.as_str(),
      ImportKind::Import,
      decl.source.span(),
      if decl.source.span().is_empty() {
        ImportRecordMeta::IS_UNSPANNED_IMPORT
      } else {
        ImportRecordMeta::empty()
      },
    );
    self.result.imports.insert(decl.span, rec_id);
    // // `import '...'` or `import {} from '...'`
    if decl.specifiers.as_ref().map_or(true, |s| s.is_empty()) {
      self.result.import_records[rec_id].meta.insert(ImportRecordMeta::IS_PLAIN_IMPORT);
    }

    let Some(specifiers) = &decl.specifiers else { return };
    specifiers.iter().for_each(|spec| match spec {
      ast::ImportDeclarationSpecifier::ImportSpecifier(spec) => {
        let sym = spec.local.expect_symbol_id();
        let imported = spec.imported.name();
        self.add_named_import(sym, imported.as_str(), rec_id, spec.imported.span());
        if imported == "default" {
          self.result.import_records[rec_id].meta.insert(ImportRecordMeta::CONTAINS_IMPORT_DEFAULT);
        }
      }
      ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(spec) => {
        self.add_named_import(spec.local.expect_symbol_id(), "default", rec_id, spec.span);
        self.result.import_records[rec_id].meta.insert(ImportRecordMeta::CONTAINS_IMPORT_DEFAULT);
      }
      ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(spec) => {
        let symbol_id = spec.local.expect_symbol_id();
        self.add_star_import(symbol_id, rec_id, spec.span);
        self.result.import_records[rec_id].meta.insert(ImportRecordMeta::CONTAINS_IMPORT_STAR);
      }
    });
  }

  fn scan_module_decl(&mut self, decl: &ModuleDeclaration<'ast>) {
    match decl {
      ast::ModuleDeclaration::ImportDeclaration(decl) => {
        self.esm_import_keyword.get_or_insert(Span::new(decl.span.start, decl.span.start + 6));
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
      ast::ModuleDeclaration::ExportDefaultDeclaration(decl) => {
        self.set_esm_export_keyword(Span::new(decl.span.start, decl.span.start + 6));
        self.scan_export_default_decl(decl);
        match &decl.declaration {
          ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            self.visit_class(class);
            // walk::walk_declaration(self, &ast::Declaration::ClassDeclaration(func));
          }
          _ => {}
        }
      }
      _ => {}
    }
  }

  pub fn add_referenced_symbol(&mut self, sym_ref: SymbolRef) {
    self.current_stmt_info.referenced_symbols.push(sym_ref.into());
  }

  pub fn add_member_expr_reference(
    &mut self,
    object_ref: SymbolRef,
    props: Vec<CompactStr>,
    span: Span,
  ) {
    self
      .current_stmt_info
      .referenced_symbols
      .push(MemberExprRef::new(object_ref, props, span).into());
  }

  fn is_root_symbol(&self, symbol_id: SymbolId) -> bool {
    self.scopes.root_scope_id() == self.result.symbol_ref_db.get_scope_id(symbol_id)
  }

  fn try_diagnostic_forbid_const_assign(&mut self, id_ref: &IdentifierReference) -> Option<()> {
    let ref_id = id_ref.reference_id.get()?;
    let reference = &self.scopes.references[ref_id];
    if reference.is_write() {
      let symbol_id = reference.symbol_id()?;
      if self.result.symbol_ref_db.get_flags(symbol_id).is_const_variable() {
        self.result.errors.push(BuildDiagnostic::forbid_const_assign(
          self.id.to_string(),
          self.source.clone(),
          self.result.symbol_ref_db.get_name(symbol_id).into(),
          self.result.symbol_ref_db.get_span(symbol_id),
          id_ref.span(),
        ));
      }
    }
    None
  }

  /// return a `Some(SymbolRef)` if the identifier referenced a top level `IdentBinding`
  fn resolve_identifier_reference(
    &mut self,
    ident: &IdentifierReference,
  ) -> IdentifierReferenceKind {
    match self.resolve_symbol_from_reference(ident) {
      Some(symbol_id) => {
        if self.is_root_symbol(symbol_id) {
          IdentifierReferenceKind::Root((self.idx, symbol_id).into())
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
  ) -> Option<(Span, Vec<CompactStr>)> {
    let mut span = SPAN;
    let mut props = vec![];
    for ancestor_ast in self.visit_path.iter().rev().take(max_len) {
      match ancestor_ast {
        AstKind::MemberExpression(MemberExpression::StaticMemberExpression(expr)) => {
          span = ancestor_ast.span();
          props.push(expr.property.name.as_str().into());
        }
        AstKind::MemberExpression(MemberExpression::ComputedMemberExpression(expr)) => {
          if let Some(name) = expr.static_property_name() {
            span = ancestor_ast.span();
            props.push(name.into());
          } else {
            break;
          }
        }
        _ => break,
      }
    }
    (!props.is_empty()).then_some((span, props))
  }

  // `console` in `console.log` is a global reference
  pub fn is_global_identifier_reference(&self, ident: &IdentifierReference) -> bool {
    let symbol_id = self.resolve_symbol_from_reference(ident);
    symbol_id.is_none()
  }

  /// If it is not a top level `this` reference visit position
  pub fn is_this_nested(&self) -> bool {
    self.is_nested_this_inside_class
      || self.scope_stack.iter().any(|scope| {
        scope.map_or(false, |scope| {
          let flags = self.scopes.get_flags(scope);
          flags.contains(ScopeFlags::Function) && !flags.contains(ScopeFlags::Arrow)
        })
      })
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
