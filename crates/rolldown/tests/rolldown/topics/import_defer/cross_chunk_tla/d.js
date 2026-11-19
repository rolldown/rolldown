import { log } from './log.js';
import './t.js';

log.push('d');
export const value = 'd:' + log.length;
