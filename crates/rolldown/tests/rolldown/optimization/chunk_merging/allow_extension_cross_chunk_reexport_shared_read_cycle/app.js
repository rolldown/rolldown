export { x, z } from './counter.cjs';
import * as ns from './seg.js';

// `seg.js` re-exports `z` from this module, so this read runs in a cycle of chunks.
const read = (namespace) => namespace.z;
globalThis.earlyZ = read(ns);
