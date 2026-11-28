import { log } from './log.js';
import './a.js';
import defer * as nsB from './b.js';

log.push('main nsB.x=' + nsB.x);
export { log };
