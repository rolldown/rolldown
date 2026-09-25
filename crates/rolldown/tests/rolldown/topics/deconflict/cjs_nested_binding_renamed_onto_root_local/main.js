// ESM entry, so `lib.cjs` renders inside a `__commonJSMin((exports, module) => { ... })` closure.
import lib from './lib.cjs';

export default lib.read({ tag: 'param' });
