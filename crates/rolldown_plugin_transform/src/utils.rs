use std::borrow::Cow;

use oxc::{span::SourceType, transformer::TransformOptions};
use rolldown_common::ModuleType;
use rolldown_utils::{clean_url::clean_url, pattern_filter::filter as pattern_filter};

use crate::TransformPlugin;

pub enum JsxRefreshFilter {
  None,
  True,
  False,
}

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

    matches!(self.jsx_refresh_filter(id, cwd), JsxRefreshFilter::True)
  }

  pub fn jsx_refresh_filter(&self, id: &str, cwd: &str) -> JsxRefreshFilter {
    if self.jsx_refresh_include.is_empty() && self.jsx_refresh_exclude.is_empty() {
      return JsxRefreshFilter::None;
    }

    if pattern_filter(Some(&self.jsx_refresh_exclude), Some(&self.jsx_refresh_include), id, cwd)
      .inner()
    {
      return JsxRefreshFilter::True;
    }

    JsxRefreshFilter::False
  }

  pub fn get_modified_transform_options(
    &self,
    id: &str,
    cwd: &str,
    ext: Option<&str>,
  ) -> (SourceType, Cow<'_, TransformOptions>) {
    let is_refresh_disabled = self.transform_options.jsx.refresh.is_some()
      && (self.environment_consumer == "server"
        || matches!(self.jsx_refresh_filter(id, cwd), JsxRefreshFilter::False));

    let is_js_lang = matches!(self.jsx_refresh_filter(id, cwd), JsxRefreshFilter::True)
      && ext.is_some_and(|ext| ["js", "jsx", "ts", "tsx", "mjs"].contains(&ext));

    let source_type = if is_js_lang {
      SourceType::mjs()
    } else if let Some(source_type) = self.source_type {
      source_type
    } else {
      match ext {
        Some("js") => SourceType::mjs(),
        Some("jsx") => SourceType::jsx(),
        Some("ts") => SourceType::ts(),
        Some("tsx") => SourceType::tsx(),
        None | Some(_) => {
          panic!("Failed to detect the lang of {id}. Please specify `transformOptions.lang`")
        }
      }
    };

    let transform_options = if is_refresh_disabled {
      let mut transform_options = self.transform_options.clone();

      if is_refresh_disabled {
        transform_options.jsx.refresh = None;
      }

      Cow::Owned(transform_options)
    } else {
      Cow::Borrowed(&self.transform_options)
    };

    (source_type, transform_options)
  }
}
