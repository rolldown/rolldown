import { y, yv } from './y.js';
globalThis.events.push('X body ' + yv);
export function x() {
  return 'x(' + y() + ')';
}
export const xv = 'xv';
