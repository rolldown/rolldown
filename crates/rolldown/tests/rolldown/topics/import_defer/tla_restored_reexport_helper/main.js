import { log } from './log.js';
import { y } from './b.js';
import defer * as ns from './a.js';

log.push('main y=' + y + ' x=' + ns.x);
export { log };
