import { LOCAL } from './shared.js';

export const local = LOCAL;
export const loadA = () => import('./lazy-a.js').then((module) => module.read());
export const loadB = () => import('./lazy-b.js').then((module) => module.read());
