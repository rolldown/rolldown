use oxc::ast::ast::{TSInferType, TSMappedType, TSTypeParameter};
use oxc::ast_visit::Visit;
use rustc_hash::FxHashMap;

use crate::types::TypeParam;

pub struct TypeParamCollector<'a> {
  params: FxHashMap<String, usize>,
  _phantom: std::marker::PhantomData<&'a ()>,
}

impl TypeParamCollector<'_> {
  pub fn new() -> Self {
    Self { params: FxHashMap::default(), _phantom: std::marker::PhantomData }
  }

  pub fn into_params(self) -> Vec<TypeParam> {
    self.params.into_iter().map(|(name, occurrences)| TypeParam { name, occurrences }).collect()
  }
}

impl<'a> Visit<'a> for TypeParamCollector<'a> {
  // Babel only collects declaration `typeParameters`. Skip registering `infer U` /
  // mapped `K` as params (they'd otherwise enter the deps-fn and break rename).
  fn visit_ts_infer_type(&mut self, node: &TSInferType<'a>) {
    if let Some(constraint) = &node.type_parameter.constraint {
      self.visit_ts_type(constraint);
    }
    if let Some(default) = &node.type_parameter.default {
      self.visit_ts_type(default);
    }
  }

  fn visit_ts_mapped_type(&mut self, node: &TSMappedType<'a>) {
    self.visit_ts_type(&node.constraint);
    if let Some(name_type) = &node.name_type {
      self.visit_ts_type(name_type);
    }
    if let Some(type_ann) = &node.type_annotation {
      self.visit_ts_type(type_ann);
    }
  }

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
