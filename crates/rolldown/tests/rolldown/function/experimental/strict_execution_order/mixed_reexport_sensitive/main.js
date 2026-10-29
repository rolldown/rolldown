import { LOCAL } from './shared.js';
import { bump } from './state.js';

export const local = LOCAL;
bump();
export const load = () => import('./lazy.js').then((module) => module.read());
