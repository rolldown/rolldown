// ESM entry: imports both aligner (directly) and default_aligner (CJS that requires aligner)
// Because default_aligner requires aligner, aligner is CommonJS even though this module
// imports it first, and both sites call the same `require_aligner()` wrapper.
import './aligner';
import DefaultAligner from './default_aligner';

console.log(DefaultAligner);
