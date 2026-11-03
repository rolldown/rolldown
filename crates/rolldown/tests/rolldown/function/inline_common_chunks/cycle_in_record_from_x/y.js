import { x, xv } from './x.js';
globalThis.events.push('Y body ' + x() + ' ' + xv);
export function y() {
  return 'y';
}
export const yv = 'yv';
