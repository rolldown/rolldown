import { log } from './log.js';
import defer * as first from './first.js';
import defer * as second from './second.js';

export function run() {
  return [first.value, second.value, ...log];
}
