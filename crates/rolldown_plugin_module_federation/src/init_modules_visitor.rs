use std::sync::Arc;

use oxc::ast::{
  ast::{ExportAllDeclaration, ExportNamedDeclaration, ImportDeclaration},
  AstBuilder, VisitMut,
};
use rustc_hash::FxHashSet;

use crate::{utils::is_remote_module, ModuleFederationPluginOption};

pub struct InitModuleVisitor<'ast, 'a> {
  #[allow(dead_code)]
  pub ast_builder: AstBuilder<'ast>,
  pub options: &'a ModuleFederationPluginOption,
  pub init_remote_modules: &'a mut FxHashSet<Arc<str>>,
}

impl InitModuleVisitor<'_, '_> {
  #[allow(clippy::too_many_lines)]
  pub fn detect_static_module_decl(&mut self, request: &str) {
    if is_remote_module(request, self.options) && !self.init_remote_modules.contains(request) {
      self.init_remote_modules.insert(request.into());
    }
  }
}

// TODO require/ import()
impl<'ast> VisitMut<'ast> for InitModuleVisitor<'ast, '_> {
  fn visit_import_declaration(&mut self, decl: &mut ImportDeclaration<'ast>) {
    self.detect_static_module_decl(&decl.source.value);
  }

  fn visit_export_all_declaration(&mut self, decl: &mut ExportAllDeclaration<'ast>) {
    self.detect_static_module_decl(&decl.source.value);
  }

  fn visit_export_named_declaration(&mut self, decl: &mut ExportNamedDeclaration<'ast>) {
    if let Some(source) = &decl.source {
      self.detect_static_module_decl(&source.value);
    }
  }
}
