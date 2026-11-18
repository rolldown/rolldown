import { wrap } from './store.js';
export var value = wrap();
export const later = () => import('./y.js');
