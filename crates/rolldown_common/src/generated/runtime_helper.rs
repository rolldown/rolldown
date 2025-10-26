// Auto-generated code, DO NOT EDIT DIRECTLY!
// To edit this generated file you have to edit `tasks/generator/src/generators/runtime_helper.rs`

use bitflags::bitflags;
bitflags! {
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
  pub struct RuntimeHelper: u32 {
    const Create = 1 << 0;
    const DefProp = 1 << 1;
    const Name = 1 << 2;
    const GetOwnPropDesc = 1 << 3;
    const GetOwnPropNames = 1 << 4;
    const GetProtoOf = 1 << 5;
    const HasOwnProp = 1 << 6;
    const Esm = 1 << 7;
    const EsmMin = 1 << 8;
    const CommonJs = 1 << 9;
    const CommonJsMin = 1 << 10;
    const Export = 1 << 11;
    const CopyProps = 1 << 12;
    const ReExport = 1 << 13;
    const ToEsm = 1 << 14;
    const ToEsmWithSymbols = 1 << 15;
    const ToCommonJs = 1 << 16;
    const ToBinaryNode = 1 << 17;
    const ToBinary = 1 << 18;
    const ToDynamicImportEsm = 1 << 19;
    const Require = 1 << 20;
  }
}

impl RuntimeHelper {
  /// # Use with caution
  /// Only used when there is only one bit is set in the `RuntimeHelper`.
  /// The function is used to get the index of the bit that is set.
  #[inline]
  pub fn bit_index(&self) -> usize {
    self.bits().trailing_zeros() as usize
  }
}
use crate::StmtInfoIdx;
pub type DependedRuntimeHelperMap = [Vec<StmtInfoIdx>; RUNTIME_HELPER_NAMES.len()];
pub trait DependedRuntimeHelperMapExt {
  /// Debug function to print runtime names and their associated statement indices
  fn debug_print(&self);
}
impl DependedRuntimeHelperMapExt for DependedRuntimeHelperMap {
  fn debug_print(&self) {
    eprintln!("DependedRuntimeHelperMap debug:");
    for (idx, stmt_infos) in self.iter().enumerate() {
      if let Some(runtime_name) = RUNTIME_HELPER_NAMES.get(idx) {
        eprintln!("  {runtime_name} (idx: {idx}): {stmt_infos:?}");
      } else {
        eprintln!("  Unknown runtime (idx: {idx}): {stmt_infos:?}");
      }
    }
  }
}

pub const RUNTIME_HELPER_NAMES: [&str; 21] = [
  "__create",
  "__defProp",
  "__name",
  "__getOwnPropDesc",
  "__getOwnPropNames",
  "__getProtoOf",
  "__hasOwnProp",
  "__esm",
  "__esmMin",
  "__commonJS",
  "__commonJSMin",
  "__export",
  "__copyProps",
  "__reExport",
  "__toESM",
  "__toESMWithSymbols",
  "__toCommonJS",
  "__toBinaryNode",
  "__toBinary",
  "__toDynamicImportESM",
  "__require",
];
