//! Enhanced transform module using internal Oxc parser and transformer.
//!
//! This module provides transform functionality similar to the bundler's internal transform,
//! but exposed as an API for use outside the bundling context.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arcstr::ArcStr;
use oxc::allocator::Allocator;
use oxc::diagnostics::{OxcDiagnostic, Severity as OxcSeverity};
use oxc::parser::{ParseOptions, Parser};
use oxc::transformer::{Helper, HelperLoaderOptions};
use oxc::{
  codegen::{Codegen, CodegenOptions, CodegenReturn},
  isolated_declarations::{IsolatedDeclarations, IsolatedDeclarationsOptions},
  semantic::SemanticBuilder,
  span::SourceType,
  transformer::Transformer,
  transformer_plugins::{
    InjectGlobalVariables, InjectGlobalVariablesConfig, InjectImport, ReplaceGlobalDefines,
    ReplaceGlobalDefinesConfig,
  },
};
use oxc_resolver::TsConfig;
use rolldown_error::{BuildDiagnostic, EventKind, Severity};
use rolldown_sourcemap::{SourceMap, collapse_sourcemaps};
use rustc_hash::FxHashMap;

use crate::inner_bundler_options::types::transform_option::{
  CompilerAssumptions, DecoratorOptions, Either,
  IsolatedDeclarationsOptions as RolldownIsolatedDeclarationsOptions, JsxOptions, PluginsOptions,
  TransformOptions, TypeScriptOptions,
};
use crate::inner_bundler_options::types::tsconfig_merge::merge_transform_options_with_tsconfig;

pub type InjectOptions = Vec<(String, Either<String, Vec<String>>)>;

/// Tsconfig option for enhanced transform.
#[derive(Debug, Clone)]
pub enum TsconfigOption {
  /// Auto-discover tsconfig.json by walking up from the file's directory.
  Auto,
  /// Use the provided tsconfig directly.
  Config(Arc<TsConfig>),
  /// Don't use tsconfig options.
  Disabled,
}

/// Result of an enhanced transform operation.
#[derive(Debug)]
pub struct EnhancedTransformResult {
  /// The transformed code
  pub code: String,
  /// The source map if sourcemap was enabled
  pub map: Option<SourceMap>,
  /// The .d.ts declaration file (if declaration generation was enabled)
  pub declaration: Option<String>,
  /// The declaration source map
  pub declaration_map: Option<SourceMap>,
  /// Helpers used.
  pub helpers_used: FxHashMap<Helper, String>,
  /// Parse and transformation errors
  pub errors: Vec<BuildDiagnostic>,
  /// Parse and transformation warnings
  pub warnings: Vec<BuildDiagnostic>,
  /// Paths to tsconfig files that were loaded during transformation.
  pub tsconfig_file_paths: Vec<PathBuf>,
}

impl EnhancedTransformResult {
  pub fn new_for_error(
    errors: Vec<BuildDiagnostic>,
    warnings: Vec<BuildDiagnostic>,
    tsconfig_file_paths: Vec<PathBuf>,
  ) -> Self {
    Self {
      code: String::new(),
      map: None,
      declaration: None,
      declaration_map: None,
      helpers_used: FxHashMap::default(),
      errors,
      warnings,
      tsconfig_file_paths,
    }
  }
}

/// Options for enhanced transform operations.
/// This is separate from `TransformOptions` to provide a clear API boundary
/// for the enhanced transform use case.
#[derive(Debug, Default, Clone)]
pub struct EnhancedTransformOptions {
  /// Configure how TSX and JSX are transformed.
  pub jsx: Option<Either<String, JsxOptions>>,

  /// Sets the target environment for the generated JavaScript.
  pub target: Option<Either<String, Vec<String>>>,

  /// Set assumptions in order to produce smaller output.
  pub assumptions: Option<CompilerAssumptions>,

  /// Decorator plugin options.
  pub decorator: Option<DecoratorOptions>,

  /// Configure how TypeScript is transformed.
  pub typescript: Option<TypeScriptOptions>,

  /// Third-party plugins to use.
  pub plugins: Option<PluginsOptions>,

  /// Behaviour for runtime helpers.
  pub helpers: Option<HelperLoaderOptions>,

  /// The current working directory. Used to resolve relative paths in other
  /// options.
  pub cwd: Option<String>,

  /// Override the source type inferred from the file extension.
  pub source_type: Option<SourceType>,

  /// Configure tsconfig handling.
  pub tsconfig: Option<TsconfigOption>,

  /// Enable source map generation.
  pub sourcemap: bool,

  /// An input source map to collapse with the output source map.
  /// This is useful when the source has already been transformed by another tool.
  pub input_map: Option<SourceMap>,

  /// Define plugin: replace global identifiers with constant expressions.
  pub define: Option<Vec<(String, String)>>,

  /// Inject plugin: auto-import globals from modules.
  /// Each entry is `(local_name, source)` where source is either:
  /// - `Left(module)`: namespace/default import from the module
  /// - `Right([module, export])`: named import from the module
  pub inject: Option<InjectOptions>,
}

impl EnhancedTransformOptions {
  #[expect(clippy::too_many_arguments)]
  pub fn from_transform_options(
    options: TransformOptions,
    cwd: Option<String>,
    source_type: Option<SourceType>,
    tsconfig: Option<TsconfigOption>,
    sourcemap: bool,
    input_map: Option<SourceMap>,
    define: Option<Vec<(String, String)>>,
    inject: Option<InjectOptions>,
  ) -> Self {
    Self {
      jsx: options.jsx,
      target: options.target,
      assumptions: options.assumptions,
      decorator: options.decorator,
      typescript: options.typescript,
      plugins: options.plugins,
      helpers: options.helpers,
      cwd,
      source_type,
      tsconfig,
      sourcemap,
      input_map,
      define,
      inject,
    }
  }
}

/// Generate isolated declarations from the parsed program.
fn generate_declarations(
  allocator: &oxc::allocator::Allocator,
  program: &oxc::ast::ast::Program,
  filename: &str,
  source: &ArcStr,
  options: &RolldownIsolatedDeclarationsOptions,
  errors: &mut Vec<BuildDiagnostic>,
  warnings: &mut Vec<BuildDiagnostic>,
) -> (Option<String>, Option<SourceMap>) {
  let isolated_decl_options =
    IsolatedDeclarationsOptions { strip_internal: options.strip_internal.unwrap_or(false) };

  let ret = IsolatedDeclarations::new(allocator, isolated_decl_options).build(program);
  if !ret.errors.is_empty() {
    append_oxc_diagnostics(ret.errors, source, filename, warnings, errors);
    if !errors.is_empty() {
      return (None, None);
    }
  }

  let enable_declaration_map = options.sourcemap.unwrap_or(false);
  let codegen_ret = Codegen::new()
    .with_options(CodegenOptions {
      source_map_path: enable_declaration_map.then(|| {
        let mut path = PathBuf::from(filename);
        path.set_extension("d.ts");
        path
      }),
      ..Default::default()
    })
    .build(&ret.program);
  (Some(codegen_ret.code), codegen_ret.map)
}

fn append_oxc_diagnostics(
  diagnostics: Vec<OxcDiagnostic>,
  source: &ArcStr,
  filename: &str,
  warnings: &mut Vec<BuildDiagnostic>,
  errors: &mut Vec<BuildDiagnostic>,
) {
  let (new_errors, new_warnings): (Vec<_>, Vec<_>) =
    diagnostics.into_iter().partition(|d| d.severity == OxcSeverity::Error);
  errors.extend(BuildDiagnostic::from_oxc_diagnostics(
    new_errors,
    &source.clone(),
    filename,
    Severity::Error,
    EventKind::ParseError,
  ));
  warnings.extend(BuildDiagnostic::from_oxc_diagnostics(
    new_warnings,
    &source.clone(),
    filename,
    Severity::Warning,
    EventKind::ParseError,
  ));
}

fn build_inject_config(
  inject: &[(String, Either<String, Vec<String>>)],
) -> InjectGlobalVariablesConfig {
  let inject_imports: Vec<InjectImport> = inject
    .iter()
    .map(|(local, value)| match value {
      Either::Left(module) => InjectImport::namespace_specifier(module, local),
      Either::Right(parts) if parts.len() >= 2 => {
        InjectImport::named_specifier(&parts[0], Some(&parts[1]), local)
      }
      Either::Right(parts) if parts.len() == 1 => {
        InjectImport::namespace_specifier(&parts[0], local)
      }
      Either::Right(_) => InjectImport::namespace_specifier("", local),
    })
    .collect();
  InjectGlobalVariablesConfig::new(inject_imports)
}

/// Transform source code using the internal Oxc parser and transformer.
///
/// # Arguments
/// * `filename` - The filename (used for source type detection and error reporting)
/// * `source_text` - The source code to transform
/// * `transform_options` - Transform options including tsconfig and sourcemap settings
pub fn enhanced_transform(
  filename: &str,
  source_text: &str,
  transform_options: EnhancedTransformOptions,
) -> EnhancedTransformResult {
  let mut errors = Vec::new();
  let mut warnings = Vec::new();
  let mut tsconfig_file_paths = Vec::new();

  let source_type = transform_options
    .source_type
    .unwrap_or_else(|| SourceType::from_path(Path::new(filename)).unwrap_or_default());
  let tsconfig: Option<Arc<TsConfig>> = match &transform_options.tsconfig {
    Some(TsconfigOption::Auto) | None => {
      let file_path = PathBuf::from(filename);
      let result = oxc_resolver::Resolver::new(oxc_resolver::ResolveOptions {
        tsconfig: Some(oxc_resolver::TsconfigDiscovery::Auto),
        ..Default::default()
      })
      .find_tsconfig(file_path);
      let found = match result {
        Ok(found) => found,
        Err(err) => {
          errors.push(BuildDiagnostic::tsconfig_error(filename.to_string(), err));
          return EnhancedTransformResult::new_for_error(errors, warnings, tsconfig_file_paths);
        }
      };
      if let Some(tsconfig) = &found {
        tsconfig_file_paths.push(tsconfig.path.clone());
      }
      found
    }
    Some(TsconfigOption::Config(config)) => Some(Arc::clone(config)),
    Some(TsconfigOption::Disabled) => None,
  };
  let enable_sourcemap = transform_options.sourcemap;
  let input_map = transform_options.input_map.clone();
  let define_options = transform_options.define.clone();
  let inject_options = transform_options.inject.clone();

  let bundler_options: TransformOptions = transform_options.into();
  let merged_options = if let Some(ref tsconfig) = tsconfig {
    let (merged, merge_warnings) =
      merge_transform_options_with_tsconfig(bundler_options, tsconfig, false);
    warnings.extend(merge_warnings);
    merged
  } else {
    bundler_options
  };
  let declaration_options =
    merged_options.typescript.as_ref().and_then(|ts| ts.declaration.clone());

  let oxc_transform_options: oxc::transformer::TransformOptions = match merged_options.try_into() {
    Ok(opts) => opts,
    Err(e) => {
      errors.push(BuildDiagnostic::bundler_initialize_error(e, None));
      return EnhancedTransformResult::new_for_error(errors, warnings, tsconfig_file_paths);
    }
  };

  let source: ArcStr = source_text.into();

  let allocator = Allocator::default();
  let parse_ret = Parser::new(&allocator, &source, source_type)
    .with_options(ParseOptions { allow_return_outside_function: true, ..Default::default() })
    .parse();
  if parse_ret.panicked || !parse_ret.errors.is_empty() {
    append_oxc_diagnostics(parse_ret.errors, &source, filename, &mut warnings, &mut errors);
    return EnhancedTransformResult::new_for_error(errors, warnings, tsconfig_file_paths);
  }

  let mut program = parse_ret.program;

  let semantic_ret = SemanticBuilder::new().build(&program);
  let mut scoping = Some(semantic_ret.semantic.into_scoping());
  if !semantic_ret.errors.is_empty() {
    append_oxc_diagnostics(semantic_ret.errors, &source, filename, &mut warnings, &mut errors);
    if !errors.is_empty() {
      return EnhancedTransformResult::new_for_error(errors, warnings, tsconfig_file_paths);
    }
  }

  // Generate isolated declarations if enabled (must be done before transform modifies the AST)
  let (declaration, declaration_map) = if let Some(ref decl_options) = declaration_options
    && source_type.is_typescript()
  {
    generate_declarations(
      &allocator,
      &program,
      filename,
      &source,
      decl_options,
      &mut errors,
      &mut warnings,
    )
  } else {
    (None, None)
  };
  if !errors.is_empty() {
    return EnhancedTransformResult::new_for_error(errors, warnings, tsconfig_file_paths);
  }

  if let Some(ref define) = define_options {
    if !define.is_empty() {
      let define_pairs: Vec<(&str, &str)> =
        define.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
      match ReplaceGlobalDefinesConfig::new(&define_pairs) {
        Ok(config) => {
          let ret = ReplaceGlobalDefines::new(&allocator, config)
            .build(scoping.take().unwrap(), &mut program);
          if !ret.changed {
            scoping = Some(ret.scoping);
          }
        }
        Err(errs) => {
          errors.extend(
            errs
              .into_iter()
              .map(|err| BuildDiagnostic::invalid_define_config(err.message.to_string())),
          );
          return EnhancedTransformResult::new_for_error(errors, warnings, tsconfig_file_paths);
        }
      }
    }
  }

  let scoping = scoping
    .take()
    .unwrap_or_else(|| SemanticBuilder::new().build(&program).semantic.into_scoping());

  let transform_ret = Transformer::new(&allocator, Path::new(filename), &oxc_transform_options)
    .build_with_scoping(scoping, &mut program);
  if !transform_ret.errors.is_empty() {
    append_oxc_diagnostics(transform_ret.errors, &source, filename, &mut warnings, &mut errors);
    if !errors.is_empty() {
      return EnhancedTransformResult::new_for_error(errors, warnings, tsconfig_file_paths);
    }
  }

  if let Some(ref inject) = inject_options
    && !inject.is_empty()
  {
    let config = build_inject_config(inject);
    let scoping = SemanticBuilder::new().build(&program).semantic.into_scoping();
    let _ = InjectGlobalVariables::new(&allocator, config).build(scoping, &mut program);
  }

  let codegen_ret: CodegenReturn = Codegen::new()
    .with_options(CodegenOptions {
      source_map_path: enable_sourcemap.then(|| std::path::PathBuf::from(filename)),
      ..Default::default()
    })
    .build(&program);

  let output_map = match (input_map, codegen_ret.map) {
    (Some(im), Some(om)) => Some(collapse_sourcemaps(&[&im, &om])),
    (None, map) => map,
    (Some(_), None) => None,
  };

  EnhancedTransformResult {
    code: codegen_ret.code,
    map: output_map,
    declaration,
    declaration_map,
    #[expect(deprecated)]
    helpers_used: transform_ret.helpers_used,
    errors,
    warnings,
    tsconfig_file_paths,
  }
}
