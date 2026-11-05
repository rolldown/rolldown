import { log } from './log.js';
import defer * as d from './d.js';

log.push('main reads d: ' + d.value);
export { log };
