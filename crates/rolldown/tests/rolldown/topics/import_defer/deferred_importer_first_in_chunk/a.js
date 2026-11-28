import defer * as d from './d.js';

globalThis.order.push('a');
export function read() {
  return d.value;
}
