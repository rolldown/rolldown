import { read } from './dep.js';
globalThis.__wrappedEvents.push('secondary-anchor');
export function current() {
  return read();
}
