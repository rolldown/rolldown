import { tag } from './i.js';

// `i.js` runs inside the evaluation of `d.js`, and its top level reads the deferred namespace.
export const value = 'd';
export const seen = tag;
