import { log } from './log.js';
import defer * as d from './d.js';

Promise.resolve().then(() => log.push('microtask'));
log.push('before read');
export const value = d.value;
log.push('after read');
export { log };
