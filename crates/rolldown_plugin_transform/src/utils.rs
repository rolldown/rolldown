use std::path::Path;

use rolldown_common::ModuleType;
use rolldown_plugin::SharedTransformPluginContext;
use rolldown_utils::{clean_url::clean_url, pattern_filter::filter as pattern_filter};
use sugar_path::SugarPath;

use crate::TransformPlugin;

impl TransformPlugin {
  pub fn filter(
    &self,
    ctx: &SharedTransformPluginContext,
    id: &str,
    module_type: &ModuleType,
  ) -> bool {
    if self.include.is_empty() && self.exclude.is_empty() {
      return matches!(module_type, ModuleType::Jsx | ModuleType::Tsx | ModuleType::Ts);
    }

    let normalized_path = Path::new(id).relative(ctx.inner.cwd());
    let normalized_id = normalized_path.to_string_lossy();
    let cleaned_id = clean_url(&normalized_id);

    let result =
      pattern_filter(Some(&self.exclude), Some(&self.include), id, &normalized_id).inner();
    if cleaned_id == normalized_id {
      result
    } else {
      result || pattern_filter(Some(&self.exclude), Some(&self.include), id, cleaned_id).inner()
    }
  }
}
