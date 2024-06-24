use rolldown_rstr::Rstr;
use rustc_hash::FxHashSet;

bitflags::bitflags! {
  #[derive(Default, Copy, Clone, Debug)]
  pub struct UsedInfo: u8 {
    // If the `NormalModule#namespace_object_ref` is used
    const USED_AS_NAMESPACE = 1 << 1;
    // TODO(hyf0): suspicious
    const INCLUDED_AS_NAMESPACE = 1 << 2;
  }
}

#[derive(Default, Clone, Debug)]
pub struct UsedExportsInfo {
  pub used_exports: FxHashSet<Rstr>,
  pub used_info: UsedInfo,
}
