import { log } from './log.js';
import defer * as ns from './a.js';

log.push('main x=' + ns.x);
export { log };
