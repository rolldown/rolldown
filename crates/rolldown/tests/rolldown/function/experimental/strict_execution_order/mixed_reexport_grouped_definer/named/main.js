import { LOCAL } from '../shared.js';

globalThis.__seed = 1;
export const local = LOCAL;
export const load = () => import('./lazy.js').then((module) => module.read());
