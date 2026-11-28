globalThis.__log = [];
import defer * as r1 from './r1.js';
import defer * as r2 from './r2.js';

export function run() {
  return [r1.read(), r2.read(), globalThis.__log];
}
