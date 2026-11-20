import { log } from './log.js';
import './r.js';
import defer * as ns from './c.js';

log.push('main sees ns.v=' + ns.v);
export { log };
