import { x } from './a.js';
// Called after a.js has already called import('./x.js').
export const y = import('./y.js');
export { x };
