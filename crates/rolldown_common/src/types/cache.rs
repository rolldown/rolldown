use arcstr::ArcStr;
use dashmap::mapref::one::Ref;
use rolldown_ecmascript::EcmaAst;
use rolldown_utils::dashmap::FxDashMap;

use crate::{ModuleType, StrOrBytes};

#[derive(Default)]
pub struct Cache {
  ecma_ast: FxDashMap<ArcStr, EcmaAst>,
  raw_source_and_module_type: FxDashMap<ArcStr, (StrOrBytes, ModuleType)>,
}

impl Cache {
  pub fn get_ecma_ast(&self, key: &str) -> Option<Ref<'_, ArcStr, EcmaAst>> {
    self.ecma_ast.get(key)
  }

  pub fn invalidate(&self, key: &str) {
    self.ecma_ast.remove(key);
    self.raw_source_and_module_type.remove(key);
  }

  pub fn get_source(&self, key: &str) -> Option<ArcStr> {
    let source = self.ecma_ast.get(key).map(|item| {
      let value = item.value();
      value.source().clone()
    });
    source
  }

  pub fn insert_ecma_ast(&self, key: ArcStr, value: EcmaAst) {
    self.ecma_ast.insert(key, value);
  }

  pub fn get_raw_source_and_module_type(
    &self,
    key: &str,
  ) -> Option<Ref<'_, ArcStr, (StrOrBytes, ModuleType)>> {
    self.raw_source_and_module_type.get(key)
  }

  pub fn insert_raw_source_and_module_type(&self, key: ArcStr, value: (StrOrBytes, ModuleType)) {
    self.raw_source_and_module_type.insert(key, value);
  }
}
