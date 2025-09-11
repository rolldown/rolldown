// Auto-generated code, DO NOT EDIT DIRECTLY!
// To edit this generated file you have to edit `tasks/generator/src/generators/checks.rs`

use bitflags::bitflags;
bitflags! {
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
  pub struct EventKindSwitcher: u32 {
    const AmbiguousExternalNamespaceError = 1 << 0;
    const CircularDependency = 1 << 1;
    const Eval = 1 << 2;
    const IllegalIdentifierAsNameError = 1 << 3;
    const InvalidExportOptionError = 1 << 4;
    const InvalidOptionError = 1 << 5;
    const MissingExportError = 1 << 6;
    const MissingGlobalName = 1 << 7;
    const MissingNameOptionForIifeExport = 1 << 8;
    const MissingNameOptionForUmdExportError = 1 << 9;
    const MixedExport = 1 << 10;
    const ParseError = 1 << 11;
    const UnresolvedEntry = 1 << 12;
    const UnresolvedImport = 1 << 13;
    const FilenameConflict = 1 << 14;
    const AssignToImportError = 1 << 15;
    const CommonJsVariableInEsm = 1 << 16;
    const ExportUndefinedVariableError = 1 << 17;
    const ImportIsUndefined = 1 << 18;
    const UnsupportedFeatureError = 1 << 19;
    const EmptyImportMeta = 1 << 20;
    const JsonParseError = 1 << 21;
    const IllegalReassignmentError = 1 << 22;
    const InvalidDefineConfigError = 1 << 23;
    const ResolveError = 1 << 24;
    const UnhandleableError = 1 << 25;
    const UnloadableDependencyError = 1 << 26;
    const IoError = 1 << 27;
    const NapiError = 1 << 28;
    const ConfigurationFieldConflict = 1 << 29;
    const PreferBuiltinFeature = 1 << 30;
    const BundlerInitializeError = 1 << 31;
  }
}
