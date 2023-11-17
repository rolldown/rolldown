use index_vec::IndexVec;
use oxc::{
  ast::{
    ast::{
      ExportAllDeclaration, ExportDefaultDeclaration, ExportNamedDeclaration, IdentifierReference,
      ImportDeclaration, ModuleDeclaration, Program,
    },
    Visit,
  },
  semantic::SymbolId,
  span::{Atom, Span},
};
use rolldown_common::{
  ExportsKind, ImportKind, ImportRecordId, LocalExport, LocalOrReExport, ModuleId, ModuleType,
  NamedImport, RawImportRecord, ReExport, Specifier, StmtInfo, StmtInfos, SymbolRef,
};
use rolldown_oxc::{BindingIdentifierExt, BindingPatternExt};
use rustc_hash::FxHashMap;

use crate::bundler::utils::{ast_scope::AstScope, ast_symbol::AstSymbol};

#[derive(Debug, Default)]
pub struct ScanResult {
  pub repr_name: String,
  pub named_imports: FxHashMap<SymbolId, NamedImport>,
  pub named_exports: FxHashMap<Atom, LocalOrReExport>,
  pub stmt_infos: StmtInfos,
  pub import_records: IndexVec<ImportRecordId, RawImportRecord>,
  pub star_exports: Vec<ImportRecordId>,
  pub export_default_symbol_id: Option<SymbolId>,
  pub imports: FxHashMap<Span, ImportRecordId>,
  pub exports_kind: ExportsKind,
}

pub struct Scanner<'a> {
  idx: ModuleId,
  module_type: ModuleType,
  scope: &'a AstScope,
  symbol_table: &'a mut AstSymbol,
  current_stmt_info: StmtInfo,
  result: ScanResult,
  esm_export_keyword: Option<Span>,
  cjs_export_keyword: Option<Span>,
  pub namespace_symbol: SymbolRef,
}

impl<'ast> Scanner<'ast> {
  pub fn new(
    idx: ModuleId,
    scope: &'ast AstScope,
    symbol_table: &'ast mut AstSymbol,
    repr_name: String,
    module_type: ModuleType,
  ) -> Self {
    let mut result = ScanResult::default();
    let name = format!("{repr_name}_ns");
    let namespace_ref: SymbolRef =
      (idx, symbol_table.create_symbol(name.into(), scope.root_scope_id())).into();
    result.repr_name = repr_name;
    // The first StmtInfo is to represent the namespace binding.
    result
      .stmt_infos
      .add_stmt_info(StmtInfo { declared_symbols: vec![namespace_ref], ..Default::default() });
    Self {
      idx,
      scope,
      symbol_table,
      current_stmt_info: StmtInfo::default(),
      result,
      esm_export_keyword: None,
      cjs_export_keyword: None,
      module_type,
      namespace_symbol: namespace_ref,
    }
  }

  pub fn scan(mut self, program: &Program<'ast>) -> ScanResult {
    self.visit_program(program);
    self.result
  }

  fn set_esm_export_keyword(&mut self, span: Span) {
    if self.esm_export_keyword.is_none() {
      self.esm_export_keyword = Some(span);
    }
  }

  fn is_unresolved_reference(&self, ident_ref: &IdentifierReference) -> bool {
    self.scope.is_unresolved(ident_ref.reference_id.get().unwrap())
  }

  fn set_cjs_export_keyword(&mut self, span: Span) {
    if self.cjs_export_keyword.is_none() {
      self.cjs_export_keyword = Some(span);
    }
  }

  fn set_exports_kind(&mut self) {
    if self.esm_export_keyword.is_some() {
      self.result.exports_kind = ExportsKind::Esm;
    } else if self.cjs_export_keyword.is_some() {
      self.result.exports_kind = ExportsKind::CommonJs;
    } else if self.module_type.is_esm() {
      self.result.exports_kind = ExportsKind::Esm;
    } else if self.module_type.is_commonjs() {
      self.result.exports_kind = ExportsKind::CommonJs;
    } else {
      self.result.exports_kind = ExportsKind::Esm;
    }
  }

  fn add_declared_id(&mut self, id: SymbolId) {
    self.current_stmt_info.declared_symbols.push((self.idx, id).into());
  }

  fn get_root_binding(&self, name: &Atom) -> SymbolId {
    self.scope.get_root_binding(name).expect("must have")
  }

  fn add_import_record(&mut self, module_request: &Atom, kind: ImportKind) -> ImportRecordId {
    let rec = RawImportRecord::new(module_request.clone(), kind);
    self.result.import_records.push(rec)
  }

  fn add_named_import(&mut self, local: SymbolId, imported: &Atom, record_id: ImportRecordId) {
    self.result.named_imports.insert(
      local,
      NamedImport {
        imported: imported.clone().into(),
        imported_as: (self.idx, local).into(),
        record_id,
      },
    );
  }

  fn add_star_import(&mut self, local: SymbolId, record_id: ImportRecordId) {
    self.result.import_records[record_id].is_import_namespace = true;
    self.result.named_imports.insert(
      local,
      NamedImport { imported: Specifier::Star, imported_as: (self.idx, local).into(), record_id },
    );
  }

  fn add_local_export(&mut self, export_name: &Atom, local: SymbolId) {
    self.result.named_exports.insert(
      export_name.clone(),
      LocalOrReExport::Local(LocalExport { referenced: (self.idx, local).into() }),
    );
  }

  fn add_local_default_export(&mut self, local: SymbolId) {
    self.result.export_default_symbol_id = Some(local);
    self.result.named_exports.insert(
      Atom::new_inline("default"),
      LocalOrReExport::Local(LocalExport { referenced: (self.idx, local).into() }),
    );
  }

  fn add_re_export(&mut self, export_name: &Atom, imported: &Atom, record_id: ImportRecordId) {
    self.result.named_exports.insert(
      export_name.clone(),
      LocalOrReExport::Re(ReExport { imported: imported.clone().into(), record_id }),
    );
  }

  fn add_star_re_export(&mut self, export_name: &Atom, record_id: ImportRecordId) {
    self.result.import_records[record_id].is_import_namespace = true;
    self.result.named_exports.insert(
      export_name.clone(),
      LocalOrReExport::Re(ReExport { imported: Specifier::Star, record_id }),
    );
  }

  fn scan_export_all_decl(&mut self, decl: &ExportAllDeclaration) {
    let id = self.add_import_record(&decl.source.value, ImportKind::Import);
    if let Some(exported) = &decl.exported {
      // export * as ns from '...'
      self.add_star_re_export(exported.name(), id);
    } else {
      // export * from '...'
      self.result.star_exports.push(id);
    }
    self.result.imports.insert(decl.span, id);
  }

  fn scan_export_named_decl(&mut self, decl: &ExportNamedDeclaration) {
    if let Some(source) = &decl.source {
      let record_id = self.add_import_record(&source.value, ImportKind::Import);
      decl.specifiers.iter().for_each(|spec| {
        self.add_re_export(spec.exported.name(), spec.local.name(), record_id);
        self.result.imports.insert(decl.span, record_id);
      });
    } else {
      decl.specifiers.iter().for_each(|spec| {
        self.add_local_export(spec.exported.name(), self.get_root_binding(spec.local.name()));
      });
      if let Some(decl) = decl.declaration.as_ref() {
        match decl {
          oxc::ast::ast::Declaration::VariableDeclaration(var_decl) => {
            var_decl.declarations.iter().for_each(|decl| {
              decl.id.binding_identifiers().into_iter().for_each(|id| {
                self.result.named_exports.insert(
                  id.name.clone(),
                  LocalExport { referenced: (self.idx, id.expect_symbol_id()).into() }.into(),
                );
              });
            });
          }
          oxc::ast::ast::Declaration::FunctionDeclaration(fn_decl) => {
            let id = fn_decl.id.as_ref().unwrap();
            // FIXME: remove this line after https://github.com/web-infra-dev/oxc/pull/843 being merged.
            self.add_declared_id(id.expect_symbol_id());
            self.add_local_export(&id.name, id.expect_symbol_id());
          }
          oxc::ast::ast::Declaration::ClassDeclaration(cls_decl) => {
            let id = cls_decl.id.as_ref().unwrap();
            self.add_local_export(&id.name, id.expect_symbol_id());
          }
          _ => unreachable!("doesn't support ts now"),
        }
      }
    }
  }

  // If the reference is a global variable, `None` will be returned.
  fn resolve_symbol_from_reference(&self, id_ref: &IdentifierReference) -> Option<SymbolId> {
    let ref_id = id_ref.reference_id.get().expect("must have reference id");
    self.scope.symbol_id_for(ref_id)
  }
  fn scan_export_default_decl(&mut self, decl: &ExportDefaultDeclaration) {
    let local = match &decl.declaration {
      oxc::ast::ast::ExportDefaultDeclarationKind::Expression(exp) => match exp {
        oxc::ast::ast::Expression::Identifier(id_ref) => self.resolve_symbol_from_reference(id_ref),
        _ => None,
      },
      oxc::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(fn_decl) => {
        fn_decl.id.as_ref().map(rolldown_oxc::BindingIdentifierExt::expect_symbol_id)
      }
      oxc::ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(cls_decl) => {
        cls_decl.id.as_ref().map(rolldown_oxc::BindingIdentifierExt::expect_symbol_id)
      }
      _ => unreachable!(),
    };

    let local = local.unwrap_or_else(|| {
      // For patterns like `export default [expression]`, we need to create
      // a facade Symbol to represent it.
      // Notice: Patterns don't include `export default [identifier]`
      let sym_id = self.symbol_table.create_symbol(
        Atom::from(format!("{}_default", self.result.repr_name)),
        self.scope.root_scope_id(),
      );
      self.add_declared_id(sym_id);
      sym_id
    });
    self.add_local_default_export(local);
  }

  fn scan_import_decl(&mut self, decl: &ImportDeclaration) {
    let id = self.add_import_record(&decl.source.value, ImportKind::Import);
    self.result.imports.insert(decl.span, id);
    let Some(specifiers) = &decl.specifiers else { return };
    specifiers.iter().for_each(|spec| match spec {
      oxc::ast::ast::ImportDeclarationSpecifier::ImportSpecifier(spec) => {
        let sym = spec.local.expect_symbol_id();
        self.add_named_import(sym, spec.imported.name(), id);
      }
      oxc::ast::ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(spec) => {
        self.add_named_import(spec.local.expect_symbol_id(), &Atom::new_inline("default"), id);
      }
      oxc::ast::ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(spec) => {
        self.add_star_import(spec.local.expect_symbol_id(), id);
      }
    });
  }
  fn scan_module_decl(&mut self, decl: &ModuleDeclaration) {
    match decl {
      oxc::ast::ast::ModuleDeclaration::ImportDeclaration(decl) => {
        self.scan_import_decl(decl);
      }
      oxc::ast::ast::ModuleDeclaration::ExportAllDeclaration(decl) => {
        self.set_esm_export_keyword(decl.span);
        self.scan_export_all_decl(decl);
      }
      oxc::ast::ast::ModuleDeclaration::ExportNamedDeclaration(decl) => {
        self.set_esm_export_keyword(decl.span);
        self.scan_export_named_decl(decl);
      }
      oxc::ast::ast::ModuleDeclaration::ExportDefaultDeclaration(decl) => {
        self.set_esm_export_keyword(decl.span);
        self.scan_export_default_decl(decl);
      }
      _ => {}
    }
  }

  pub fn add_referenced_symbol(&mut self, id: SymbolId) {
    self.current_stmt_info.referenced_symbols.push((self.idx, id).into());
  }

  fn is_top_level(&self, symbol_id: SymbolId) -> bool {
    self.scope.root_scope_id() == self.symbol_table.scope_id_for(symbol_id)
  }
}

impl<'ast> Visit<'ast> for Scanner<'ast> {
  #[tracing::instrument(skip_all)]
  fn visit_program(&mut self, program: &oxc::ast::ast::Program<'ast>) {
    for (idx, stmt) in program.body.iter().enumerate() {
      self.current_stmt_info.stmt_idx = Some(idx);
      self.visit_statement(stmt);
      self.result.stmt_infos.add_stmt_info(std::mem::take(&mut self.current_stmt_info));
    }
    self.set_exports_kind();
  }

  fn visit_binding_identifier(&mut self, ident: &oxc::ast::ast::BindingIdentifier) {
    let symbol_id = ident.symbol_id.get().unwrap();
    if self.is_top_level(symbol_id) {
      self.add_declared_id(symbol_id);
    }
  }

  fn visit_identifier_reference(&mut self, ident: &IdentifierReference) {
    let symbol_id = self.resolve_symbol_from_reference(ident);
    match symbol_id {
      Some(symbol_id) if self.is_top_level(symbol_id) => {
        self.add_referenced_symbol(symbol_id);
      }
      _ => {}
    }
    if ident.name == "module" || ident.name == "exports" {
      if let Some(refs) = self.scope.root_unresolved_references().get(&ident.name) {
        if refs.iter().any(|r| (*r).eq(&ident.reference_id.get().unwrap())) {
          self.set_cjs_export_keyword(ident.span);
        }
      }
    }
  }

  fn visit_statement(&mut self, stmt: &oxc::ast::ast::Statement<'ast>) {
    if let oxc::ast::ast::Statement::ModuleDeclaration(decl) = stmt {
      self.scan_module_decl(decl.0);
    }
    self.visit_statement_match(stmt);
  }

  fn visit_import_expression(&mut self, expr: &oxc::ast::ast::ImportExpression<'ast>) {
    if let oxc::ast::ast::Expression::StringLiteral(request) = &expr.source {
      let id = self.add_import_record(&request.value, ImportKind::DynamicImport);
      self.result.imports.insert(expr.span, id);
    }
  }

  fn visit_call_expression(&mut self, expr: &oxc::ast::ast::CallExpression<'ast>) {
    match &expr.callee {
      oxc::ast::ast::Expression::Identifier(ident)
        if ident.name == "require" && self.is_unresolved_reference(ident) =>
      {
        if let Some(oxc::ast::ast::Argument::Expression(
          oxc::ast::ast::Expression::StringLiteral(request),
        )) = &expr.arguments.get(0)
        {
          let id = self.add_import_record(&request.value, ImportKind::Require);
          self.result.imports.insert(expr.span, id);
        }
      }
      _ => {}
    }
    for arg in &expr.arguments {
      self.visit_argument(arg);
    }
    self.visit_expression(&expr.callee);
  }
}
