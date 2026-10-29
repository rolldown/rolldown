import { bump } from './s.js';
globalThis.events.push('V body');
export function viaV() {
  return bump();
}
