import { svc0 } from '../services/svc0.js';
import { svc1 } from '../services/svc1.js';
import { val1 } from '../utils/util1.js';
export function render() {
  return svc0('r0') + svc1('r0') + val1;
}
