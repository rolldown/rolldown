use rolldown_rstr::Rstr;
use rustc_hash::FxHashSet;

bitflags::bitflags! {
  #[derive(Default, Copy, Clone, Debug)]
  pub struct UsedInfo: u8 {
    // If the module is used as a namespace
    const USED_AS_NAMESPACE = 1 << 1;
    const INCLUDED_AS_NAMESPACE = 1 << 2;
    const HAS_COMMONJS_CANINOCAL_EXPORTS = 1 << 3;
  }
}

#[derive(Default, Clone, Debug)]
pub struct UsedExportsInfo {
  pub used_exports: FxHashSet<Rstr>,
  pub used_info: UsedInfo,
}
