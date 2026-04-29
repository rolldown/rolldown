// This module has no import/export statements, so it starts with ExportsKind::None.
// It should be promoted to ExportsKind::Esm when imported by an ESM module,
// and then when required by a CJS module, it should get WrapKind::Esm.
var Aligner = function () {};
Aligner.prototype.align = function () {};
