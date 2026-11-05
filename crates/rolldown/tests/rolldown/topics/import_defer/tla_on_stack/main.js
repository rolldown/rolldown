import { log } from './log.js';
// The walk from `d.js` stops at `main.js`, which is on the stack. So `i.js` does not wait for `a.js`.
import './i.js';
import './a.js';

log.push('main');
export { log };
