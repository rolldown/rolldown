// ESM entry. `lib.cjs` and `dup.cjs` must be dependencies so both render inside
// `__commonJSMin((exports, module) => { ... })` closures.
import lib from './lib.cjs';

export default lib;
