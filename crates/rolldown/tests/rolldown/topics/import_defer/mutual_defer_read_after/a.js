import defer * as b from './b.js';
import { log } from './log.js';

log.push('a');
export const y = 1;
export const readB = () => b.x;
