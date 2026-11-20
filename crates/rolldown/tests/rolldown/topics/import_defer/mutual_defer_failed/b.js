import defer * as a from './a.js';
import { log } from './log.js';

log.push('b throws');
export const readA = () => a.y;
throw new Error('B failed');
