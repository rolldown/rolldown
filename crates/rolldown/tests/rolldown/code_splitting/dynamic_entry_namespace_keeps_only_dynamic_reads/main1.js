import { X } from './lib/index.js';
export const fromMain1 = X.value;
export const loadX = () => import('./lib/index.js').then((lib) => lib.X.value);
