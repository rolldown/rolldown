import { log } from './log.js';
// The preparation starts `a.js`, then `b.js`, in the order of the walk from `d.js`.
import defer * as d from './d.js';

log.push('main');
export const value = d.value;
export { log };
