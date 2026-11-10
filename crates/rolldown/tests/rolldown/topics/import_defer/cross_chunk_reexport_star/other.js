import { b } from './b.js';

globalThis.__events.push('other');

export function other() {
  return b;
}
