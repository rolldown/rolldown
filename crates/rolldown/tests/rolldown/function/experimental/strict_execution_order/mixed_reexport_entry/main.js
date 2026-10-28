export { VALUE } from './value.js';
export const LOCAL = 'local';
export const load = () => import('./shared.js').then((module) => module.FORWARDED);
