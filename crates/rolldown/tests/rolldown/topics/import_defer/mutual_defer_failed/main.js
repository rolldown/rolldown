import { log, read } from './log.js';
import './a.js';
import defer * as b from './b.js';
import defer * as a from './a.js';

log.push('b: ' + read(() => b.x));
// The error of `b.js` does not go to `a.js`: `a.js` finished first, in its own evaluation.
log.push('a: ' + read(() => a.y));
export { log };
