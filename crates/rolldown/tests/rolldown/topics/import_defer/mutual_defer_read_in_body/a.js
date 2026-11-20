import defer * as b from './b.js';
import { log, read } from './log.js';

// `b.js` requests `a.js`, which is evaluating: the read throws, and `b.js` does not run.
log.push('a reads b: ' + read(() => b.x));
