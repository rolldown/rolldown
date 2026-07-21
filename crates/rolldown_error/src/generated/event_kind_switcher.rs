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
    const MissingExportError = 1 << 7;
    const MissingGlobalName = 1 << 8;
    const MissingNameOptionForIifeExport = 1 << 9;
    const InvalidAnnotation = 1 << 10;
    const MixedExports = 1 << 11;
    const ParseError = 1 << 12;
    const UnresolvedEntry = 1 << 13;
    const UnresolvedImport = 1 << 14;
    const FilenameConflict = 1 << 15;
    const FilenameOutsideOutputDirectoryError = 1 << 16;
    const FileNotFoundError = 1 << 17;
    const AssignToImportError = 1 << 18;
    const CommonJsVariableInEsm = 1 << 19;
    const ImportIsUndefined = 1 << 20;
    const UnsupportedFeatureError = 1 << 21;
    const EmptyImportMeta = 1 << 22;
    const JsonParseError = 1 << 23;
    const IllegalReassignmentError = 1 << 24;
    const InvalidDefineConfigError = 1 << 25;
    const ResolveError = 1 << 26;
    const UnhandleableError = 1 << 27;
    const UnloadableDependencyError = 1 << 28;
    const TransformError = 1 << 29;
    const ToleratedTransform = 1 << 30;
    const NapiError = 1 << 31;
    const CannotCallNamespace = 1 << 32;
    const ConfigurationFieldConflict = 1 << 33;
    const PreferBuiltinFeature = 1 << 34;
    const BundlerInitializeError = 1 << 35;
    const PluginError = 1 << 36;
    const AlreadyClosedError = 1 << 37;
    const CouldNotCleanDirectory = 1 << 38;
    const PluginTimings = 1 << 39;
    const DuplicateShebang = 1 << 40;
    const TsConfigError = 1 << 41;
    const UnsupportedTsconfigOption = 1 << 42;
    const RuntimeModuleSymbolNotFoundError = 1 << 43;
    const IneffectiveDynamicImport = 1 << 45;
    const RequireTlaError = 1 << 46;
    const LargeBarrelModules = 1 << 47;
    const SourcemapBroken = 1 << 48;
  }
}
