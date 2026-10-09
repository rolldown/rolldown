import { LOCAL } from '../shared.js';

export const local = LOCAL;
export const load = () => import('./lazy.js').then((module) => [module.SNAPSHOT, module.PURE]);
