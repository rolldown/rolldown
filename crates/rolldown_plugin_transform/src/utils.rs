use std::path::{Path, PathBuf};

use itertools::Either;
use oxc::{span::SourceType, transformer::TransformOptions};
use rolldown_common::ModuleType;
use rolldown_plugin::SharedTransformPluginContext;
use rolldown_utils::{pattern_filter::filter as pattern_filter, url::clean_url};

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

  #[allow(clippy::too_many_lines)]
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
      match self.transform_options.lang.as_deref().or(ext) {
        Some("js" | "cjs" | "mjs") => SourceType::mjs(),
        Some("jsx") => SourceType::jsx(),
        Some("ts" | "cts" | "mts") => SourceType::ts(),
        Some("tsx") => SourceType::tsx(),
        None | Some(_) => {
          let message = if let Some(lang) = &self.transform_options.lang {
            anyhow::anyhow!("Invalid value for `transformOptions.lang`: `{lang}`.")
          } else {
            anyhow::anyhow!(
              "Failed to detect the lang of {id}. Please specify `transformOptions.lang`."
            )
          };

          return Err(message);
        }
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
      let path = Path::new(id).parent().and_then(find_tsconfig_json_for_file);
      let tsconfig = path.map(|path| ctx.resolver().resolve_tsconfig(&path)).transpose()?;

      if let Some(tsconfig) = tsconfig {
        // Tsconfig could be out of root, make sure it is watched
        let tsconfig_path = tsconfig.path.to_string_lossy();
        if !tsconfig_path.starts_with(cwd) {
          ctx.add_watch_file(&tsconfig_path);
        }

        let compiler_options = &tsconfig.compiler_options;

        // when both the normal options and tsconfig is set,
        // we want to prioritize the normal options
        if compiler_options.jsx.as_deref() == Some("preserve")
          && transform_options
            .jsx
            .as_ref()
            .is_none_or(|jsx| matches!(jsx, Either::Right(right) if right.runtime.is_none()))
        {
          transform_options.jsx = Some(Either::Left(String::from("preserve")));
        }
        if !matches!(&transform_options.jsx, Some(Either::Left(left)) if left == "preserve") {
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

          if jsx.runtime.is_none() {
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
          }
          transform_options.jsx = Some(Either::Right(jsx));
        }

        if transform_options.decorator.as_ref().is_none_or(|decorator| decorator.legacy.is_none()) {
          let mut decorator = transform_options.decorator.unwrap_or_default();

          if compiler_options.experimental_decorators.is_some() {
            decorator.legacy = compiler_options.experimental_decorators;
          }

          if compiler_options.emit_decorator_metadata.is_some() {
            decorator.emit_decorator_metadata = compiler_options.emit_decorator_metadata;
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

        let disable_use_define_for_class_fields = !compiler_options
          .use_define_for_class_fields
          .unwrap_or_else(|| is_use_define_for_class_fields(compiler_options.target.as_deref()));

        let mut assumptions = transform_options.assumptions.unwrap_or_default();
        assumptions.set_public_class_fields = Some(disable_use_define_for_class_fields);
        transform_options.assumptions = Some(assumptions);

        // set target to es2021 or lower to enable class property transforms
        // https://github.com/oxc-project/oxc/issues/6735#issuecomment-2513866362
        if disable_use_define_for_class_fields {
          let target = if let Some(target) = transform_options.target {
            let mut target = match target {
              Either::Left(t) => t.split(',').map(String::from).collect(),
              Either::Right(t) => t,
            };

            if let Some(target) =
              target.iter_mut().find(|t| t.len() > 2 && t[..2].eq_ignore_ascii_case("es"))
            {
              let reset = &target[2..];
              if reset.eq_ignore_ascii_case("next")
                || reset.parse::<usize>().is_ok_and(|x| x > 2021)
              {
                *target = String::from("es2021");
              }
            } else {
              target.push(String::from("es2021"));
            }
            Either::Right(target)
          } else {
            Either::Left(String::from("es2021"))
          };
          transform_options.target = Some(target);
        }
      }
    }

    Ok((source_type, transform_options.try_into().map_err(|err: String| anyhow::anyhow!(err))?))
  }
}

fn find_tsconfig_json_for_file(path: &Path) -> Option<PathBuf> {
  // don't load tsconfig for paths in node_modules like esbuild
  if is_in_node_modules(path) {
    return None;
  }

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

fn is_in_node_modules(id: &Path) -> bool {
  id.components().any(|comp| comp.as_os_str() == "node_modules")
}

fn is_use_define_for_class_fields(target: Option<&str>) -> bool {
  let Some(target) = target else { return false };

  if target.len() < 3 || !&target[..2].eq_ignore_ascii_case("es") {
    return false;
  }

  let reset = &target[2..];
  if reset.eq_ignore_ascii_case("next") {
    return true;
  }

  reset.parse::<usize>().is_ok_and(|x| x > 2021)
}
