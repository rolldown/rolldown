use std::{
  ops::{Deref, DerefMut},
  path::{Path, PathBuf},
  sync::Arc,
};

use dashmap::Entry;
use itertools::Either;
use oxc::transformer::{ESFeature, EngineTargets, TransformOptions as OxcTransformOptions};
use oxc_resolver::{ResolveOptions, Resolver, TsconfigDiscovery, TsconfigOptions};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_utils::dashmap::FxDashMap;

use crate::{BundlerTransformOptions, JsxOptions, TsConfig};

#[derive(Debug, Default, Clone)]
pub enum JsxPreset {
  /// Enable JSX transformer
  #[default]
  Enable,
  /// Disable JSX parser - syntax error if JSX is encountered
  Disable,
  /// Parse JSX but preserve it in output
  Preserve,
}

/// Transform options with auto tsconfig discovery and caching
#[derive(Debug, Clone)]
pub struct RawTransformOptions {
  pub base_options: Arc<BundlerTransformOptions>,
  /// Cache key: tsconfig path, or empty PathBuf for files without tsconfig
  pub cache: FxDashMap<PathBuf, Arc<OxcTransformOptions>>,
  resolver: Arc<Resolver>,
}

impl RawTransformOptions {
  pub fn new(base_options: BundlerTransformOptions, tsconfig: TsConfig) -> Self {
    Self {
      base_options: Arc::new(base_options),
      cache: FxDashMap::default(),
      resolver: Arc::new(Resolver::new(ResolveOptions {
        tsconfig: match tsconfig {
          TsConfig::Auto(v) => v.then_some(TsconfigDiscovery::Auto),
          TsConfig::Manual(config_file) => Some(TsconfigDiscovery::Manual(TsconfigOptions {
            config_file,
            references: oxc_resolver::TsconfigReferences::Auto,
          })),
        },
        ..Default::default()
      })),
    }
  }

  pub fn get_or_create_for_tsconfig(
    &self,
    tsconfig: Option<&oxc_resolver::TsConfig>,
    warnings: &mut Vec<BuildDiagnostic>,
  ) -> BuildResult<Arc<OxcTransformOptions>> {
    let cache_key = tsconfig.map(|t| t.path.clone()).unwrap_or_default();
    match self.cache.entry(cache_key) {
      Entry::Occupied(entry) => Ok(Arc::clone(entry.get())),
      Entry::Vacant(vacant_entry) => {
        let merged_options = Arc::new(merge_transform_options_with_tsconfig(
          self.base_options.as_ref().clone(),
          tsconfig,
          warnings,
        )?);
        vacant_entry.insert(Arc::clone(&merged_options));
        Ok(merged_options)
      }
    }
  }
}

#[derive(Debug, Clone)]
pub enum TransformOptionsInner {
  /// Auto tsconfig discovery - each file uses its nearest tsconfig
  Raw(RawTransformOptions),
  /// Pre-resolved options - all files use the same options
  Normal(Arc<OxcTransformOptions>),
}

#[derive(Debug, Clone)]
pub struct TransformOptions {
  inner: TransformOptionsInner,
  pub target: EngineTargets,
  pub jsx_preset: JsxPreset,
}

impl Deref for TransformOptions {
  type Target = TransformOptionsInner;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl DerefMut for TransformOptions {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}

impl TransformOptions {
  #[inline]
  pub fn new(options: OxcTransformOptions, target: EngineTargets, jsx_preset: JsxPreset) -> Self {
    Self { inner: TransformOptionsInner::Normal(Arc::new(options)), target, jsx_preset }
  }

  #[inline]
  pub fn new_raw(raw: RawTransformOptions, target: EngineTargets, jsx_preset: JsxPreset) -> Self {
    Self { inner: TransformOptionsInner::Raw(raw), target, jsx_preset }
  }

  #[inline]
  pub fn is_jsx_disabled(&self) -> bool {
    matches!(self.jsx_preset, JsxPreset::Disable)
  }

  #[inline]
  pub fn is_jsx_preserve(&self) -> bool {
    matches!(self.jsx_preset, JsxPreset::Preserve)
  }

  pub fn should_transform_js(&self) -> bool {
    match &self.inner {
      TransformOptionsInner::Normal(opts) => opts.env.regexp.set_notation,
      TransformOptionsInner::Raw(_) => self.target.has_feature(ESFeature::ES2024UnicodeSetsRegex),
    }
  }

  pub fn options_for_file(
    &self,
    file_path: Option<&Path>,
    warnings: &mut Vec<BuildDiagnostic>,
  ) -> BuildResult<Arc<OxcTransformOptions>> {
    match &self.inner {
      TransformOptionsInner::Normal(opts) => Ok(Arc::clone(opts)),
      TransformOptionsInner::Raw(raw) => {
        let tsconfig = match file_path {
          Some(path) => raw
            .resolver
            .find_tsconfig(path)
            .map_err(|err| BuildDiagnostic::tsconfig_error(path.display().to_string(), err))?,
          None => None,
        };
        raw.get_or_create_for_tsconfig(tsconfig.as_deref(), warnings)
      }
    }
  }
}

impl Default for TransformOptions {
  fn default() -> Self {
    Self {
      inner: TransformOptionsInner::Normal(Arc::new(OxcTransformOptions::default())),
      target: EngineTargets::default(),
      jsx_preset: JsxPreset::default(),
    }
  }
}

pub fn merge_transform_options_with_tsconfig(
  mut transform_options: BundlerTransformOptions,
  tsconfig: Option<&oxc_resolver::TsConfig>,
  warnings: &mut Vec<BuildDiagnostic>,
) -> BuildResult<OxcTransformOptions> {
  if let Some(tsconfig) = &tsconfig {
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

  Ok(transform_options.try_into().map_err(|message: String| {
    let hint = message
      .contains("Invalid target")
      .then(|| "Rolldown only supports ES2015 (ES6) and later.".to_owned());
    BuildDiagnostic::bundler_initialize_error(message, hint)
  })?)
}
