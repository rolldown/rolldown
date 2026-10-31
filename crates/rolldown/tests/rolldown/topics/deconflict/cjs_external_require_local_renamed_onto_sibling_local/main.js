// The chunk-level `node_path` is the external namespace for this import. `lib.cjs` renders inside
// a `__commonJSMin((exports, module) => { ... })` closure.
import { sep } from 'node:path';
import lib from './lib.cjs';
export default [sep, ...lib()];
