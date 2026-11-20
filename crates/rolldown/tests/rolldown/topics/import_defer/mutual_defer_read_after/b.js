import defer * as a from './a.js';
import { log, read } from './log.js';

log.push('b start');
// `a.js` finished before `b.js` started, so the read works.
log.push('b reads a: ' + read(() => a.y));
export const x = 2;
