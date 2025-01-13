use arcstr::ArcStr;
use dashmap::mapref::one::Ref;
use rolldown_ecmascript::EcmaAst;
use rolldown_utils::dashmap::FxDashMap;

#[derive(Default)]
pub struct Cache {
  ecma_ast: FxDashMap<ArcStr, EcmaAst>,
}

impl Cache {
  pub fn get_ecma_ast(&self, key: &str) -> Option<Ref<'_, ArcStr, EcmaAst>> {
    self.ecma_ast.get(key)
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
}
