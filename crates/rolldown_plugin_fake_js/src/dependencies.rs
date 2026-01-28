use oxc::ast::ast::{
  Class, ExportNamedDeclaration, Expression, ModuleExportName, PropertyKey, TSConditionalType,
  TSImportType, TSImportTypeQualifiedName, TSImportTypeQualifier, TSInterfaceHeritage,
  TSMethodSignature, TSPropertySignature, TSQualifiedName, TSType, TSTypeName, TSTypeParameter,
  TSTypeQuery, TSTypeQueryExprName, TSTypeReference,
};
use oxc::ast_visit::Visit;
use std::collections::HashSet;

#[derive(Debug, Clone)]
#[expect(dead_code)]
pub struct ImportTypeInfo {
  pub source: String,
  pub qualifier: Option<String>,
}

pub struct DependencyCollector<'a> {
  pub deps: HashSet<String>,
  pub import_types: Vec<ImportTypeInfo>,
  bindings: HashSet<String>,
  inferred_stack: Vec<Vec<String>>,
  current_inferred: HashSet<String>,
  _phantom: std::marker::PhantomData<&'a ()>,
}

impl DependencyCollector<'_> {
  pub fn new(bindings: Vec<String>) -> Self {
    Self {
      deps: HashSet::new(),
      import_types: Vec::new(),
      bindings: bindings.into_iter().collect(),
      inferred_stack: Vec::new(),
      current_inferred: HashSet::new(),
      _phantom: std::marker::PhantomData,
    }
  }

  fn is_inferred(&self, name: &str) -> bool {
    self.current_inferred.contains(name)
  }

  fn is_binding(&self, name: &str) -> bool {
    self.bindings.contains(name)
  }

  fn is_builtin(name: &str) -> bool {
    matches!(
      name,
      "String"
        | "Number"
        | "Boolean"
        | "Array"
        | "Object"
        | "Function"
        | "Promise"
        | "Record"
        | "Partial"
        | "Required"
        | "Readonly"
        | "Pick"
        | "Omit"
        | "Exclude"
        | "Extract"
        | "NonNullable"
        | "ReturnType"
        | "InstanceType"
        | "ThisType"
        | "Parameters"
        | "ConstructorParameters"
        | "Awaited"
    )
  }

  fn add_dependency(&mut self, name: String) {
    if name != "this"
      && !self.is_inferred(&name)
      && !self.is_binding(&name)
      && !Self::is_builtin(&name)
    {
      self.deps.insert(name);
    }
  }

  fn extract_type_name(type_name: &TSTypeName) -> Option<String> {
    match type_name {
      TSTypeName::IdentifierReference(id) => Some(id.name.to_string()),
      TSTypeName::QualifiedName(qualified) => Self::extract_qualified_name_root(qualified),
      TSTypeName::ThisExpression(_) => None,
    }
  }

  fn extract_qualified_name_root(qualified: &TSQualifiedName) -> Option<String> {
    let mut current = qualified;
    loop {
      match &current.left {
        TSTypeName::IdentifierReference(id) => {
          return Some(id.name.to_string());
        }
        TSTypeName::QualifiedName(q) => {
          current = q;
        }
        TSTypeName::ThisExpression(_) => {
          return None;
        }
      }
    }
  }

  fn update_current_inferred(&mut self, include_last: bool) {
    self.current_inferred.clear();
    let stack_len = self.inferred_stack.len();
    let limit = if include_last { stack_len } else { stack_len.saturating_sub(1) };

    for i in 0..limit {
      for name in &self.inferred_stack[i] {
        self.current_inferred.insert(name.clone());
      }
    }
  }

  fn collect_inferred_names(&self, ts_type: &TSType) -> Vec<String> {
    let mut inferred = Vec::new();
    self.collect_inferred_recursive(ts_type, &mut inferred);
    inferred
  }

  #[expect(clippy::self_only_used_in_recursion)]
  fn collect_inferred_recursive(&self, ts_type: &TSType, inferred: &mut Vec<String>) {
    match ts_type {
      TSType::TSInferType(infer) => {
        inferred.push(infer.type_parameter.name.name.to_string());
      }
      TSType::TSUnionType(union) => {
        for t in &union.types {
          self.collect_inferred_recursive(t, inferred);
        }
      }
      TSType::TSIntersectionType(intersection) => {
        for t in &intersection.types {
          self.collect_inferred_recursive(t, inferred);
        }
      }
      TSType::TSConditionalType(cond) => {
        self.collect_inferred_recursive(&cond.extends_type, inferred);
      }
      _ => {}
    }
  }
}

impl<'a> Visit<'a> for DependencyCollector<'a> {
  fn visit_ts_type_reference(&mut self, node: &TSTypeReference<'a>) {
    if let Some(name) = Self::extract_type_name(&node.type_name) {
      self.add_dependency(name);
    }

    if let Some(type_args) = &node.type_arguments {
      for param in &type_args.params {
        self.visit_ts_type(param);
      }
    }
  }

  fn visit_ts_type_query(&mut self, node: &TSTypeQuery<'a>) {
    match &node.expr_name {
      TSTypeQueryExprName::IdentifierReference(id) => {
        self.add_dependency(id.name.to_string());
      }
      TSTypeQueryExprName::QualifiedName(qualified) => {
        if let Some(name) = Self::extract_qualified_name_root(qualified) {
          self.add_dependency(name);
        }
      }
      TSTypeQueryExprName::TSImportType(_) | TSTypeQueryExprName::ThisExpression(_) => {}
    }
  }

  fn visit_ts_conditional_type(&mut self, node: &TSConditionalType<'a>) {
    let inferred = self.collect_inferred_names(&node.extends_type);
    self.inferred_stack.push(inferred);
    self.visit_ts_type(&node.check_type);
    self.visit_ts_type(&node.extends_type);
    self.update_current_inferred(true);
    self.visit_ts_type(&node.true_type);
    self.update_current_inferred(false);
    self.visit_ts_type(&node.false_type);
    self.inferred_stack.pop();
    self.current_inferred.clear();
  }

  fn visit_ts_interface_heritage(&mut self, node: &TSInterfaceHeritage<'a>) {
    match &node.expression {
      Expression::Identifier(id) => {
        self.add_dependency(id.name.to_string());
      }
      Expression::StaticMemberExpression(member) => {
        if let Expression::Identifier(id) = &member.object {
          self.add_dependency(id.name.to_string());
        }
      }
      _ => {}
    }

    if let Some(type_args) = &node.type_arguments {
      for param in &type_args.params {
        self.visit_ts_type(param);
      }
    }
  }

  fn visit_class(&mut self, node: &Class<'a>) {
    if let Some(Expression::Identifier(id)) = &node.super_class {
      self.add_dependency(id.name.to_string());
    }

    for implement in &node.implements {
      if let Some(name) = Self::extract_type_name(&implement.expression) {
        self.add_dependency(name);
      }

      if let Some(type_args) = &implement.type_arguments {
        for param in &type_args.params {
          self.visit_ts_type(param);
        }
      }
    }

    if let Some(type_params) = &node.type_parameters {
      for param in &type_params.params {
        if let Some(constraint) = &param.constraint {
          self.visit_ts_type(constraint);
        }
        if let Some(default) = &param.default {
          self.visit_ts_type(default);
        }
      }
    }

    self.visit_class_body(&node.body);
  }

  fn visit_ts_import_type(&mut self, node: &TSImportType<'a>) {
    let source_value = node.source.value.to_string();

    let qualifier = node.qualifier.as_ref().and_then(|q| match q {
      TSImportTypeQualifier::Identifier(id) => Some(id.name.to_string()),
      TSImportTypeQualifier::QualifiedName(qn) => {
        fn get_leftmost(q: &TSImportTypeQualifiedName) -> Option<String> {
          match &q.left {
            TSImportTypeQualifier::Identifier(id) => Some(id.name.to_string()),
            TSImportTypeQualifier::QualifiedName(inner) => get_leftmost(inner),
          }
        }
        get_leftmost(qn)
      }
    });

    self.import_types.push(ImportTypeInfo { source: source_value, qualifier: qualifier.clone() });

    if let Some(name) = qualifier {
      self.add_dependency(name);
    }

    if let Some(type_args) = &node.type_arguments {
      for param in &type_args.params {
        self.visit_ts_type(param);
      }
    }
  }

  fn visit_ts_type_parameter(&mut self, node: &TSTypeParameter<'a>) {
    if let Some(constraint) = &node.constraint {
      self.visit_ts_type(constraint);
    }
    if let Some(default) = &node.default {
      self.visit_ts_type(default);
    }
  }

  fn visit_export_named_declaration(&mut self, node: &ExportNamedDeclaration<'a>) {
    for specifier in &node.specifiers {
      let name = match &specifier.local {
        ModuleExportName::IdentifierName(id) => id.name.to_string(),
        ModuleExportName::IdentifierReference(id) => id.name.to_string(),
        ModuleExportName::StringLiteral(lit) => lit.value.to_string(),
      };
      self.add_dependency(name);
    }

    if let Some(decl) = &node.declaration {
      self.visit_declaration(decl);
    }
  }

  fn visit_ts_property_signature(&mut self, node: &TSPropertySignature<'a>) {
    if node.computed {
      if let PropertyKey::StaticIdentifier(id) = &node.key {
        self.add_dependency(id.name.to_string());
      }
    }

    if let Some(type_ann) = &node.type_annotation {
      self.visit_ts_type(&type_ann.type_annotation);
    }
  }

  fn visit_ts_method_signature(&mut self, node: &TSMethodSignature<'a>) {
    if node.computed {
      if let PropertyKey::StaticIdentifier(id) = &node.key {
        self.add_dependency(id.name.to_string());
      }
    }

    if let Some(return_type) = &node.return_type {
      self.visit_ts_type(&return_type.type_annotation);
    }

    if let Some(param) = &node.this_param {
      if let Some(type_ann) = &param.type_annotation {
        self.visit_ts_type(&type_ann.type_annotation);
      }
    }

    if let Some(type_params) = &node.type_parameters {
      for param in &type_params.params {
        self.visit_ts_type_parameter(param);
      }
    }
  }
}
