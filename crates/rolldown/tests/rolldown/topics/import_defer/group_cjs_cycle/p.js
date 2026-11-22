import c from './c.cjs';
import * as ns from './c.cjs';
import { observed, seen } from './m.js';

export { c, ns, observed };
export function same() {
  return seen === ns;
}
