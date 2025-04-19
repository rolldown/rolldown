use std::path::{Path, PathBuf};

use itertools::Either;
use oxc::{span::SourceType, transformer::TransformOptions};
use rolldown_common::ModuleType;
use rolldown_plugin::SharedTransformPluginContext;
use rolldown_utils::{clean_url::clean_url, pattern_filter::filter as pattern_filter};

use crate::{JsxOptions, TransformPlugin};

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

  #[allow(clippy::too_many_lines)]
  pub fn get_modified_transform_options(
    &self,
    ctx: &SharedTransformPluginContext,
    id: &str,
    cwd: &str,
    ext: Option<&str>,
  ) -> (SourceType, TransformOptions) {
    let is_js_lang = matches!(self.jsx_refresh_filter(id, cwd), JsxRefreshFilter::True)
      && ext.is_some_and(|ext| ["js", "jsx", "ts", "tsx", "mjs"].contains(&ext));

    let is_refresh_disabled = self.is_server_consumer
      || matches!(self.jsx_refresh_filter(id, cwd), JsxRefreshFilter::False);

    let source_type = if is_js_lang {
      SourceType::mjs()
    } else {
      match self.transform_options.lang.as_deref().xor(ext) {
        Some("js") => SourceType::mjs(),
        Some("jsx") => SourceType::jsx(),
        Some("ts") => SourceType::ts(),
        Some("tsx") => SourceType::tsx(),
        None | Some(_) => {
          if let Some(lang) = &self.transform_options.lang {
            panic!("Invalid value for `transformOptions.lang`: `{lang}`.")
          } else {
            panic!("Failed to detect the lang of {id}. Please specify `transformOptions.lang`.")
          }
        }
      }
    };

    let mut transform_options = self.transform_options.clone();

    if is_refresh_disabled {
      if let Some(Either::Right(jsx)) = &mut transform_options.jsx {
        if jsx.refresh.is_some() {
          jsx.refresh = None;
        }
      }
    }

    if source_type.is_typescript() {
      let path = Path::new(id).parent().and_then(find_tsconfig_json_for_file);
      let tsconfig = path.and_then(|path| ctx.inner.resolver().resolve_tsconfig(&path).ok());

      if let Some(tsconfig) = tsconfig {
        // Tsconfig could be out of root, make sure it is watched
        let tsconfig_path = tsconfig.path.to_string_lossy();
        if !tsconfig_path.starts_with(cwd) {
          ctx.inner.add_watch_file(&tsconfig_path);
        }

        let compiler_options = &tsconfig.compiler_options;

        // when both the normal options and tsconfig is set,
        // we want to prioritize the normal options
        if transform_options
          .jsx
          .as_ref()
          .is_none_or(|jsx| matches!(jsx, Either::Right(right) if right.runtime.is_none()))
        {
          if compiler_options.jsx.as_deref() == Some("preserve") {
            transform_options.jsx = Some(Either::Left(String::from("preserve")));
          } else {
            let mut jsx = if let Some(Either::Right(jsx)) = transform_options.jsx {
              jsx
            } else {
              JsxOptions::default()
            };

            if compiler_options.jsx_factory.is_some() && jsx.pragma.is_none() {
              jsx.pragma.clone_from(&compiler_options.jsx_factory);
            }
            if compiler_options.jsx_import_source.is_some() && jsx.import_source.is_none() {
              jsx.import_source.clone_from(&compiler_options.jsx_import_source);
            }
            if compiler_options.jsx_fragment_factory.is_some() && jsx.pragma_frag.is_none() {
              jsx.pragma_frag.clone_from(&compiler_options.jsx_fragment_factory);
            }

            match compiler_options.jsx.as_deref() {
              Some("react") => {
                jsx.runtime = Some(String::from("classic"));
                // this option should not be set when using classic runtime
                jsx.import_source = None;
              }
              Some("react-jsx") => {
                jsx.runtime = Some(String::from("automatic"));
                // these options should not be set when using automatic runtime
                jsx.pragma = None;
                jsx.pragma_frag = None;
              }
              Some("react-jsxdev") => jsx.development = Some(true),
              _ => {}
            }

            transform_options.jsx = Some(Either::Right(jsx));
          }
        }

        if transform_options.decorator.as_ref().is_none_or(|decorator| decorator.legacy.is_none()) {
          let mut decorator = transform_options.decorator.unwrap_or_default();

          if compiler_options.experimental_decorators.is_some() {
            decorator.legacy = compiler_options.experimental_decorators;
          }

          transform_options.decorator = Some(decorator);
        }

        // | preserveValueImports | importsNotUsedAsValues | verbatimModuleSyntax | onlyRemoveTypeImports |
        // | -------------------- | ---------------------- | -------------------- |---------------------- |
        // | false                | remove                 | false                | false                 |
        // | false                | preserve, error        | -                    | -                     |
        // | true                 | remove                 | -                    | -                     |
        // | true                 | preserve, error        | true                 | true                  |
        let mut typescript = transform_options.typescript.unwrap_or_default();
        typescript.only_remove_type_imports = if compiler_options.verbatim_module_syntax.is_some() {
          compiler_options.verbatim_module_syntax
        } else if compiler_options.preserve_value_imports.is_some()
          || compiler_options.imports_not_used_as_values.is_some()
        {
          let preserve_value_imports = compiler_options.preserve_value_imports.unwrap_or(false);
          let imports_not_used_as_values =
            compiler_options.imports_not_used_as_values.as_deref().unwrap_or("remove");
          if !preserve_value_imports && imports_not_used_as_values == "remove" {
            Some(true)
          } else if preserve_value_imports
            && (imports_not_used_as_values == "preserve" || imports_not_used_as_values == "error")
          {
            Some(false)
          } else {
            // warnings.push(
            //   `preserveValueImports=${preserveValueImports} + importsNotUsedAsValues=${importsNotUsedAsValues} is not supported by oxc.` +
            //     'Please migrate to the new verbatimModuleSyntax option.',
            // )
            Some(false)
          }
        } else {
          Some(false)
        };
        transform_options.typescript = Some(typescript);
      }
    }

    (source_type, transform_options.into())
  }
}

fn find_tsconfig_json_for_file(path: &Path) -> Option<PathBuf> {
  let mut dir = path.to_path_buf();

  loop {
    let tsconfig_json = dir.join("tsconfig.json");
    if tsconfig_json.exists() {
      return Some(tsconfig_json);
    }

    let Some(parent) = dir.parent() else { break };
    dir = parent.to_path_buf();
  }

  None
}
