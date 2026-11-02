import { read } from './private.js';

globalThis.__wrappedEvents.push('named-wrapper-anchor');
export { read };
export let current = 1;
export function increment() {
  current += 1;
}
