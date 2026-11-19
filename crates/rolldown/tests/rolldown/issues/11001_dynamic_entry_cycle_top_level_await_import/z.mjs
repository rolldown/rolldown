import { setMarker } from './state.mjs';
await import('./a.mjs');
setMarker(2);
export function read() {
  return import('./a.mjs').then((module) => module.value);
}
