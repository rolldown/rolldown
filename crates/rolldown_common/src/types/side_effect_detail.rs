use bitflags::bitflags;
bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
    /// Metadata flags describing a statement's side effects.
    /// Used to determine execution-order sensitivity for runtime wrapper optimization.
    /// e.g. a global variable access may require preserving execution order even if the
    /// statement is otherwise side-effect-free.
    pub struct SideEffectDetail: u8 {
        const GlobalVarAccess = 1;
        const PureCjs = 1 << 1;
        const Unknown = 1 << 2;
    }
}

impl SideEffectDetail {
  #[inline]
  pub fn has_side_effect(&self) -> bool {
    self.intersects(SideEffectDetail::PureCjs | SideEffectDetail::Unknown)
  }
}

impl From<bool> for SideEffectDetail {
  fn from(value: bool) -> Self {
    if value { SideEffectDetail::Unknown } else { SideEffectDetail::empty() }
  }
}
