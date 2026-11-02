import { count } from './route.mjs';

export function cycleRead() {
  return count;
}
globalThis.__event('cycle');
