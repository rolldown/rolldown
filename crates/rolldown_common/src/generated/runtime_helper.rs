// Auto-generated code, DO NOT EDIT DIRECTLY!
// To edit this generated file you have to edit `tasks/generator/src/generators/runtime_helper.rs`

#![expect(clippy::print_stderr)]

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
    const ExportAll = 1 << 11;
    const CopyProps = 1 << 12;
    const ReExport = 1 << 13;
    const ToEsm = 1 << 14;
    const ToCommonJs = 1 << 15;
    const ToBinaryNode = 1 << 16;
    const ToBinary = 1 << 17;
    const Require = 1 << 18;
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

/// Maps each single-bit `RuntimeHelper` to the `StmtInfoIdx`s that depend on it.
#[derive(Debug, Default, Clone)]
pub struct DependedRuntimeHelperMap([Vec<StmtInfoIdx>; RUNTIME_HELPER_NAMES.len()]);

impl DependedRuntimeHelperMap {
  /// Record that `stmt_info_idx` depends on `helper`. `helper` must have exactly one bit set.
  #[inline]
  pub fn push(&mut self, helper: RuntimeHelper, stmt_info_idx: StmtInfoIdx) {
    self.0[helper.bit_index()].push(stmt_info_idx);
  }

  /// Iterate over `(single-bit helper, &statements)` pairs for each defined `RuntimeHelper` bit.
  pub fn iter(&self) -> impl Iterator<Item = (RuntimeHelper, &Vec<StmtInfoIdx>)> {
    self.0.iter().enumerate().map(|(i, v)| {
      // `i` is always within `0..RUNTIME_HELPER_NAMES.len()`, matching a defined single-bit flag.
      (RuntimeHelper::from_bits_truncate(1 << i), v)
    })
  }

  /// Debug function to print runtime names and their associated statement indices
  pub fn debug_print(&self) {
    eprintln!("DependedRuntimeHelperMap debug:");
    for (idx, stmt_infos) in self.0.iter().enumerate() {
      if let Some(runtime_name) = RUNTIME_HELPER_NAMES.get(idx) {
        eprintln!("  {runtime_name} (idx: {idx}): {stmt_infos:?}");
      } else {
        eprintln!("  Unknown runtime (idx: {idx}): {stmt_infos:?}");
      }
    }
  }
}

pub const RUNTIME_HELPER_NAMES: [&str; 19] = [
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
  "__exportAll",
  "__copyProps",
  "__reExport",
  "__toESM",
  "__toCommonJS",
  "__toBinaryNode",
  "__toBinary",
  "__require",
];
