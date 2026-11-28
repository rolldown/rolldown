import { foo } from './a.js';
import './y.js';

globalThis.order.push('b sees ' + foo);
export function bar() {
  return 'bar';
}
