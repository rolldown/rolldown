use std::sync::Arc;

use oxc::transformer::ESTarget;
use rolldown_common::{BundlerTransformOptions, Either, JsxOptions, JsxPreset, TransformOptions};

#[expect(clippy::too_many_lines)]
pub fn normalize_transform_options_with_tsconfig(
  mut transform_options: BundlerTransformOptions,
  tsconfig: Option<Arc<rolldown_resolver::TsConfig>>,
) -> anyhow::Result<TransformOptions> {
  let es_target = normalize_es_target(transform_options.target.as_ref());
  let mut jsx_preset = JsxPreset::Enable;

  if let Some(Either::Left(jsx)) = &mut transform_options.jsx {
    jsx_preset = match jsx.as_str() {
      "preserve" => JsxPreset::Preserve,
      "disable" => {
        "preserve".clone_into(jsx);
        JsxPreset::Disable
      }
      _ => return Err(anyhow::anyhow!("Invalid jsx option: `{jsx}`.")),
    };
  }

  if let Some(tsconfig) = tsconfig {
    let compiler_options = &tsconfig.compiler_options;

    // when both the normal options and tsconfig is set, we want to prioritize the normal options
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
    if typescript.only_remove_type_imports.is_none() {
      if compiler_options.verbatim_module_syntax.is_some() {
        typescript.only_remove_type_imports = compiler_options.verbatim_module_syntax;
      } else if compiler_options.preserve_value_imports.is_some()
        || compiler_options.imports_not_used_as_values.is_some()
      {
        let preserve_value_imports = compiler_options.preserve_value_imports.unwrap_or(false);
        let imports_not_used_as_values =
          compiler_options.imports_not_used_as_values.as_deref().unwrap_or("remove");
        typescript.only_remove_type_imports =
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
          };
      }
    }

    let disable_use_define_for_class_fields =
      !compiler_options.use_define_for_class_fields.unwrap_or_else(|| {
        let target = compiler_options.target.as_deref();
        let Some(target) = target else { return false };
        if target.len() < 3 || !&target[..2].eq_ignore_ascii_case("es") {
          return false;
        }
        let reset = &target[2..];
        if reset.eq_ignore_ascii_case("next") {
          return true;
        }
        reset.parse::<usize>().is_ok_and(|x| x > 2021)
      });

    let mut assumptions = transform_options.assumptions.unwrap_or_default();
    assumptions.set_public_class_fields = Some(disable_use_define_for_class_fields);
    typescript.remove_class_fields_without_initializer = Some(disable_use_define_for_class_fields);

    transform_options.typescript = Some(typescript);
    transform_options.assumptions = Some(assumptions);
  }

  Ok(TransformOptions::new(
    transform_options.try_into().map_err(|err: String| anyhow::anyhow!(err))?,
    es_target,
    jsx_preset,
  ))
}

fn normalize_es_target(target: Option<&Either<String, Vec<String>>>) -> ESTarget {
  target.map_or(ESTarget::ESNext, |target| {
    let targets = match target {
      Either::Left(target) => {
        if target.contains(',') {
          target.split(',').collect::<Vec<&str>>()
        } else {
          vec![target.as_str()]
        }
      }
      Either::Right(target) => {
        target.iter().map(std::string::String::as_str).collect::<Vec<&str>>()
      }
    };
    for target in targets {
      if target.len() <= 2 || !target[..2].eq_ignore_ascii_case("es") {
        continue;
      }
      if target[2..].eq_ignore_ascii_case("next") {
        return ESTarget::ESNext;
      }
      if let Ok(n) = target[2..].parse::<usize>() {
        return match n {
          6 | 2015 => ESTarget::ES2015,
          2016 => ESTarget::ES2016,
          2017 => ESTarget::ES2017,
          2018 => ESTarget::ES2018,
          2019 => ESTarget::ES2019,
          2020 => ESTarget::ES2020,
          2021 => ESTarget::ES2021,
          2022 => ESTarget::ES2022,
          2023 => ESTarget::ES2023,
          2024 => ESTarget::ES2024,
          _ => continue,
        };
      }
    }
    ESTarget::ES2015
  })
}
