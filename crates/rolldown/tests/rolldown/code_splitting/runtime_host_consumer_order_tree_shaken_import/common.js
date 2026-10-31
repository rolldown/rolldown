import { data } from './marker.js';

export const snapshot = globalThis.ready ?? data[globalThis.key ?? 'fallback'];

export function getData() {
  return data;
}
