import { f0 } from './vendor.js';

console.log(f0(1));

export * from './cjs-dep.cjs';
export const done = import('./lazy.js').then((m) => m.value);
