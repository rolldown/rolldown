use std::borrow::Cow;

use oxc::{
  ast::{
    AstKind,
    ast::{
      BindingIdentifier, Declaration, ExportAllDeclaration, ExportDefaultDeclaration,
      ExportNamedDeclaration, IdentifierReference, ImportDeclaration, ImportDeclarationSpecifier,
      MemberExpression, ModuleDeclaration, Program,
    },
  },
  ast_visit::Visit,
  semantic::{Scoping, SymbolId},
  span::{CompactStr, GetSpan, SPAN, Span},
};
use oxc_index::IndexVec;
use rolldown_common::{
  ImportKind, ImportRecordIdx, ImportRecordMeta, LocalExport, MemberExprRef, ModuleId, ModuleIdx,
  NamedImport, RawImportRecord, Specifier, StmtInfo, StmtInfos, SymbolRef, SymbolRefDbForModule,
};
use rolldown_ecmascript::ToSourceString;
use rolldown_ecmascript_utils::{BindingIdentifierExt, BindingPatternExt};
use rolldown_rstr::Rstr;
use rolldown_std_utils::{OptionExt, PathExt};
use rolldown_utils::{concat_string, ecmascript::legitimize_identifier_name, indexmap::FxIndexMap};
use rustc_hash::FxHashMap;
use sugar_path::SugarPath;

pub struct DtsAstScanner<'a> {
  pub named_imports: FxIndexMap<SymbolRef, NamedImport>,
  pub named_exports: FxHashMap<Rstr, LocalExport>,
  pub stmt_infos: StmtInfos,
  pub import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub symbol_ref_db: SymbolRefDbForModule,
  pub module_idx: ModuleIdx,
  pub default_export_ref: SymbolRef,
  pub namespace_object_ref: SymbolRef,
  pub current_stmt_info: StmtInfo,
  pub visit_path: Vec<AstKind<'a>>,
}

impl DtsAstScanner<'_> {
  pub fn new(scoping: Scoping, module_idx: ModuleIdx, module_id: &ModuleId) -> Self {
    let root_scope_id = scoping.root_scope_id();

    let mut symbol_ref_db = SymbolRefDbForModule::new(scoping, module_idx, root_scope_id);

    let repr_name = module_id.as_path().representative_file_name();
    let legitimized_repr_name = legitimize_identifier_name(&repr_name);

    let default_export_ref = symbol_ref_db
      .create_facade_root_symbol_ref(&concat_string!(legitimized_repr_name, "_default"));

    let name = concat_string!(legitimized_repr_name, "_exports");
    let namespace_object_ref = symbol_ref_db.create_facade_root_symbol_ref(&name);

    Self {
      namespace_object_ref,
      current_stmt_info: StmtInfo::default(),
      symbol_ref_db,
      named_imports: FxIndexMap::default(),
      named_exports: FxHashMap::default(),
      import_records: IndexVec::new(),
      module_idx,
      stmt_infos: StmtInfos::default(),
      default_export_ref,
      visit_path: vec![],
    }
  }

  fn scan_module_decl(&mut self, decl: &ModuleDeclaration<'_>) {
    match decl {
      ModuleDeclaration::ImportDeclaration(decl) => {
        self.scan_import_decl(decl);
      }
      ModuleDeclaration::ExportAllDeclaration(decl) => {
        self.scan_export_all_decl(decl);
      }
      ModuleDeclaration::ExportNamedDeclaration(decl) => {
        self.scan_export_named_decl(decl);
      }
      ModuleDeclaration::ExportDefaultDeclaration(decl) => {
        self.scan_export_default_decl(decl);
      }
      _ => {}
    }
  }

  fn scan_import_decl(&mut self, decl: &ImportDeclaration) {
    let Some(specifiers) = &decl.specifiers else { return };
    let rec_id = self.add_import_record(
      decl.source.value.as_str(),
      ImportKind::Import,
      decl.source.span(),
      ImportRecordMeta::empty(),
    );
    specifiers.iter().for_each(|spec| match spec {
      ImportDeclarationSpecifier::ImportSpecifier(spec) => {
        let sym = spec.local.expect_symbol_id();
        let imported = spec.imported.name();
        self.add_named_import(sym, imported.as_str(), rec_id, spec.imported.span());
      }
      ImportDeclarationSpecifier::ImportDefaultSpecifier(spec) => {
        self.add_named_import(spec.local.expect_symbol_id(), "default", rec_id, spec.span);
      }
      ImportDeclarationSpecifier::ImportNamespaceSpecifier(spec) => {
        let symbol_id = spec.local.expect_symbol_id();
        self.add_star_import(symbol_id, rec_id, spec.span);
      }
    });
  }

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
    let namespace_ref: SymbolRef = self
      .symbol_ref_db
      .create_facade_root_symbol_ref(&concat_string!("#LOCAL_NAMESPACE_IN_", module_request, "#"));
    let rec = RawImportRecord::new(
      Rstr::from(module_request),
      kind,
      namespace_ref,
      span,
      None,
      // The first index stmt is reserved for the facade statement that constructs Module Namespace
      // Object
      None,
    )
    .with_meta(init_meta);

    self.import_records.push(rec)
  }

  fn add_named_import(
    &mut self,
    local: SymbolId,
    imported: &str,
    record_id: ImportRecordIdx,
    span_imported: Span,
  ) {
    self.named_imports.insert(
      (self.module_idx, local).into(),
      NamedImport {
        imported: Rstr::new(imported).into(),
        imported_as: (self.module_idx, local).into(),
        span_imported,
        record_id,
      },
    );
  }

  fn add_star_import(&mut self, local: SymbolId, record_id: ImportRecordIdx, span_imported: Span) {
    self.named_imports.insert(
      (self.module_idx, local).into(),
      NamedImport {
        imported: Specifier::Star,
        imported_as: (self.module_idx, local).into(),
        record_id,
        span_imported,
      },
    );
  }

  fn scan_export_all_decl(&mut self, decl: &ExportAllDeclaration) {
    let id = self.add_import_record(
      decl.source.value.as_str(),
      ImportKind::Import,
      decl.source.span(),
      ImportRecordMeta::empty(),
    );
    if let Some(exported) = &decl.exported {
      // export * as ns from '...'
      self.add_star_re_export(exported.name().as_str(), id, decl.span);
    } else {
      // export * from '...'
      self.import_records[id].meta.insert(ImportRecordMeta::IS_EXPORT_STAR);
    }
  }

  fn scan_export_named_decl(&mut self, decl: &ExportNamedDeclaration) {
    if let Some(source) = &decl.source {
      if decl.specifiers.is_empty() {
        return;
      }
      let record_id = self.add_import_record(
        source.value.as_str(),
        ImportKind::Import,
        source.span(),
        ImportRecordMeta::empty(),
      );
      decl.specifiers.iter().for_each(|spec| {
        self.add_re_export(
          spec.exported.name().as_str(),
          spec.local.name().as_str(),
          record_id,
          spec.local.span(),
        );
      });
    } else {
      decl.specifiers.iter().for_each(|spec| {
        if let Some(local_symbol_id) = self.get_root_binding(spec.local.name().as_str()) {
          self.add_local_export(spec.exported.name().as_str(), local_symbol_id, spec.span);
        } else {
          // BuildDiagnostic::export_undefined_variable
        }
      });
      if let Some(decl) = decl.declaration.as_ref() {
        match decl {
          Declaration::VariableDeclaration(var_decl) => {
            var_decl.declarations.iter().for_each(|decl| {
              decl.id.binding_identifiers().into_iter().for_each(|id| {
                self.add_local_export(&id.name, id.expect_symbol_id(), id.span);
              });
            });
          }
          Declaration::FunctionDeclaration(fn_decl) => {
            let id = fn_decl.id.as_ref().unwrap();
            self.add_local_export(id.name.as_str(), id.expect_symbol_id(), id.span);
          }
          Declaration::ClassDeclaration(cls_decl) => {
            let id = cls_decl.id.as_ref().unwrap();
            self.add_local_export(id.name.as_str(), id.expect_symbol_id(), id.span);
          }
          _ => todo!("doesn't support ts now"),
        }
      }
    }
  }

  fn add_star_re_export(
    &mut self,
    export_name: &str,
    record_id: ImportRecordIdx,
    span_for_export_name: Span,
  ) {
    let generated_imported_as_ref = self
      .symbol_ref_db
      .create_facade_root_symbol_ref(legitimize_identifier_name(export_name).as_ref());
    self.current_stmt_info.declared_symbols.push(generated_imported_as_ref);
    let name_import = NamedImport {
      imported: Specifier::Star,
      span_imported: span_for_export_name,
      imported_as: generated_imported_as_ref,
      record_id,
    };

    self.import_records[record_id].meta.insert(ImportRecordMeta::CONTAINS_IMPORT_STAR);
    self.named_exports.insert(
      export_name.into(),
      LocalExport { referenced: generated_imported_as_ref, span: name_import.span_imported },
    );
    self.named_imports.insert(generated_imported_as_ref, name_import);
  }

  fn add_local_export(&mut self, export_name: &str, local: SymbolId, span: Span) {
    let symbol_ref: SymbolRef = (self.module_idx, local).into();
    self.named_exports.insert(export_name.into(), LocalExport { referenced: symbol_ref, span });
  }

  fn add_local_default_export(&mut self, local: SymbolId, span: Span) {
    let symbol_ref: SymbolRef = (self.module_idx, local).into();
    self.named_exports.insert("default".into(), LocalExport { referenced: symbol_ref, span });
  }

  fn add_re_export(
    &mut self,
    export_name: &str,
    imported: &str,
    record_id: ImportRecordIdx,
    span_imported: Span,
  ) {
    // We will pretend `export { [imported] as [export_name] }` to be `import `
    let ident = if export_name == "default" {
      let importee_repr =
        self.import_records[record_id].module_request.as_path().representative_file_name();
      let importee_repr = legitimize_identifier_name(&importee_repr);
      Cow::Owned(concat_string!(importee_repr, "_default"))
    } else {
      // the export_name could be a string literal
      legitimize_identifier_name(export_name)
    };
    let generated_imported_as_ref =
      self.symbol_ref_db.create_facade_root_symbol_ref(ident.as_ref());

    self.current_stmt_info.declared_symbols.push(generated_imported_as_ref);
    let name_import = NamedImport {
      imported: imported.into(),
      imported_as: generated_imported_as_ref,
      record_id,
      span_imported,
    };
    self.named_exports.insert(
      export_name.into(),
      LocalExport { referenced: generated_imported_as_ref, span: name_import.span_imported },
    );
    self.named_imports.insert(generated_imported_as_ref, name_import);
  }

  fn get_root_binding(&self, name: &str) -> Option<SymbolId> {
    self.symbol_ref_db.get_root_binding(name)
  }

  fn scan_export_default_decl(&mut self, decl: &ExportDefaultDeclaration) {
    use oxc::ast::ast::ExportDefaultDeclarationKind;
    let local_binding_for_default_export = match &decl.declaration {
      oxc::ast::match_expression!(ExportDefaultDeclarationKind) => None,
      ExportDefaultDeclarationKind::FunctionDeclaration(fn_decl) => fn_decl
        .id
        .as_ref()
        .map(|id| (rolldown_ecmascript_utils::BindingIdentifierExt::expect_symbol_id(id), id.span)),
      ExportDefaultDeclarationKind::ClassDeclaration(cls_decl) => cls_decl
        .id
        .as_ref()
        .map(|id| (rolldown_ecmascript_utils::BindingIdentifierExt::expect_symbol_id(id), id.span)),
      ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => todo!(),
    };

    let (reference, span) =
      local_binding_for_default_export.unwrap_or((self.default_export_ref.symbol, Span::default()));

    self.add_declared_id(reference);
    self.add_local_default_export(reference, span);
  }

  fn add_declared_id(&mut self, id: SymbolId) {
    self.current_stmt_info.declared_symbols.push((self.module_idx, id).into());
  }

  pub fn add_referenced_symbol(&mut self, sym_ref: SymbolRef) {
    self.current_stmt_info.referenced_symbols.push(sym_ref.into());
  }

  fn is_root_symbol(&self, symbol_id: SymbolId) -> bool {
    self.symbol_ref_db.root_scope_id() == self.symbol_ref_db.symbol_scope_id(symbol_id)
  }

  fn resolve_symbol_from_reference(&self, id_ref: &IdentifierReference) -> Option<SymbolId> {
    let ref_id = id_ref.reference_id.get().unwrap_or_else(|| {
      panic!(
        "{id_ref:#?} must have reference id in code```\n{}\n```\n",
        self.current_stmt_info.unwrap_debug_label()
      )
    });
    self.symbol_ref_db.ast_scopes.symbol_id_for(ref_id)
  }

  fn resolve_identifier_reference(&self, ident: &IdentifierReference) -> IdentifierReferenceKind {
    match self.resolve_symbol_from_reference(ident) {
      Some(symbol_id) => {
        if self.is_root_symbol(symbol_id) {
          IdentifierReferenceKind::Root((self.module_idx, symbol_id).into())
        } else {
          IdentifierReferenceKind::Other
        }
      }
      None => IdentifierReferenceKind::Global,
    }
  }

  fn process_identifier_ref_by_scope(&mut self, ident_ref: &IdentifierReference) {
    match self.resolve_identifier_reference(ident_ref) {
      IdentifierReferenceKind::Root(root_symbol_id) => {
        // if the identifier_reference is a NamedImport MemberExpr access, we store it as a `MemberExpr`
        // use this flag to avoid insert it as `Symbol` at the same time.
        let mut is_inserted_before = false;
        if self.named_imports.contains_key(&root_symbol_id) {
          if let Some((span, props)) = self.try_extract_parent_static_member_expr_chain(usize::MAX)
          {
            if !span.is_unspanned() {
              is_inserted_before = true;
              self.add_member_expr_reference(root_symbol_id, props, span);
            }
          }
        }
        if !is_inserted_before {
          self.add_referenced_symbol(root_symbol_id);
        }
      }
      _ => {}
    };
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

impl<'a> Visit<'a> for DtsAstScanner<'a> {
  fn enter_node(&mut self, kind: oxc::ast::AstKind<'a>) {
    self.visit_path.push(kind);
  }

  fn leave_node(&mut self, _: oxc::ast::AstKind<'a>) {
    self.visit_path.pop();
  }

  fn visit_module_declaration(&mut self, decl: &ModuleDeclaration<'a>) {
    self.scan_module_decl(decl);
  }

  fn visit_binding_identifier(&mut self, ident: &BindingIdentifier<'a>) {
    let symbol_id = ident.symbol_id.get().unpack();
    if self.is_root_symbol(symbol_id) {
      self.add_declared_id(symbol_id);
    }
  }

  fn visit_identifier_reference(&mut self, ident: &IdentifierReference<'a>) {
    self.process_identifier_ref_by_scope(ident);
  }

  fn visit_program(&mut self, program: &Program<'a>) {
    for (idx, stmt) in program.body.iter().enumerate() {
      self.current_stmt_info.stmt_idx = Some(idx.into());
      #[cfg(debug_assertions)]
      {
        self.current_stmt_info.debug_label = Some(stmt.to_source_string());
      }

      self.visit_statement(stmt);
      self.stmt_infos.add_stmt_info(std::mem::take(&mut self.current_stmt_info));
    }
  }
}
