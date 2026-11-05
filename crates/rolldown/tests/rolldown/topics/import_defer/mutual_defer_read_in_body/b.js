import defer * as a from './a.js';
import { log } from './log.js';

log.push('b runs');
export const x = 1;
export const readA = () => a.y;
