import { value } from './shim.js';

if (globalThis.__rolldown10906PolyfillCalls !== 1) {
  throw new Error('runtime polyfill ran after the entry');
}

export const first = value;
