import { log } from './log.js';
import defer * as r1 from './r1.js';
import defer * as r2 from './r2.js';

export function run() {
  return [r1.value, r2.value, ...log];
}
