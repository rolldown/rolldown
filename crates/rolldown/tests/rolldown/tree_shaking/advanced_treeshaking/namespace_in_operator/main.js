import * as ns from './mod.js';
export const value = 'a' in ns ? ns.a : 0;
export const missing = 'nope' in ns;
