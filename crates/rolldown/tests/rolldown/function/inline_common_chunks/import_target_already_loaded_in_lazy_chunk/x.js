import { d } from './d.js';
export const x = 'x' + d;
globalThis.loadD = () => import('./d.js');
