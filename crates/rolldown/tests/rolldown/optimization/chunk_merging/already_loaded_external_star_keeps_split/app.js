import { f0 } from './vendor.js';

console.log(f0(1));

export * from 'node:path';
export const done = import('./lazy.js').then((m) => m.value);
