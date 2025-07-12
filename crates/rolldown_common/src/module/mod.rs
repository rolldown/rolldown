pub mod external_module;
pub mod normal_module;

use arcstr::ArcStr;
use oxc_index::IndexVec;
use rolldown_std_utils::OptionExt;

use crate::{
  EcmaAstIdx, ExternalModule, ImportRecordIdx, ModuleIdx, NormalModule, ResolvedImportRecord,
};

#[derive(Debug, Clone)]
pub enum Module {
  Normal(Box<NormalModule>),
  External(Box<ExternalModule>),
}

impl Module {
  pub fn idx(&self) -> ModuleIdx {
    match self {
      Module::Normal(v) => v.idx,
      Module::External(v) => v.idx,
    }
  }

  #[inline]
  pub fn exec_order(&self) -> u32 {
    match self {
      Module::Normal(v) => v.exec_order,
      Module::External(v) => v.exec_order,
    }
  }

  pub fn id(&self) -> &str {
    match self {
      Module::Normal(v) => &v.id,
      Module::External(v) => &v.id,
    }
  }

  pub fn id_clone(&self) -> &ArcStr {
    match self {
      Module::Normal(v) => v.id.resource_id(),
      Module::External(v) => &v.id,
    }
  }

  pub fn side_effects(&self) -> &crate::side_effects::DeterminedSideEffects {
    match self {
      Module::Normal(v) => &v.side_effects,
      Module::External(v) => &v.side_effects,
    }
  }

  pub fn stable_id(&self) -> &str {
    match self {
      Module::Normal(v) => &v.stable_id,
      Module::External(v) => &v.name,
    }
  }

  pub fn repr_name(&self) -> &str {
    match self {
      Module::Normal(v) => v.repr_name.as_str(),
      Module::External(v) => v.identifier_name.as_str(),
    }
  }

  pub fn normal(v: NormalModule) -> Self {
    Module::Normal(Box::new(v))
  }

  pub fn external(v: ExternalModule) -> Self {
    Module::External(Box::new(v))
  }

  pub fn as_normal(&self) -> Option<&NormalModule> {
    match self {
      Module::Normal(v) => Some(v),
      Module::External(_) => None,
    }
  }

  pub fn as_external(&self) -> Option<&ExternalModule> {
    match self {
      Module::External(v) => Some(v),
      Module::Normal(_) => None,
    }
  }

  pub fn as_normal_mut(&mut self) -> Option<&mut NormalModule> {
    match self {
      Module::Normal(v) => Some(v),
      Module::External(_) => None,
    }
  }

  pub fn as_external_mut(&mut self) -> Option<&mut ExternalModule> {
    match self {
      Module::External(v) => Some(v),
      Module::Normal(_) => None,
    }
  }

  pub fn import_records(&self) -> &IndexVec<ImportRecordIdx, ResolvedImportRecord> {
    match self {
      Module::Normal(v) => match v.module_type {
        crate::ModuleType::Css => &v.css_view.unpack_ref().import_records,
        _ => &v.ecma_view.import_records,
      },
      Module::External(v) => &v.import_records,
    }
  }

  pub fn set_import_records(&mut self, records: IndexVec<ImportRecordIdx, ResolvedImportRecord>) {
    match self {
      Module::Normal(v) => match v.module_type {
        crate::ModuleType::Css => v.css_view.unpack_ref_mut().import_records = records,
        _ => v.ecma_view.import_records = records,
      },
      Module::External(v) => v.import_records = records,
    }
  }

  pub fn set_ecma_ast_idx(&mut self, idx: EcmaAstIdx) {
    match self {
      Module::Normal(v) => v.ecma_ast_idx = Some(idx),
      Module::External(_) => panic!("set_ecma_ast_idx should be called on EcmaModule"),
    }
  }

  /// Returns `true` if the module is [`Ecma`].
  ///
  /// [`Ecma`]: Module::Ecma
  #[must_use]
  pub fn is_normal(&self) -> bool {
    matches!(self, Self::Normal(..))
  }

  pub fn is_external(&self) -> bool {
    matches!(self, Self::External(..))
  }

  pub fn size(&self) -> usize {
    match self {
      Module::Normal(v) => v.source.len(),
      Module::External(_) => 0,
    }
  }
}

impl From<NormalModule> for Module {
  fn from(module: NormalModule) -> Self {
    Module::Normal(Box::new(module))
  }
}

impl From<ExternalModule> for Module {
  fn from(module: ExternalModule) -> Self {
    Module::External(Box::new(module))
  }
}
