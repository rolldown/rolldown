import { log, read } from './log.js';
import defer * as a from './a.js';
import defer * as b from './b.js';

log.push('a: ' + read(() => a.x));
// `b.js` finished its body, but its cycle root `a.js` failed.
log.push('b: ' + read(() => b.y));
export { log };
