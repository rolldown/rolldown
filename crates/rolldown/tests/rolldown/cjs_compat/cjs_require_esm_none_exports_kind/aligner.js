// This module has no import/export statements, so it starts with ExportsKind::None.
// It is both imported by an ESM module and required by a CJS module, so it is promoted to
// ExportsKind::CommonJs and gets WrapKind::Cjs, whichever importer is visited first.
var Aligner = function () {};
Aligner.prototype.align = function () {};
