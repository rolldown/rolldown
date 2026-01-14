//! Naming convention:
//! - All kinds that will terminate the build process should be named with a postfix "Error".
use std::fmt::Display;

#[derive(Debug, Clone, Copy)]
pub enum EventKind {
  // --- These kinds are copied from rollup: https://github.com/rollup/rollup/blob/0b665c31833525c923c0fc20f43ebfca748c6670/src/utils/logs.ts#L102-L179
  AmbiguousExternalNamespaceError = 0,
  /// Whether to emit warnings when detecting circular dependency
  ///
  /// Circular dependencies lead to a bigger bundle size and sometimes cause execution order issues and are better to avoid.
  CircularDependency = 1,
  CircularReexportError = 2,
  /// Whether to emit warnings when detecting uses of direct `eval`s
  ///
  /// See [Avoiding Direct `eval` in Troubleshooting page](https://rolldown.rs/guide/troubleshooting#avoiding-direct-eval) for more details.
  Eval = 3,
  IllegalIdentifierAsNameError = 4,
  InvalidExportOptionError = 5,
  InvalidOptionError = 6,
  MissingExportError = 7,
  /// Whether to emit warnings when the `output.globals` option is missing when needed
  ///
  /// See [`output.globals`](https://rolldown.rs/reference/OutputOptions.globals).
  MissingGlobalName = 8,
  /// Whether to emit warnings when the `output.name` option is missing when needed
  ///
  /// See [`output.name`](https://rolldown.rs/reference/OutputOptions.name).
  MissingNameOptionForIifeExport = 9,
  /// Whether to emit warnings when the way to export values is ambiguous
  ///
  /// See [`output.exports`](https://rolldown.rs/reference/OutputOptions.exports).
  MixedExports = 11,
  ParseError = 12,
  /// Whether to emit warnings when an entrypoint cannot be resolved
  UnresolvedEntry = 13,
  /// Whether to emit warnings when an import cannot be resolved
  UnresolvedImport = 14,
  /// Whether to emit warnings when files generated have the same name with different contents
  FilenameConflict = 15,
  // !! Only add new kind if it's not covered by the kinds from rollup !!

  // --- These kinds are derived from esbuild
  AssignToImportError = 16,
  /// Whether to emit warnings when a CommonJS variable is used in an ES module
  ///
  /// CommonJS variables like `module` and `exports` are treated as global variables in ES modules and may not work as expected.
  CommonJsVariableInEsm = 17,
  ExportUndefinedVariableError = 18,
  /// Whether to emit warnings when an imported variable is not exported
  ///
  /// If the code is importing a variable that is not exported by the imported module, the value will always be `undefined`. This might be a mistake in the code.
  ImportIsUndefined = 19,
  UnsupportedFeatureError = 20,
  /// Whether to emit warnings when `import.meta` is not supported with the output format and is replaced with an empty object (`{}`)
  ///
  /// See [`import.meta` in Non-ESM Output Formats page](https://rolldown.rs/in-depth/non-esm-output-formats#import-meta) for more details.
  EmptyImportMeta = 21,

  // --- These kinds are rolldown specific
  JsonParseError = 22,
  IllegalReassignmentError = 23,
  InvalidDefineConfigError = 24,
  ResolveError = 25,
  UnhandleableError = 26,
  UnloadableDependencyError = 27,
  TransformError = 28,
  ToleratedTransform = 29,

  NapiError = 30,
  /// Whether to emit warnings when a namespace is called as a function
  ///
  /// A module namespace object is an object and not a function. Calling it as a function will cause a runtime error.
  CannotCallNamespace = 31,
  /// Whether to emit warnings when a config value is overridden by another config value with a higher priority
  ConfigurationFieldConflict = 32,
  /// Whether to emit warnings when a plugin that is covered by a built-in feature is used
  ///
  /// Using built-in features is generally more performant than using plugins.
  PreferBuiltinFeature = 33,
  BundlerInitializeError = 34,
  PluginError = 35,
  AlreadyClosedError = 36,
  /// Whether to emit warnings when Rolldown could not clean the output directory
  ///
  /// See [`output.cleanDir`](https://rolldown.rs/reference/OutputOptions.cleanDir).
  CouldNotCleanDirectory = 37,
  /// Whether to emit warnings when plugins take significant time during the build process
  ///
  /// {@include ../docs/checks-plugin-timings.md}
  PluginTimings = 38,
}

impl Display for EventKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      // --- Copied from rollup
      EventKind::AmbiguousExternalNamespaceError => write!(f, "AMBIGUOUS_EXTERNAL_NAMESPACES"),
      EventKind::CircularDependency => write!(f, "CIRCULAR_DEPENDENCY"),
      EventKind::CircularReexportError => write!(f, "CIRCULAR_REEXPORT"),
      EventKind::Eval => write!(f, "EVAL"),
      EventKind::IllegalIdentifierAsNameError => write!(f, "ILLEGAL_IDENTIFIER_AS_NAME"),
      EventKind::InvalidExportOptionError => write!(f, "INVALID_EXPORT_OPTION"),
      EventKind::InvalidOptionError => write!(f, "INVALID_OPTION"),
      EventKind::MixedExports => write!(f, "MIXED_EXPORTS"),
      EventKind::MissingGlobalName => write!(f, "MISSING_GLOBAL_NAME"),
      EventKind::MissingNameOptionForIifeExport => write!(f, "MISSING_NAME_OPTION_FOR_IIFE_EXPORT"),
      EventKind::MissingExportError => write!(f, "MISSING_EXPORT"),
      EventKind::ParseError => write!(f, "PARSE_ERROR"),
      EventKind::UnresolvedEntry => write!(f, "UNRESOLVED_ENTRY"),
      EventKind::UnresolvedImport => write!(f, "UNRESOLVED_IMPORT"),
      EventKind::FilenameConflict => write!(f, "FILE_NAME_CONFLICT"),

      // --- Derived from esbuild
      EventKind::AssignToImportError => write!(f, "ASSIGN_TO_IMPORT"),
      EventKind::CommonJsVariableInEsm => write!(f, "COMMONJS_VARIABLE_IN_ESM"),
      EventKind::ExportUndefinedVariableError => write!(f, "EXPORT_UNDEFINED_VARIABLE"),
      EventKind::ImportIsUndefined => write!(f, "IMPORT_IS_UNDEFINED"),
      EventKind::UnsupportedFeatureError => write!(f, "UNSUPPORTED_FEATURE"),
      EventKind::EmptyImportMeta => write!(f, "EMPTY_IMPORT_META"),

      // --- Rolldown specific
      EventKind::JsonParseError => write!(f, "JSON_PARSE"),
      EventKind::IllegalReassignmentError => write!(f, "ILLEGAL_REASSIGNMENT"),
      EventKind::InvalidDefineConfigError => write!(f, "INVALID_DEFINE_CONFIG"),
      EventKind::ResolveError => write!(f, "RESOLVE_ERROR"),
      EventKind::UnhandleableError => write!(f, "UNHANDLEABLE_ERROR"),
      EventKind::UnloadableDependencyError => write!(f, "UNLOADABLE_DEPENDENCY"),
      EventKind::TransformError => write!(f, "TRANSFORM_ERROR"),
      EventKind::ToleratedTransform => write!(f, "TOLERATED_TRANSFORM"),

      EventKind::NapiError => write!(f, "NAPI_ERROR"),
      EventKind::CannotCallNamespace => write!(f, "CANNOT_CALL_NAMESPACE"),
      EventKind::ConfigurationFieldConflict => write!(f, "CONFIGURATION_FIELD_CONFLICT"),
      EventKind::PreferBuiltinFeature => write!(f, "PREFER_BUILTIN_FEATURE"),
      EventKind::BundlerInitializeError => write!(f, "BUNDLER_INITIALIZE_ERROR"),
      EventKind::PluginError => write!(f, "PLUGIN_ERROR"),
      EventKind::AlreadyClosedError => write!(f, "ALREADY_CLOSED"),
      EventKind::CouldNotCleanDirectory => write!(f, "COULD_NOT_CLEAN_DIRECTORY"),
      EventKind::PluginTimings => write!(f, "PLUGIN_TIMINGS"),
    }
  }
}
