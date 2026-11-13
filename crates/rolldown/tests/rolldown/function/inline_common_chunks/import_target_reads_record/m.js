import { label } from './dep.js';
globalThis.events.push('M body ' + label);
export let count = 0;
export function inc() {
  return ++count;
}
