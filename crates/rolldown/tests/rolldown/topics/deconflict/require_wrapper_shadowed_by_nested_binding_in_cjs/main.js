// ESM entry, so `lib.cjs` renders inside a `__commonJSMin((exports, module) => { ... })` closure.
import read from './lib.cjs';

export { read };
