// Auto-generated code, DO NOT EDIT DIRECTLY!
// To edit this generated file you have to edit `tasks/generator/src/generators/checks.rs`

use bitflags::bitflags;
bitflags! {
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
  pub struct EventKindSwitcher: u64 {
    const AmbiguousExternalNamespaceError = 1 << 0;
    const CircularDependency = 1 << 1;
    const CircularReexportError = 1 << 2;
    const Eval = 1 << 3;
    const IllegalIdentifierAsNameError = 1 << 4;
    const InvalidExportOptionError = 1 << 5;
    const InvalidOptionError = 1 << 6;
    const InvalidTlaFormatError = 1 << 7;
    const MissingExportError = 1 << 8;
    const MissingGlobalName = 1 << 9;
    const MissingNameOptionForIifeExport = 1 << 10;
    const InvalidAnnotation = 1 << 11;
    const MixedExports = 1 << 12;
    const ParseError = 1 << 13;
    const UnresolvedEntry = 1 << 14;
    const UnresolvedImport = 1 << 15;
    const FilenameConflict = 1 << 16;
    const FilenameOutsideOutputDirectoryError = 1 << 17;
    const FileNotFoundError = 1 << 18;
    const ModuleLevelDirective = 1 << 45;
    const AssignToImportError = 1 << 19;
    const CommonJsVariableInEsm = 1 << 20;
    const ImportIsUndefined = 1 << 21;
    const UnsupportedFeatureError = 1 << 22;
    const EmptyImportMeta = 1 << 23;
    const JsonParseError = 1 << 24;
    const IllegalReassignmentError = 1 << 25;
    const InvalidDefineConfigError = 1 << 26;
    const ResolveError = 1 << 27;
    const UnhandleableError = 1 << 28;
    const UnloadableDependencyError = 1 << 29;
    const TransformError = 1 << 30;
    const ToleratedTransform = 1 << 31;
    const NapiError = 1 << 32;
    const CannotCallNamespace = 1 << 33;
    const ConfigurationFieldConflict = 1 << 34;
    const PreferBuiltinFeature = 1 << 35;
    const BundlerInitializeError = 1 << 36;
    const PluginError = 1 << 37;
    const AlreadyClosedError = 1 << 38;
    const CouldNotCleanDirectory = 1 << 39;
    const PluginTimings = 1 << 40;
    const DuplicateShebang = 1 << 41;
    const TsConfigError = 1 << 42;
    const UnsupportedTsconfigOption = 1 << 43;
    const RuntimeModuleSymbolNotFoundError = 1 << 44;
    const IneffectiveDynamicImport = 1 << 46;
    const RequireTlaError = 1 << 47;
    const LargeBarrelModules = 1 << 48;
    const SourcemapBroken = 1 << 49;
    const NamespaceConflict = 1 << 50;
    const ShimmedExport = 1 << 51;
    const DeprecatedImportAssertion = 1 << 52;
  }
}
