use oxc::ast::ast::{
  Class, ExportNamedDeclaration, Expression, IdentifierReference, MethodDefinition,
  ModuleExportName, PropertyDefinition, PropertyKey, TSConditionalType, TSImportType,
  TSImportTypeQualifiedName, TSImportTypeQualifier, TSInferType, TSInterfaceHeritage,
  TSMethodSignature, TSPropertySignature, TSQualifiedName, TSType, TSTypeName, TSTypeParameter,
  TSTypeQuery, TSTypeQueryExprName, TSTypeReference,
};
use oxc::ast_visit::Visit;
use oxc::span::GetSpan;
use rustc_hash::FxHashSet;

#[derive(Debug, Clone)]
#[expect(dead_code)]
pub struct ImportTypeInfo {
  pub source: String,
  pub qualifier: Option<String>,
}

pub struct DependencyCollector<'a> {
  pub deps: Vec<String>,
  pub dep_refs: Vec<(String, u32, u32)>,
  pub children: Vec<(u32, u32)>,
  pub import_types: Vec<ImportTypeInfo>,
  bindings: FxHashSet<String>,
  dep_spans: FxHashSet<(u32, u32)>,
  inferred_stack: Vec<Vec<String>>,
  current_inferred: FxHashSet<String>,
  _phantom: std::marker::PhantomData<&'a ()>,
}

impl DependencyCollector<'_> {
  pub fn new(bindings: Vec<String>) -> Self {
    Self {
      deps: Vec::new(),
      dep_refs: Vec::new(),
      children: Vec::new(),
      import_types: Vec::new(),
      bindings: bindings.into_iter().collect(),
      dep_spans: FxHashSet::default(),
      inferred_stack: Vec::new(),
      current_inferred: FxHashSet::default(),
      _phantom: std::marker::PhantomData,
    }
  }

  fn consider_child(&mut self, start: u32, end: u32) {
    let key = (start, end);
    if self.dep_spans.insert(key) {
      self.children.push(key);
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
        | "PropertyDescriptor"
        | "PropertyKey"
        | "ClassDecorator"
        | "MethodDecorator"
    )
  }

  fn add_dependency(&mut self, name: String) {
    if name != "this"
      && !self.is_inferred(&name)
      && !self.is_binding(&name)
      && !Self::is_builtin(&name)
      && !self.deps.iter().any(|dep| dep == &name)
    {
      self.deps.push(name);
    }
  }

  fn add_dependency_at(&mut self, name: String, start: u32, end: u32) {
    if name != "this"
      && !self.is_inferred(&name)
      && !self.is_binding(&name)
      && !Self::is_builtin(&name)
    {
      if !self.deps.iter().any(|dep| dep == &name) {
        self.deps.push(name.clone());
      }
      self.dep_refs.push((name, start, end));
      self.dep_spans.insert((start, end));
    }
  }

  fn collect_computed_key_deps(&mut self, key: &PropertyKey<'_>) {
    match key {
      PropertyKey::StaticIdentifier(id) => {
        self.add_dependency_at(id.name.to_string(), id.span.start, id.span.end);
      }
      PropertyKey::Identifier(id) => {
        self.add_dependency_at(id.name.to_string(), id.span.start, id.span.end);
      }
      PropertyKey::StaticMemberExpression(member) => {
        let mut object = &member.object;
        loop {
          match object {
            Expression::Identifier(id) => {
              self.add_dependency_at(id.name.to_string(), id.span.start, id.span.end);
              break;
            }
            Expression::StaticMemberExpression(inner) => {
              object = &inner.object;
            }
            _ => break,
          }
        }
      }
      PropertyKey::StringLiteral(lit) => {
        self.consider_child(lit.span.start, lit.span.end);
      }
      _ => {}
    }
  }

  fn extract_qualified_name_root(qualified: &TSQualifiedName) -> Option<(String, u32, u32)> {
    let mut current = qualified;
    loop {
      match &current.left {
        TSTypeName::IdentifierReference(id) => {
          return Some((id.name.to_string(), id.span.start, id.span.end));
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
      TSType::TSTypeReference(type_ref) => {
        if let Some(type_args) = &type_ref.type_arguments {
          for param in &type_args.params {
            self.collect_inferred_recursive(param, inferred);
          }
        }
      }
      TSType::TSTypeOperatorType(op) => {
        self.collect_inferred_recursive(&op.type_annotation, inferred);
      }
      TSType::TSArrayType(array) => {
        self.collect_inferred_recursive(&array.element_type, inferred);
      }
      _ => {}
    }
  }
}

impl<'a> Visit<'a> for DependencyCollector<'a> {
  // Don't treat `infer U` as a free type ref; scoping is handled via the conditional stack.
  fn visit_ts_infer_type(&mut self, node: &TSInferType<'a>) {
    if let Some(constraint) = &node.type_parameter.constraint {
      self.visit_ts_type(constraint);
    }
    if let Some(default) = &node.type_parameter.default {
      self.visit_ts_type(default);
    }
  }

  // Free value ids are children (span bookkeeping), not deps. Deps come from type-name paths.
  fn visit_identifier_reference(&mut self, node: &IdentifierReference<'a>) {
    let name = node.name.as_str();
    let span = node.span;
    if !self.is_binding(name) && !self.is_inferred(name) && !self.deps.iter().any(|dep| dep == name)
    {
      self.consider_child(span.start, span.end);
    }
  }

  fn visit_ts_type_reference(&mut self, node: &TSTypeReference<'a>) {
    if let TSTypeName::IdentifierReference(id) = &node.type_name {
      self.add_dependency_at(id.name.to_string(), id.span.start, id.span.end);
    } else if let TSTypeName::QualifiedName(qualified) = &node.type_name {
      if let Some((name, start, end)) = Self::extract_qualified_name_root(qualified) {
        self.add_dependency_at(name, start, end);
      }
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
        self.add_dependency_at(id.name.to_string(), id.span.start, id.span.end);
      }
      TSTypeQueryExprName::QualifiedName(qualified) => {
        if let Some((name, start, end)) = Self::extract_qualified_name_root(qualified) {
          self.add_dependency_at(name, start, end);
        }
      }
      TSTypeQueryExprName::TSImportType(_) | TSTypeQueryExprName::ThisExpression(_) => {}
    }
  }

  fn visit_ts_conditional_type(&mut self, node: &TSConditionalType<'a>) {
    let inferred = self.collect_inferred_names(&node.extends_type);
    self.inferred_stack.push(inferred);
    self.update_current_inferred(true);
    self.visit_ts_type(&node.check_type);
    self.visit_ts_type(&node.extends_type);
    self.visit_ts_type(&node.true_type);
    // Drop inferred names before the false branch so outer `U` can still be a dep.
    self.update_current_inferred(false);
    self.visit_ts_type(&node.false_type);
    self.inferred_stack.pop();
    if self.inferred_stack.is_empty() {
      self.current_inferred.clear();
    } else {
      self.update_current_inferred(true);
    }
  }

  fn visit_ts_interface_heritage(&mut self, node: &TSInterfaceHeritage<'a>) {
    match &node.type_name {
      TSTypeName::IdentifierReference(id) => {
        self.add_dependency_at(id.name.to_string(), id.span.start, id.span.end);
      }
      TSTypeName::QualifiedName(qualified) => {
        if let Some((name, start, end)) = Self::extract_qualified_name_root(qualified) {
          self.add_dependency_at(name, start, end);
        }
      }
      TSTypeName::ThisExpression(_) => {}
    }

    if let Some(type_args) = &node.type_arguments {
      for param in &type_args.params {
        self.visit_ts_type(param);
      }
    }
  }

  fn visit_class(&mut self, node: &Class<'a>) {
    if let Some(super_class) = node.heritage_expression() {
      match super_class {
        Expression::Identifier(id) => {
          self.add_dependency_at(id.name.to_string(), id.span.start, id.span.end);
        }
        Expression::StaticMemberExpression(member) => {
          let mut object = &member.object;
          loop {
            match object {
              Expression::Identifier(id) => {
                self.add_dependency_at(id.name.to_string(), id.span.start, id.span.end);
                break;
              }
              Expression::StaticMemberExpression(inner) => {
                object = &inner.object;
              }
              _ => break,
            }
          }
        }
        _ => {}
      }
    }

    for implement in &node.implements {
      if let Some((name, start, end)) = match &implement.expression {
        TSTypeName::IdentifierReference(id) => {
          Some((id.name.to_string(), id.span.start, id.span.end))
        }
        TSTypeName::QualifiedName(qualified) => Self::extract_qualified_name_root(qualified),
        TSTypeName::ThisExpression(_) => None,
      } {
        self.add_dependency_at(name, start, end);
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
      match &specifier.local {
        ModuleExportName::StringLiteral(lit) => {
          self.add_dependency(lit.value.to_string());
        }
        other => {
          let span = other.span();
          self.add_dependency_at(other.name().to_string(), span.start, span.end);
        }
      }
    }
  }

  fn visit_ts_property_signature(&mut self, node: &TSPropertySignature<'a>) {
    if node.computed {
      self.collect_computed_key_deps(&node.key);
    } else if let PropertyKey::StringLiteral(lit) = &node.key {
      self.consider_child(lit.span.start, lit.span.end);
    }

    if let Some(type_ann) = &node.type_annotation {
      self.visit_ts_type(&type_ann.type_annotation);
    }
  }

  fn visit_ts_method_signature(&mut self, node: &TSMethodSignature<'a>) {
    if node.computed {
      self.collect_computed_key_deps(&node.key);
    } else if let PropertyKey::StringLiteral(lit) = &node.key {
      self.consider_child(lit.span.start, lit.span.end);
    }

    if let Some(return_type) = &node.return_type {
      self.visit_ts_type(&return_type.type_annotation);
    }

    if let Some(param) = &node.this_param {
      if let Some(type_ann) = &param.type_annotation {
        self.visit_ts_type(&type_ann.type_annotation);
      }
    }

    for param in &node.params.items {
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

  fn visit_property_definition(&mut self, node: &PropertyDefinition<'a>) {
    if node.computed {
      self.collect_computed_key_deps(&node.key);
    }
    if let Some(type_ann) = &node.type_annotation {
      self.visit_ts_type(&type_ann.type_annotation);
    }
    if let Some(value) = &node.value {
      match value {
        Expression::Identifier(id) => {
          self.add_dependency_at(id.name.to_string(), id.span.start, id.span.end);
        }
        Expression::StaticMemberExpression(member) => {
          let mut object = &member.object;
          loop {
            match object {
              Expression::Identifier(id) => {
                self.add_dependency_at(id.name.to_string(), id.span.start, id.span.end);
                break;
              }
              Expression::StaticMemberExpression(inner) => {
                object = &inner.object;
              }
              _ => break,
            }
          }
        }
        _ => {}
      }
    }
  }

  fn visit_method_definition(&mut self, node: &MethodDefinition<'a>) {
    if node.computed {
      self.collect_computed_key_deps(&node.key);
    }
    if let Some(type_ann) = &node.value.return_type {
      self.visit_ts_type(&type_ann.type_annotation);
    }
    for param in &node.value.params.items {
      if let Some(type_ann) = &param.type_annotation {
        self.visit_ts_type(&type_ann.type_annotation);
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  use oxc::allocator::Allocator;
  use oxc::ast_visit::Visit;

  use crate::parser::TypeScriptParser;

  #[test]
  fn test_typeof_qualified_dep_ref() {
    let source = "export declare const bar: typeof mod.a;";
    let allocator = Allocator::default();
    let parser = TypeScriptParser::new(&allocator);
    let parse = parser.parse(source, "f.d.ts").unwrap();
    let mut collector = DependencyCollector::new(vec!["bar".into()]);
    for stmt in &parse.program.body {
      collector.visit_statement(stmt);
    }
    assert_eq!(collector.deps, vec!["mod"]);
    assert_eq!(collector.dep_refs.len(), 1);
    let (name, start, end) = &collector.dep_refs[0];
    assert_eq!(name, "mod");
    assert_eq!(&source[*start as usize..*end as usize], "mod");
  }

  #[test]
  fn test_infer_false_branch_deps() {
    let source =
      "export type Test<T> = T extends Array<infer U> ? (T extends Array<infer U2> ? U2 : U) : U";
    let allocator = Allocator::default();
    let parser = TypeScriptParser::new(&allocator);
    let parse = parser.parse(source, "f.d.ts").unwrap();
    let mut collector = DependencyCollector::new(vec!["Test".into(), "T".into()]);
    for stmt in &parse.program.body {
      collector.visit_statement(stmt);
    }
    assert_eq!(collector.deps, vec!["U"]);
    assert_eq!(collector.dep_refs.len(), 1);
  }

  #[test]
  fn test_infer_false_branch_deps_multiline() {
    let source = "export type Test<T> =\n  T extends Array<infer U> ? (T extends Array<infer U2> ? U2 : U) : U";
    let allocator = Allocator::default();
    let parser = TypeScriptParser::new(&allocator);
    let parse = parser.parse(source, "f.d.ts").unwrap();
    let mut collector = DependencyCollector::new(vec!["Test".into(), "T".into()]);
    for stmt in &parse.program.body {
      collector.visit_statement(stmt);
    }
    assert_eq!(collector.deps, vec!["U"]);
    assert_eq!(collector.dep_refs.len(), 1);
    let (name, start, end) = &collector.dep_refs[0];
    assert_eq!(name, "U");
    assert_eq!(&source[*start as usize..*end as usize], "U");
  }
}
