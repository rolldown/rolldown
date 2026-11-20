import { read } from './a.js';

// `d.js` imports `a.js`, which defers `d.js`. The first access evaluates `d.js` after `a.js`.
export const value = read();
