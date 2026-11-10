import c from './c1.cjs';
import './m1.js';

globalThis.__log.push('p1');
export function read() {
  return c.value;
}
