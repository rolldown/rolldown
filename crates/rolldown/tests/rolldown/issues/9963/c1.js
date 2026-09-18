// A "staircase" of shared consumers: c1 -> c2 -> c3, each one a consumer of the
// previous, and each shared with a different overlapping subset of the routes.
// This is what produces several chunks with descending dependent-entry sets,
// which is what makes folding `common` into the entry close an import cycle.
import { common } from './common.js';
export const c1 = 'c1:' + common;
