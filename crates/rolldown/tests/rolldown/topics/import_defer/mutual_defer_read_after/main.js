import { log, read } from './log.js';
import './a.js';
import defer * as b from './b.js';

log.push('main reads b: ' + read(() => b.x));
export { log };
