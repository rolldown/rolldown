import { log } from './log.js';
import { h } from './helper.js';
import defer * as inner from './inner.js';

log.push('root');
export function read() {
  return [h, inner.value];
}
