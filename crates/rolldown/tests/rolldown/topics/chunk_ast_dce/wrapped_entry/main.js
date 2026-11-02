import { read, change, ready } from './dep.js';
globalThis.__wrappedEvents.push('entry-anchor');
export const initial = read();
export function update() {
  change();
  return read();
}
export const completed = ready;
