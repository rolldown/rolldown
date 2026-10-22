import { fromB } from './b.js';
export var a = { mode: 'a' };
export function readA() {
  return a && a.mode === 'a' ? 'a-ready' : 'a-missing';
}
export { fromB };
