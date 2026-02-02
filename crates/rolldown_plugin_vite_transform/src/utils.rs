use std::path::Path;

use itertools::Either;
use memchr::memmem;
use oxc::{span::SourceType, transformer::TransformOptions};
use rolldown_common::{LogWithoutPlugin, ModuleType, merge_tsconfig};
use rolldown_plugin::SharedTransformPluginContext;
use rolldown_utils::{pattern_filter::filter as pattern_filter, url::clean_url};

use super::ViteTransformPlugin;

pub enum JsxRefreshFilter {
  None,
  True,
  False,
}

impl ViteTransformPlugin {
  pub fn filter(&self, id: &str, cwd: &str, module_type: Option<&ModuleType>) -> bool {
    // rollup `createFilter` always skips when id includes null byte
    // https://github.com/rollup/plugins/blob/ad58c8d87c5ab4864e25b5a777290fdf12a3879f/packages/pluginutils/src/createFilter.ts#L51
    if memmem::find(id.as_bytes(), b"\0").is_some() {
      return false;
    }

    if self.include.is_empty() && self.exclude.is_empty() {
      return matches!(module_type, Some(ModuleType::Jsx | ModuleType::Tsx | ModuleType::Ts));
    }

    let exclude = (!self.exclude.is_empty()).then_some(self.exclude.as_slice());
    let include = (!self.include.is_empty()).then_some(self.include.as_slice());

    if pattern_filter(exclude, include, id, cwd).inner() {
      return true;
    }

    let cleaned_id = clean_url(id);
    if cleaned_id != id && pattern_filter(exclude, include, cleaned_id, cwd).inner() {
      return true;
    }

    matches!(self.jsx_refresh_filter(id, cwd), JsxRefreshFilter::True)
  }

  pub fn jsx_refresh_filter(&self, id: &str, cwd: &str) -> JsxRefreshFilter {
    if self.jsx_refresh_include.is_empty() && self.jsx_refresh_exclude.is_empty() {
      return JsxRefreshFilter::None;
    }

    let jsx_refresh_exclude =
      (!self.jsx_refresh_exclude.is_empty()).then_some(self.jsx_refresh_exclude.as_slice());
    let jsx_refresh_include =
      (!self.jsx_refresh_include.is_empty()).then_some(self.jsx_refresh_include.as_slice());

    if pattern_filter(jsx_refresh_exclude, jsx_refresh_include, id, cwd).inner() {
      return JsxRefreshFilter::True;
    }

    JsxRefreshFilter::False
  }

  pub fn get_modified_transform_options(
    &self,
    ctx: &SharedTransformPluginContext,
    id: &str,
    cwd: &str,
    ext: Option<&str>,
    code: &str,
  ) -> anyhow::Result<(SourceType, TransformOptions)> {
    let is_jsx_refresh_lang = matches!(self.jsx_refresh_filter(id, cwd), JsxRefreshFilter::True)
      && ext.is_none_or(|ext| ["js", "jsx", "mjs", "ts", "tsx"].binary_search(&ext).is_err());

    let source_type = if is_jsx_refresh_lang {
      SourceType::mjs()
    } else {
      match ext {
        Some("js" | "cjs" | "mjs") => SourceType::mjs(),
        Some("jsx") => SourceType::jsx(),
        Some("ts" | "cts" | "mts") => SourceType::ts(),
        Some("tsx") => SourceType::tsx(),
        None | Some(_) => Err(anyhow::anyhow!("Failed to detect the lang of {id}."))?,
      }
    };

    let mut transform_options = self.transform_options.clone();

    if let Some(Either::Right(jsx)) = &mut transform_options.jsx {
      let is_refresh_disabled = self.is_server_consumer
        || matches!(self.jsx_refresh_filter(id, cwd), JsxRefreshFilter::False)
        || !(ext.is_some_and(|v| v.ends_with('x')) || {
          let jsx_import_source = self
            .transform_options
            .jsx
            .as_ref()
            .and_then(|v| match v {
              Either::Right(jsx) => jsx.import_source.as_deref(),
              Either::Left(_) => None,
            })
            .unwrap_or("react");

          let bytes = code.as_bytes();
          let prefix = jsx_import_source.as_bytes();

          let mut found = false;
          for pos in memchr::memmem::find_iter(bytes, prefix) {
            let rest = &bytes[pos + prefix.len()..];
            if rest.starts_with(b"/jsx-runtime") || rest.starts_with(b"/jsx-dev-runtime") {
              found = true;
              break;
            }
          }
          found
        });

      if is_refresh_disabled && jsx.refresh.is_some() {
        jsx.refresh = None;
      }
    }

    if source_type.is_typescript() {
      let path = Path::new(cwd).join(id);
      if let Some(tsconfig) = self.resolver.find_tsconfig(path)? {
        // Tsconfig could be out of root, make sure it is watched
        let tsconfig_path = tsconfig.path.to_string_lossy();
        if !tsconfig_path.starts_with(cwd) {
          ctx.add_watch_file(&tsconfig_path);
        }

        let (merged_options, warnings) = merge_tsconfig(transform_options, &tsconfig, false);
        for warning in warnings {
          ctx.warn(LogWithoutPlugin {
            message: warning.to_string(),
            id: Some(id.to_string()),
            code: Some(warning.kind().to_string()),
            ..Default::default()
          });
        }
        transform_options = merged_options;
      }
    }

    Ok((source_type, transform_options.try_into().map_err(|err: String| anyhow::anyhow!(err))?))
  }
}
