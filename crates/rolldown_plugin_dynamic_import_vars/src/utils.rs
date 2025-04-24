use rolldown_utils::pattern_filter::filter as pattern_filter;

use super::DynamicImportVarsPlugin;

impl DynamicImportVarsPlugin {
  pub fn filter(&self, id: &str, cwd: &str) -> bool {
    if self.include.is_empty() && self.exclude.is_empty() {
      return false;
    }

    let exclude = (!self.exclude.is_empty()).then_some(self.exclude.as_slice());
    let include = (!self.include.is_empty()).then_some(self.include.as_slice());
    pattern_filter(exclude, include, id, cwd).inner()
  }
}
