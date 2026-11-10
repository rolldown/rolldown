import { log } from './log.js';
import defer * as t from './t.js';

log.push('a');
export const a = 'a';
export function readT() {
  return t.value;
}
