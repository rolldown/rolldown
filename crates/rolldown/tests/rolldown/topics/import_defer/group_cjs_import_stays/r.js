import { value } from './m.js';

export function read() {
  return [value, globalThis.__log];
}
