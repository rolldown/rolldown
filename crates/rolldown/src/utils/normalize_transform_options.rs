use std::sync::Arc;

use oxc::transformer::ESTarget;
use rolldown_common::{BundlerTransformOptions, Either, JsxOptions, JsxPreset, TransformOptions};
use rolldown_error::{BuildDiagnostic, BuildResult};

#[expect(clippy::too_many_lines)]
pub fn normalize_transform_options_with_tsconfig(
  mut transform_options: BundlerTransformOptions,
  tsconfig: Option<Arc<rolldown_resolver::TsConfig>>,
  warnings: &mut Vec<BuildDiagnostic>,
) -> BuildResult<TransformOptions> {
  let es_target = normalize_es_target(transform_options.target.as_ref());
  let mut jsx_preset = JsxPreset::Enable;

  if let Some(Either::Left(jsx)) = &mut transform_options.jsx {
    jsx_preset = match jsx.as_str() {
      "preserve" => JsxPreset::Preserve,
      "disable" => {
        "preserve".clone_into(jsx);
        JsxPreset::Disable
      }
      _ => {
        return Err(BuildDiagnostic::bundler_initialize_error(
          format!("Invalid jsx option: `{jsx}`."),
          Some("Valid options are 'preserve' to keep JSX as-is, or omit the option to use default transform.".to_owned()),
        ))?;
      }
    };
  }

  if let Some(tsconfig) = tsconfig {
    let compiler_options = &tsconfig.compiler_options;

    // when both the normal options and tsconfig is set, we want to prioritize the normal options
    if compiler_options.jsx.as_deref() == Some("preserve") {
      if transform_options
        .jsx
        .as_ref()
        .is_none_or(|jsx| matches!(jsx, Either::Right(right) if right.runtime.is_none()))
      {
        transform_options.jsx = Some(Either::Left(String::from("preserve")));
      } else {
        warnings.push(
          BuildDiagnostic::configuration_field_conflict(
            "transform",
            "jsx",
            "tsconfig.json",
            "compilerOptions.jsx",
          )
          .with_severity_warning(),
        );
      }
    }

    if !matches!(&transform_options.jsx, Some(Either::Left(left)) if left == "preserve") {
      let mut jsx = if let Some(Either::Right(jsx)) = transform_options.jsx {
        jsx
      } else {
        JsxOptions::default()
      };

      if compiler_options.jsx_factory.is_some() {
        if jsx.pragma.is_none() {
          jsx.pragma.clone_from(&compiler_options.jsx_factory);
        } else {
          warnings.push(
            BuildDiagnostic::configuration_field_conflict(
              "transform.jsx",
              "pragma",
              "tsconfig.json",
              "compilerOptions.jsxFactory",
            )
            .with_severity_warning(),
          );
        }
      }
      if compiler_options.jsx_import_source.is_some() {
        if jsx.import_source.is_none() {
          jsx.import_source.clone_from(&compiler_options.jsx_import_source);
        } else {
          warnings.push(
            BuildDiagnostic::configuration_field_conflict(
              "transform.jsx",
              "importSource",
              "tsconfig.json",
              "compilerOptions.jsxImportSource",
            )
            .with_severity_warning(),
          );
        }
      }
      if compiler_options.jsx_fragment_factory.is_some() {
        if jsx.pragma_frag.is_none() {
          jsx.pragma_frag.clone_from(&compiler_options.jsx_fragment_factory);
        } else {
          warnings.push(
            BuildDiagnostic::configuration_field_conflict(
              "transform.jsx",
              "pragmaFrag",
              "tsconfig.json",
              "compilerOptions.jsxFragmentFactory",
            )
            .with_severity_warning(),
          );
        }
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
    } else {
      if compiler_options.experimental_decorators.is_some() {
        warnings.push(
          BuildDiagnostic::configuration_field_conflict(
            "transform.decorator",
            "legacy",
            "tsconfig.json",
            "compilerOptions.experimentalDecorators",
          )
          .with_severity_warning(),
        );
      }
      if compiler_options.emit_decorator_metadata.is_some()
        && transform_options.decorator.as_ref().is_some_and(|d| d.emit_decorator_metadata.is_some())
      {
        warnings.push(
          BuildDiagnostic::configuration_field_conflict(
            "transform.decorator",
            "emitDecoratorMetadata",
            "tsconfig.json",
            "compilerOptions.emitDecoratorMetadata",
          )
          .with_severity_warning(),
        );
      }
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
    } else if compiler_options.verbatim_module_syntax.is_some() {
      warnings.push(
        BuildDiagnostic::configuration_field_conflict(
          "transform.typescript",
          "onlyRemoveTypeImports",
          "tsconfig.json",
          "compilerOptions.verbatimModuleSyntax",
        )
        .with_severity_warning(),
      );
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

  let options = transform_options.try_into().map_err(|err: String| {
    let hint = if err.contains("Invalid target") {
      Some("Rolldown only supports ES2015 (ES6) and later.".to_owned())
    } else {
      None
    };

    BuildDiagnostic::bundler_initialize_error(err, hint)
  })?;

  Ok(TransformOptions::new(options, es_target, jsx_preset))
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
