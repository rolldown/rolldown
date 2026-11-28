import { bar } from './b.js';

globalThis.order.push('c sees ' + typeof bar);
export function baz() {
  return 'baz';
}
