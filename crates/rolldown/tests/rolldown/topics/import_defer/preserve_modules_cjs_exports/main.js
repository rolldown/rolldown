import { log } from './log.js';
import defer * as a from './a.js';

log.push('main');
export const before = log.slice();
export const values = [a.value, a.count, typeof a.Klass, a.read()];
export { log };
