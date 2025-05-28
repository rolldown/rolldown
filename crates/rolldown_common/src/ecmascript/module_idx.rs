oxc_index::define_index_type! {
    #[derive(Default)]
    pub struct ModuleIdx = u32;
}

/// Preserved `ModuleIdx` that used for representing a module that is not in the module graph.
/// e.g.
/// We need to create a record for this `require()`, so that we could
/// polyfill it in ast finalization.
/// ```js
/// require();
/// ```
/// needs to be rewriten as:
/// ```js
/// import { __require } from 'rolldown-runtime';
/// __require();
/// ```
/// when `platform: 'node'` and `format: 'esm'`
pub const DUMMY_MODULE_IDX: ModuleIdx = ModuleIdx::from_usize_unchecked(u32::MAX as usize);

impl ModuleIdx {
  #[inline]
  pub fn is_dummy(&self) -> bool {
    *self == DUMMY_MODULE_IDX
  }
}
