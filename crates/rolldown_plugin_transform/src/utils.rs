use rolldown_common::ModuleType;
use rolldown_utils::{clean_url::clean_url, pattern_filter::filter as pattern_filter};

use crate::TransformPlugin;

impl TransformPlugin {
  pub fn filter(&self, id: &str, cwd: &str, module_type: &Option<ModuleType>) -> bool {
    if self.include.is_empty() && self.exclude.is_empty() {
      return matches!(module_type, Some(ModuleType::Jsx | ModuleType::Tsx | ModuleType::Ts));
    }

    if pattern_filter(Some(&self.exclude), Some(&self.include), id, cwd).inner() {
      return true;
    }

    let cleaned_id = clean_url(id);
    if cleaned_id != id
      && pattern_filter(Some(&self.exclude), Some(&self.include), cleaned_id, cwd).inner()
    {
      return true;
    }

    !(self.jsx_refresh_include.is_empty() && self.jsx_refresh_exclude.is_empty())
      && pattern_filter(Some(&self.jsx_refresh_exclude), Some(&self.jsx_refresh_include), id, cwd)
        .inner()
  }
}
