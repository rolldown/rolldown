import { log } from './log.js';
// The group puts `d.js` in another chunk. The preparation and the access use the init function of `d.js`.
import defer * as d from './d.js';

log.push('main');
export const value = d.value;
export { log };
