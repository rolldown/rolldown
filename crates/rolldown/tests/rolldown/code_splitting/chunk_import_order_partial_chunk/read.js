import { marker } from './marker.js';
const snapshot = globalThis.ready ?? 'unset';
export function read() {
  return snapshot + ':' + marker();
}
