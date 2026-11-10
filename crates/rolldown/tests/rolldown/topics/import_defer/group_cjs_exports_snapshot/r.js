import * as ns from './c.cjs';
import './m.js';

export function read() {
  return [ns.x, 'added' in ns];
}
