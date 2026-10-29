import './m1.js';
import { m2 } from './m2.js';
import './m6.js';
import { log } from './log.js';
log.push('m0');
export const m0 = 'm0';
export const viaM2 = m2;
export function run() {
  return log.slice();
}
