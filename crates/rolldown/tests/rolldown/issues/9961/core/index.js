// Like MSW's `core/index.mjs`: calls an imported cross-module function at the
// module top level, while also re-exporting other entries that are used.
import { checkGlobals } from './checkGlobals.js';

checkGlobals();

export { setupWorker } from './setupWorker.js';
export { http } from './http.js';
