import { log } from './log.js';
import defer * as d from './d.js';

log.push('main');
const e = await import.defer('./e.js');
log.push('after import.defer');
export const values = [d.value, e.value];
export { log };
