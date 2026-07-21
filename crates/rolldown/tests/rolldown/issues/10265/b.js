import { getA } from './a.js';

export function getB() {
  return getA ? 'b' : 'unreachable';
}
