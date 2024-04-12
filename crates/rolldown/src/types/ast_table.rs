use index_vec::IndexVec;
use rolldown_common::NormalModuleId;
use rolldown_oxc_utils::OxcAst;

#[derive(Debug, Default)]
pub struct AstTable {
  inner: IndexVec<NormalModuleId, OxcAst>,
}

impl AstTable {
  pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut OxcAst> {
    self.inner.iter_mut()
  }

  pub fn get(&self, index: NormalModuleId) -> &OxcAst {
    &self.inner[index]
  }
}

impl From<IndexVec<NormalModuleId, OxcAst>> for AstTable {
  fn from(inner: IndexVec<NormalModuleId, OxcAst>) -> Self {
    Self { inner }
  }
}
