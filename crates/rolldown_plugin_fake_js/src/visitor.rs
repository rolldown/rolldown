use oxc::ast::ast::{
  BindingPattern, Declaration, ExportDefaultDeclaration, ExportDefaultDeclarationKind,
  ExportNamedDeclaration, Program, Statement, TSModuleDeclarationKind, TSModuleDeclarationName,
};
use oxc::ast_visit::Visit;
use oxc::span::Span;

#[derive(Debug, Clone)]
pub struct DeclarationNode {
  #[expect(dead_code)]
  pub kind: DeclarationKind,
  pub bindings: Vec<String>,
  pub span: Span,
  pub is_export: bool,
  pub is_default: bool,
  pub is_side_effect: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum DeclarationKind {
  Function,
  Class,
  Interface,
  TypeAlias,
  Enum,
  Variable,
  Namespace,
}

pub struct DeclarationCollector {
  pub declarations: Vec<DeclarationNode>,
}

impl DeclarationCollector {
  pub fn new() -> Self {
    Self { declarations: Vec::new() }
  }

  fn extract_binding_names(pattern: &BindingPattern) -> Vec<String> {
    match pattern {
      BindingPattern::BindingIdentifier(id) => {
        vec![id.name.to_string()]
      }
      BindingPattern::ObjectPattern(obj) => {
        let mut names = Vec::new();
        for prop in &obj.properties {
          names.extend(Self::extract_binding_names(&prop.value));
        }
        names
      }
      BindingPattern::ArrayPattern(arr) => {
        let mut names = Vec::new();
        for elem in arr.elements.iter().flatten() {
          names.extend(Self::extract_binding_names(elem));
        }
        names
      }
      BindingPattern::AssignmentPattern(assign) => Self::extract_binding_names(&assign.left),
    }
  }
}

impl<'a> Visit<'a> for DeclarationCollector {
  fn visit_program(&mut self, program: &Program<'a>) {
    for stmt in &program.body {
      self.visit_statement(stmt);
    }
  }

  fn visit_export_named_declaration(&mut self, decl: &ExportNamedDeclaration<'a>) {
    if let Some(declaration) = &decl.declaration {
      let is_default = decl.export_kind == oxc::ast::ast::ImportOrExportKind::Value
        && matches!(
          declaration,
          Declaration::TSInterfaceDeclaration(_) | Declaration::TSTypeAliasDeclaration(_)
        );

      self.collect_declaration(declaration, true, is_default);
    }
  }

  fn visit_export_default_declaration(&mut self, decl: &ExportDefaultDeclaration<'a>) {
    match &decl.declaration {
      ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
        let binding = func
          .id
          .as_ref()
          .map(|id| id.name.to_string())
          .unwrap_or_else(|| "export_default".to_string());

        self.declarations.push(DeclarationNode {
          kind: DeclarationKind::Function,
          bindings: vec![binding],
          span: func.span,
          is_export: true,
          is_default: true,
          is_side_effect: false,
        });
      }
      ExportDefaultDeclarationKind::ClassDeclaration(class) => {
        let binding = class
          .id
          .as_ref()
          .map(|id| id.name.to_string())
          .unwrap_or_else(|| "export_default".to_string());

        self.declarations.push(DeclarationNode {
          kind: DeclarationKind::Class,
          bindings: vec![binding],
          span: class.span,
          is_export: true,
          is_default: true,
          is_side_effect: false,
        });
      }
      _ => {}
    }
  }

  fn visit_statement(&mut self, stmt: &Statement<'a>) {
    match stmt {
      Statement::ExportNamedDeclaration(decl) => {
        self.visit_export_named_declaration(decl);
      }
      Statement::ExportDefaultDeclaration(decl) => {
        self.visit_export_default_declaration(decl);
      }
      Statement::TSInterfaceDeclaration(decl) => {
        self.declarations.push(DeclarationNode {
          kind: DeclarationKind::Interface,
          bindings: vec![decl.id.name.to_string()],
          span: decl.span,
          is_export: false,
          is_default: false,
          is_side_effect: false,
        });
      }
      Statement::TSTypeAliasDeclaration(decl) => {
        self.declarations.push(DeclarationNode {
          kind: DeclarationKind::TypeAlias,
          bindings: vec![decl.id.name.to_string()],
          span: decl.span,
          is_export: false,
          is_default: false,
          is_side_effect: false,
        });
      }
      Statement::TSEnumDeclaration(decl) => {
        self.declarations.push(DeclarationNode {
          kind: DeclarationKind::Enum,
          bindings: vec![decl.id.name.to_string()],
          span: decl.span,
          is_export: false,
          is_default: false,
          is_side_effect: false,
        });
      }
      Statement::VariableDeclaration(decl) => {
        let bindings: Vec<String> =
          decl.declarations.iter().flat_map(|d| Self::extract_binding_names(&d.id)).collect();

        if !bindings.is_empty() {
          self.declarations.push(DeclarationNode {
            kind: DeclarationKind::Variable,
            bindings,
            span: decl.span,
            is_export: false,
            is_default: false,
            is_side_effect: false,
          });
        }
      }
      Statement::TSModuleDeclaration(decl) => {
        let binding = match &decl.id {
          TSModuleDeclarationName::Identifier(id) => id.name.to_string(),
          TSModuleDeclarationName::StringLiteral(lit) => lit.value.to_string(),
        };

        let is_side_effect = decl.kind != TSModuleDeclarationKind::Namespace;

        self.declarations.push(DeclarationNode {
          kind: DeclarationKind::Namespace,
          bindings: vec![binding],
          span: decl.span,
          is_export: false,
          is_default: false,
          is_side_effect,
        });
      }
      Statement::FunctionDeclaration(func) => {
        if let Some(id) = &func.id {
          self.declarations.push(DeclarationNode {
            kind: DeclarationKind::Function,
            bindings: vec![id.name.to_string()],
            span: func.span,
            is_export: false,
            is_default: false,
            is_side_effect: false,
          });
        }
      }
      Statement::ClassDeclaration(class) => {
        if let Some(id) = &class.id {
          self.declarations.push(DeclarationNode {
            kind: DeclarationKind::Class,
            bindings: vec![id.name.to_string()],
            span: class.span,
            is_export: false,
            is_default: false,
            is_side_effect: false,
          });
        }
      }
      _ => {}
    }
  }
}

impl DeclarationCollector {
  fn collect_declaration(&mut self, declaration: &Declaration, is_export: bool, is_default: bool) {
    match declaration {
      Declaration::FunctionDeclaration(func) => {
        if let Some(id) = &func.id {
          self.declarations.push(DeclarationNode {
            kind: DeclarationKind::Function,
            bindings: vec![id.name.to_string()],
            span: func.span,
            is_export,
            is_default,
            is_side_effect: false,
          });
        }
      }
      Declaration::ClassDeclaration(class) => {
        if let Some(id) = &class.id {
          self.declarations.push(DeclarationNode {
            kind: DeclarationKind::Class,
            bindings: vec![id.name.to_string()],
            span: class.span,
            is_export,
            is_default,
            is_side_effect: false,
          });
        }
      }
      Declaration::VariableDeclaration(var_decl) => {
        let bindings: Vec<String> =
          var_decl.declarations.iter().flat_map(|d| Self::extract_binding_names(&d.id)).collect();

        if !bindings.is_empty() {
          self.declarations.push(DeclarationNode {
            kind: DeclarationKind::Variable,
            bindings,
            span: var_decl.span,
            is_export,
            is_default,
            is_side_effect: false,
          });
        }
      }
      Declaration::TSInterfaceDeclaration(interface) => {
        self.declarations.push(DeclarationNode {
          kind: DeclarationKind::Interface,
          bindings: vec![interface.id.name.to_string()],
          span: interface.span,
          is_export,
          is_default,
          is_side_effect: false,
        });
      }
      Declaration::TSTypeAliasDeclaration(type_alias) => {
        self.declarations.push(DeclarationNode {
          kind: DeclarationKind::TypeAlias,
          bindings: vec![type_alias.id.name.to_string()],
          span: type_alias.span,
          is_export,
          is_default,
          is_side_effect: false,
        });
      }
      Declaration::TSEnumDeclaration(enum_decl) => {
        self.declarations.push(DeclarationNode {
          kind: DeclarationKind::Enum,
          bindings: vec![enum_decl.id.name.to_string()],
          span: enum_decl.span,
          is_export,
          is_default,
          is_side_effect: false,
        });
      }
      Declaration::TSModuleDeclaration(module) => {
        let binding = match &module.id {
          TSModuleDeclarationName::Identifier(id) => id.name.to_string(),
          TSModuleDeclarationName::StringLiteral(lit) => lit.value.to_string(),
        };

        let is_side_effect = module.kind != TSModuleDeclarationKind::Namespace;

        self.declarations.push(DeclarationNode {
          kind: DeclarationKind::Namespace,
          bindings: vec![binding],
          span: module.span,
          is_export,
          is_default,
          is_side_effect,
        });
      }
      _ => {}
    }
  }
}

impl Default for DeclarationCollector {
  fn default() -> Self {
    Self::new()
  }
}
