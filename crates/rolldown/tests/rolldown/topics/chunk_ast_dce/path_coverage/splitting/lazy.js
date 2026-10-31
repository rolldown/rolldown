import { shared } from './shared.js';

globalThis.__pathEvents.push('split-lazy-anchor');
export function read() {
  return 'lazy:' + shared;
}
