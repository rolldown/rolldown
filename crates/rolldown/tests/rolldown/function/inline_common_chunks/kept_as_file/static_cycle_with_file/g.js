import { r } from './r.js';
export const g = String(globalThis.__g ?? 'g');
export function readR() {
  return r;
}
