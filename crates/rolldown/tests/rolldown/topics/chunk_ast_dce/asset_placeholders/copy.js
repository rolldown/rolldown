import value from './copied.mjs';

export { default as reexported } from './copied.mjs';
export const copied = value;
export const load = () => import('./copied.mjs');
