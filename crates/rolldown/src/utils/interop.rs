use rolldown_common::{ExportsKind, Module};

pub fn calculate_interop_from_module(module: &Module) -> Option<Interop> {
  match module {
    Module::External(_) => None,
    Module::Ecma(module) => {
      if matches!(module.exports_kind, ExportsKind::CommonJs) {
        if module.def_format.is_esm() {
          Some(Interop::Node)
        } else {
          Some(Interop::Babel)
        }
      } else {
        None
      }
    }
  }
}

pub enum Interop {
  Babel,
  Node,
}
