use crate::types::TypeParam;
use oxc::ast::ast::TSTypeParameter;
use oxc::ast_visit::Visit;
use std::collections::HashMap;

pub struct TypeParamCollector<'a> {
  params: HashMap<String, usize>,
  _phantom: std::marker::PhantomData<&'a ()>,
}

impl TypeParamCollector<'_> {
  pub fn new() -> Self {
    Self { params: HashMap::new(), _phantom: std::marker::PhantomData }
  }

  pub fn into_params(self) -> Vec<TypeParam> {
    self.params.into_iter().map(|(name, occurrences)| TypeParam { name, occurrences }).collect()
  }
}

impl<'a> Visit<'a> for TypeParamCollector<'a> {
  fn visit_ts_type_parameter(&mut self, node: &TSTypeParameter<'a>) {
    let name = node.name.name.to_string();
    *self.params.entry(name).or_insert(0) += 1;

    if let Some(constraint) = &node.constraint {
      self.visit_ts_type(constraint);
    }
    if let Some(default) = &node.default {
      self.visit_ts_type(default);
    }
  }
}

impl Default for TypeParamCollector<'_> {
  fn default() -> Self {
    Self::new()
  }
}
