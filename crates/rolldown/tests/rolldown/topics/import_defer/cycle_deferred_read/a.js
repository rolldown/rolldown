import './b.js';
import defer * as b from './b.js';
import { log, read } from './log.js';

// `b.js` finished its body, but its cycle root `a.js` is still evaluating.
log.push('a reads b: ' + read(() => b.x));
