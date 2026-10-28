import { compute } from './shared.js';

export const own = compute(21) + 1;
export const done = import('./lazy.js').then((m) => m.lazyValue);

export * from 'node:path';
