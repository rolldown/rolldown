// ESM entry: imports both aligner (directly) and default_aligner (CJS that requires aligner)
// The import of aligner should promote it from ExportsKind::None to ExportsKind::Esm.
// Then when default_aligner's require of aligner is processed, aligner should have
// ExportsKind::Esm, causing it to get WrapKind::Esm.
import './aligner';
import DefaultAligner from './default_aligner';

console.log(DefaultAligner);
