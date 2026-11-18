export { x } from './lib.cjs';
export { y } from './barrel.js';
export { z } from './star_lib.cjs';
export { foreign } from './dep.js';
export const load = () => import('./seg_named.js');
export const loadStar = () => import('./seg_star.js');
